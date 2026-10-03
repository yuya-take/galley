use async_trait::async_trait;

use super::{Document, DocumentListItem, DocumentQuery, DocumentSlug};
use crate::domain::{
    blob::Blob,
    error::RepositoryError,
    revision::Revision,
    shared::{DocumentId, ProjectId},
};

/// 資料の永続化。実装は `adapter`。
#[async_trait]
pub trait DocumentRepository: Send + Sync {
    /// 資料と第1版、ブロブの情報を1トランザクションで保存する。
    /// ブロブの情報は同じハッシュがあれば保存しない。
    async fn insert_with_first_revision(
        &self,
        document: &Document,
        revision: &Revision,
        blob: &Blob,
    ) -> Result<(), RepositoryError>;

    /// 新しい版と、資料の現在版・更新日時を1トランザクションで保存する。
    /// ブロブの情報は渡されたときだけ、同じハッシュが無ければ保存する（戻す操作では渡さない）。
    ///
    /// 同じ番号の版が既にある（同時に別の版が追加された）ときは [`RepositoryError::Conflict`] を返す。
    async fn insert_revision(
        &self,
        document: &Document,
        revision: &Revision,
        blob: Option<&Blob>,
    ) -> Result<(), RepositoryError>;

    /// 資料名とアーカイブの状態を保存する。版と現在版は変えない。
    async fn update(&self, document: &Document) -> Result<(), RepositoryError>;

    async fn find_by_id(&self, id: DocumentId) -> Result<Option<Document>, RepositoryError>;

    /// アーカイブした資料も対象にする。
    async fn find_by_slug(
        &self,
        project_id: ProjectId,
        slug: &DocumentSlug,
    ) -> Result<Option<Document>, RepositoryError>;

    /// プロジェクト内で `prefix` から始まる slug をすべて返す（アーカイブした資料も含む）。
    /// slug が衝突したときに `-2` などを付けるために使う。
    async fn slugs_with_prefix(
        &self,
        project_id: ProjectId,
        prefix: &str,
    ) -> Result<Vec<DocumentSlug>, RepositoryError>;

    async fn list(&self, query: &DocumentQuery) -> Result<Vec<DocumentListItem>, RepositoryError>;
}
