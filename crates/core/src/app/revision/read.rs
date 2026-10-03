use std::sync::Arc;

use crate::{
    app::error::AppError,
    domain::{
        blob::BlobStore,
        error::StorageError,
        revision::{Revision, RevisionRepository},
        shared::RevisionId,
    },
};

/// 版の HTML。資料配信がそのまま返す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionContent {
    pub revision: Revision,
    pub html: Vec<u8>,
}

/// 版の HTML を読む。アーカイブした資料の版も読める（アーカイブから戻せるため）。
pub struct ReadRevisionContent {
    revisions: Arc<dyn RevisionRepository>,
    blobs: Arc<dyn BlobStore>,
}

impl ReadRevisionContent {
    pub fn new(revisions: Arc<dyn RevisionRepository>, blobs: Arc<dyn BlobStore>) -> Self {
        Self { revisions, blobs }
    }

    pub async fn execute(&self, id: RevisionId) -> Result<RevisionContent, AppError> {
        let revision = self
            .revisions
            .find_by_id(id)
            .await?
            .ok_or(AppError::RevisionNotFound)?;
        // 実体は DB より先に保存しているので、無ければ保存先が壊れている
        let content = self.blobs.get(&revision.blob_hash).await?.ok_or_else(|| {
            StorageError::Corrupted(format!(
                "版 {} の実体（{}）がありません",
                revision.id, revision.blob_hash
            ))
        })?;
        Ok(RevisionContent {
            revision,
            html: content.into_bytes(),
        })
    }
}
