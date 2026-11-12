use async_trait::async_trait;

use super::{Revision, RevisionNumber};
use crate::domain::{
    blob::BlobHash,
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

    /// 資料の版を新しい順に返す。`limit` を指定すれば、その件数まで（プレビュー欄の直近の版など）。
    async fn list_by_document(
        &self,
        document_id: DocumentId,
        limit: Option<u32>,
    ) -> Result<Vec<Revision>, RepositoryError>;

    /// `hashes` のどれかのブロブを使っている版を、資料をまたいですべて返す（物理削除で、ブロブを
    /// 消してよいかを確かめるのに使う）。
    async fn list_by_blob_hashes(
        &self,
        hashes: &[BlobHash],
    ) -> Result<Vec<Revision>, RepositoryError>;
}
