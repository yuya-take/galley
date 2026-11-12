use async_trait::async_trait;

use super::{Document, DocumentListItem, DocumentQuery, DocumentSlug, PurgedRevisions};
use crate::domain::{
    blob::Blob,
    error::RepositoryError,
    project::Project,
    revision::Revision,
    shared::{DocumentId, ProjectId, RevisionId},
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

    /// 資料と、資料が属するプロジェクトを返す。版を追加する前に、プロジェクトのアーカイブを確かめるのに使う。
    async fn find_with_project(
        &self,
        id: DocumentId,
    ) -> Result<Option<(Document, Project)>, RepositoryError>;

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

    /// 資料を物理削除する。資料とそのすべての版を1トランザクションで消し、消した版のブロブのうち、
    /// どの版からも参照されなくなったものの情報も消す。ストレージの実体は消さない（呼び出し側が、
    /// コミットのあとに消す）。資料が無ければ何も消さない。
    async fn purge(&self, id: DocumentId) -> Result<PurgedRevisions, RepositoryError>;

    /// 版を1つ物理削除する。版の番号は詰めない（欠番になる）。ブロブの扱いは [`Self::purge`] と同じ。
    /// 最新の版でないことは、呼び出し側が [`Document::ensure_can_purge_revision`] で確かめる。
    /// 版が無ければ何も消さない。
    async fn purge_revision(&self, id: RevisionId) -> Result<PurgedRevisions, RepositoryError>;
}
