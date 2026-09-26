//! 永続化のトレイト。実装は `adapter`。

use async_trait::async_trait;
use jiff::Timestamp;

use super::{
    blob::Blob,
    document::{Document, DocumentListItem},
    id::{DocumentId, ProjectId},
    project::{Project, ProjectSummary},
    revision::Revision,
    slug::{DocumentSlug, ProjectSlug},
    text::AuthorName,
};

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// 一意制約に反した（slug の重複など）。
    #[error("すでに同じものがあります: {0}")]
    Conflict(String),
    /// 保存されている値が壊れている。
    #[error("保存されているデータを読めません: {0}")]
    Corrupted(String),
    #[error(transparent)]
    Backend(Box<dyn std::error::Error + Send + Sync>),
}

/// アーカイブしたものを一覧に含めるか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArchiveFilter {
    /// アーカイブしていないものだけ。
    #[default]
    Active,
    /// アーカイブしたものだけ。
    Archived,
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn insert(&self, project: &Project) -> Result<(), RepositoryError>;

    async fn update(&self, project: &Project) -> Result<(), RepositoryError>;

    async fn find_by_id(&self, id: ProjectId) -> Result<Option<Project>, RepositoryError>;

    async fn find_by_slug(&self, slug: &ProjectSlug) -> Result<Option<Project>, RepositoryError>;

    /// 作成順に返す（名前順は漢字の読みの順にならないため）。
    /// 資料の件数はアーカイブしていない資料だけ数える。
    async fn list(&self, filter: ArchiveFilter) -> Result<Vec<ProjectSummary>, RepositoryError>;
}

/// 資料の一覧の並び順。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentOrder {
    /// 更新が新しい順。
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    /// 資料名の昇順。
    TitleAsc,
    TitleDesc,
}

/// 資料の一覧の条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocumentQuery {
    /// `None` なら全プロジェクト（アーカイブしたプロジェクトの資料は除く）。
    pub project_id: Option<ProjectId>,
    pub archive: ArchiveFilter,
    /// 資料名に含まれる文字列（「この中を絞り込む」、⌘K の検索）。
    pub title_contains: Option<String>,
    /// この日時以降に更新した資料だけ（「今週更新」）。
    pub updated_since: Option<Timestamp>,
    /// この名前で版を登録したことがある資料だけ（「自分が更新」）。
    pub author_name: Option<AuthorName>,
    pub order: DocumentOrder,
    pub limit: Option<u32>,
}

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
