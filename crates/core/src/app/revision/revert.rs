use std::sync::Arc;

use jiff::Timestamp;

use super::{RegisteredRevision, load_current, should_retry};
use crate::{
    app::{document::load, error::AppError},
    domain::{
        document::DocumentRepository,
        revision::{AuthorName, RevisionNumber, RevisionRepository, RevisionSource},
        shared::DocumentId,
    },
};

/// 過去の版に戻す。過去の版と同じ内容の新しい版を追加する（履歴は書き換えない）。
pub struct RevertToRevision {
    documents: Arc<dyn DocumentRepository>,
    revisions: Arc<dyn RevisionRepository>,
}

impl RevertToRevision {
    pub fn new(
        documents: Arc<dyn DocumentRepository>,
        revisions: Arc<dyn RevisionRepository>,
    ) -> Self {
        Self {
            documents,
            revisions,
        }
    }

    /// `number` は戻す先の版番号（第N版）。
    pub async fn execute(
        &self,
        document_id: DocumentId,
        number: u32,
        author_name: &str,
        source: RevisionSource,
    ) -> Result<RegisteredRevision, AppError> {
        let author_name = AuthorName::parse(author_name)?;
        let number = RevisionNumber::new(number).map_err(|_| AppError::RevisionNotFound)?;
        let now = Timestamp::now();

        let mut attempt = 1;
        loop {
            let mut document = load(self.documents.as_ref(), document_id).await?;
            let current = load_current(self.revisions.as_ref(), &document).await?;
            let target = self
                .revisions
                .find_by_number(document.id, number)
                .await?
                .ok_or(AppError::RevisionNotFound)?;
            let revision =
                document.revert_to(&current, &target, author_name.clone(), source, now)?;
            // 実体は過去の版と同じで保存済みなので、ブロブは渡さない
            let saved = self
                .documents
                .insert_revision(&document, &revision, None)
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
