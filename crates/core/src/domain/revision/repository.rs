use async_trait::async_trait;

use super::{Revision, RevisionNumber};
use crate::domain::{
    error::RepositoryError,
    shared::{DocumentId, RevisionId},
};

/// 版の読み出し。版の保存は資料と1トランザクションで行うので [`DocumentRepository`] が持つ。
///
/// [`DocumentRepository`]: crate::domain::document::DocumentRepository
#[async_trait]
pub trait RevisionRepository: Send + Sync {
    async fn find_by_id(&self, id: RevisionId) -> Result<Option<Revision>, RepositoryError>;

    async fn find_by_number(
        &self,
        document_id: DocumentId,
        number: RevisionNumber,
    ) -> Result<Option<Revision>, RepositoryError>;
}
