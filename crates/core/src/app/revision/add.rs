use std::sync::Arc;

use jiff::Timestamp;

use super::{PreparedRevision, RegisteredRevision, RevisionInput, load_current, should_retry};
use crate::{
    app::{document::load, error::AppError},
    domain::{
        blob::BlobStore, document::DocumentRepository, revision::RevisionRepository,
        shared::DocumentId,
    },
};

/// 既存の資料に新しい版を追加する。
pub struct AddRevision {
    documents: Arc<dyn DocumentRepository>,
    revisions: Arc<dyn RevisionRepository>,
    blobs: Arc<dyn BlobStore>,
}

impl AddRevision {
    pub fn new(
        documents: Arc<dyn DocumentRepository>,
        revisions: Arc<dyn RevisionRepository>,
        blobs: Arc<dyn BlobStore>,
    ) -> Self {
        Self {
            documents,
            revisions,
            blobs,
        }
    }

    /// HTML の検査で CPU を使う（10MB で 0.1 秒ほど）ので、非同期の処理からはブロックする処理として呼ぶ。
    pub async fn execute(
        &self,
        document_id: DocumentId,
        input: RevisionInput,
    ) -> Result<RegisteredRevision, AppError> {
        // 資料が無い・アーカイブ済みなら、HTML の検査とストレージへの書き込みの前に断る
        let document = load(self.documents.as_ref(), document_id).await?;
        document.ensure_accepts_revisions()?;
        let prepared = PreparedRevision::parse(input)?;
        let now = Timestamp::now();
        let blob = prepared.store(self.blobs.as_ref(), now).await?;

        let mut attempt = 1;
        loop {
            let mut document = load(self.documents.as_ref(), document_id).await?;
            let current = load_current(self.revisions.as_ref(), &document).await?;
            let revision = document.add_revision(&current, prepared.new_revision(), now)?;
            let saved = self
                .documents
                .insert_revision(&document, &revision, Some(&blob))
                .await;
            if should_retry(&saved, attempt) {
                attempt += 1;
                continue;
            }
            saved?;
            return Ok(RegisteredRevision { document, revision });
        }
    }
}
