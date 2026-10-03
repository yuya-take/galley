use std::sync::Arc;

use jiff::Timestamp;

use super::{SlugSource, choose_slug::choose_slug};
use crate::{
    app::{
        error::AppError,
        project,
        revision::{PreparedRevision, RegisteredRevision, RevisionInput},
    },
    domain::{
        blob::BlobStore,
        document::{Document, DocumentRepository, DocumentTitle},
        error::RepositoryError,
        project::ProjectRepository,
        shared::ProjectId,
    },
};

/// 新しい資料の登録内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDocumentInput {
    pub project_id: ProjectId,
    /// 資料名。無ければ HTML の `<title>`、それも無ければ「無題の資料」。
    pub title: Option<String>,
    pub slug: SlugSource,
    pub revision: RevisionInput,
}

/// 新しい資料を第1版として登録する。
pub struct CreateDocument {
    projects: Arc<dyn ProjectRepository>,
    documents: Arc<dyn DocumentRepository>,
    blobs: Arc<dyn BlobStore>,
}

/// 同時に同じ slug の資料が作られたとき、slug を選び直す回数。
const MAX_ATTEMPTS: usize = 3;

impl CreateDocument {
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        documents: Arc<dyn DocumentRepository>,
        blobs: Arc<dyn BlobStore>,
    ) -> Self {
        Self {
            projects,
            documents,
            blobs,
        }
    }

    /// HTML の検査で CPU を使う（10MB で 0.1 秒ほど）ので、非同期の処理からはブロックする処理として呼ぶ。
    pub async fn execute(
        &self,
        input: CreateDocumentInput,
    ) -> Result<RegisteredRevision, AppError> {
        let project = project::load(self.projects.as_ref(), input.project_id).await?;
        project.ensure_accepts_documents()?;
        let prepared = PreparedRevision::parse(input.revision)?;
        let title = DocumentTitle::for_new_document(
            input.title.as_deref(),
            prepared.html_title.as_deref(),
        )?;
        let now = Timestamp::now();
        let blob = prepared.store(self.blobs.as_ref(), now).await?;

        let mut attempt = 1;
        loop {
            let slug = choose_slug(self.documents.as_ref(), project.id, &input.slug).await?;
            let (document, revision) = Document::create(
                project.id,
                slug,
                title.clone(),
                prepared.new_revision(),
                now,
            );
            match self
                .documents
                .insert_with_first_revision(&document, &revision, &blob)
                .await
            {
                Err(RepositoryError::Conflict(_)) if attempt < MAX_ATTEMPTS => attempt += 1,
                saved => {
                    saved?;
                    return Ok(RegisteredRevision { document, revision });
                }
            }
        }
    }
}
