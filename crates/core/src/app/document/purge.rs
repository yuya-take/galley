use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use crate::{
    app::error::AppError,
    domain::{
        blob::{BlobHash, BlobStore},
        document::{BlobDisposal, Document, DocumentRepository, DocumentSlug, PurgedRevisions},
        project::{Project, ProjectRepository, ProjectSlug},
        revision::{Revision, RevisionNumber, RevisionRepository},
        shared::DocumentId,
    },
};

/// 資料ごと、または版を1つ物理削除する。画面と MCP には置かず、コンテナ内で実行する
/// `galley purge` だけが使う（ログインが無いので、画面から消せると誰でも消せてしまうため）。
///
/// [`Self::plan`] で消すものを確かめてから、[`Self::execute`] で消す。DB から版と資料を消して
/// コミットしてから、どの版からも参照されなくなったブロブの実体をストレージから消す。
pub struct PurgeDocument {
    projects: Arc<dyn ProjectRepository>,
    documents: Arc<dyn DocumentRepository>,
    revisions: Arc<dyn RevisionRepository>,
    blobs: Arc<dyn BlobStore>,
}

/// 消す前に見せる内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgePlan {
    pub project: Project,
    pub document: Document,
    /// 版だけを消すときの、その版。`None` なら資料ごと消す。
    pub revision: Option<Revision>,
    /// 消す版（新しい順）。
    pub revisions: Vec<Revision>,
    /// 消す版だけが使っているので、一緒に消すブロブ。
    pub blobs: Vec<BlobHash>,
    /// ほかの版も使っているので残すブロブ。
    pub kept_blobs: Vec<KeptBlob>,
}

/// ほかの版も使っているので残すブロブ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptBlob {
    pub hash: BlobHash,
    /// 使っている版。
    pub used_by: Vec<RevisionLocation>,
}

/// 版の場所（プロジェクトと資料の slug、版の番号）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionLocation {
    pub project: ProjectSlug,
    pub document: DocumentSlug,
    pub number: RevisionNumber,
}

/// 消した結果。
#[derive(Debug)]
pub struct PurgeOutcome {
    /// DB から消した版と、情報を消したブロブ。
    pub purged: PurgedRevisions,
    /// ストレージから実体を消したブロブ。
    pub deleted_blobs: Vec<BlobHash>,
    /// DB から消したあと、実体を消す直前に、同時に登録された版が使い始めていたので残したブロブ。
    pub reused_blobs: Vec<BlobHash>,
    /// 実体を消せなかったブロブ（DB からは消えている）。
    pub failed_blobs: Vec<(BlobHash, AppError)>,
}

impl PurgeDocument {
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        documents: Arc<dyn DocumentRepository>,
        revisions: Arc<dyn RevisionRepository>,
        blobs: Arc<dyn BlobStore>,
    ) -> Self {
        Self {
            projects,
            documents,
            revisions,
            blobs,
        }
    }

    /// 消すものを調べる。`revision` を省略すると資料ごと。アーカイブした資料・プロジェクトも対象にする。
    /// 最新の版は版だけでは消せない（[`Document::ensure_can_purge_revision`]）。
    pub async fn plan(
        &self,
        project: &str,
        document: &str,
        revision: Option<u32>,
    ) -> Result<PurgePlan, AppError> {
        let project_slug = ProjectSlug::parse(project).map_err(|_| AppError::ProjectNotFound)?;
        let project = self
            .projects
            .find_by_slug(&project_slug)
            .await?
            .ok_or(AppError::ProjectNotFound)?;
        let document_slug =
            DocumentSlug::parse(document).map_err(|_| AppError::DocumentNotFound)?;
        let document = self
            .documents
            .find_by_slug(project.id, &document_slug)
            .await?
            .ok_or(AppError::DocumentNotFound)?;

        let (revision, revisions) = match revision {
            None => (
                None,
                self.revisions.list_by_document(document.id, None).await?,
            ),
            Some(number) => {
                let number = RevisionNumber::new(number).map_err(|_| AppError::RevisionNotFound)?;
                let revision = self
                    .revisions
                    .find_by_number(document.id, number)
                    .await?
                    .ok_or(AppError::RevisionNotFound)?;
                document.ensure_can_purge_revision(&revision)?;
                (Some(revision.clone()), vec![revision])
            }
        };

        let hashes: Vec<BlobHash> = revisions.iter().map(|r| r.blob_hash.clone()).collect();
        let sharing = self.revisions.list_by_blob_hashes(&hashes).await?;
        let BlobDisposal { deleted, kept } = BlobDisposal::split(&revisions, &sharing);
        let kept_blobs = self.locate(kept).await?;

        Ok(PurgePlan {
            project,
            document,
            revision,
            revisions,
            blobs: deleted,
            kept_blobs,
        })
    }

    /// 計画どおりに消す。資料ごとのときは、計画のあとに足された版も消す。
    ///
    /// DB から消したあとのストレージの失敗はエラーにせず、[`PurgeOutcome::failed_blobs`] で返す
    /// （DB からは消えているので、やり直しても見つからない。呼び出し側が知らせる）。
    pub async fn execute(&self, plan: &PurgePlan) -> Result<PurgeOutcome, AppError> {
        let purged = match &plan.revision {
            None => self.documents.purge(plan.document.id).await?,
            Some(revision) => {
                plan.document.ensure_can_purge_revision(revision)?;
                self.documents.purge_revision(revision.id).await?
            }
        };

        let mut outcome = PurgeOutcome {
            purged,
            deleted_blobs: Vec::new(),
            reused_blobs: Vec::new(),
            failed_blobs: Vec::new(),
        };
        for hash in &outcome.purged.unreferenced_blobs {
            // コミットのあとに同じ内容の版が登録されていれば、その版の実体なので消さない
            match self
                .revisions
                .list_by_blob_hashes(std::slice::from_ref(hash))
                .await
            {
                Ok(users) if users.is_empty() => {}
                Ok(_) => {
                    outcome.reused_blobs.push(hash.clone());
                    continue;
                }
                Err(err) => {
                    outcome.failed_blobs.push((hash.clone(), err.into()));
                    continue;
                }
            }
            match self.blobs.delete(hash).await {
                Ok(()) => outcome.deleted_blobs.push(hash.clone()),
                Err(err) => outcome.failed_blobs.push((hash.clone(), err.into())),
            }
        }
        Ok(outcome)
    }

    /// 残すブロブを使っている版の場所（プロジェクトと資料の slug）を調べる。
    async fn locate(
        &self,
        kept: Vec<(BlobHash, Vec<Revision>)>,
    ) -> Result<Vec<KeptBlob>, AppError> {
        let mut slugs: HashMap<DocumentId, Option<(ProjectSlug, DocumentSlug)>> = HashMap::new();
        let mut kept_blobs = Vec::with_capacity(kept.len());
        for (hash, users) in kept {
            let mut used_by = Vec::with_capacity(users.len());
            for user in users {
                let place = match slugs.entry(user.document_id) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(
                        self.documents
                            .find_with_project(user.document_id)
                            .await?
                            .map(|(document, project)| (project.slug, document.slug)),
                    ),
                };
                // 調べている間に消された資料は飛ばす
                if let Some((project, document)) = place {
                    used_by.push(RevisionLocation {
                        project: project.clone(),
                        document: document.clone(),
                        number: user.number,
                    });
                }
            }
            kept_blobs.push(KeptBlob { hash, used_by });
        }
        Ok(kept_blobs)
    }
}
