use jiff::Timestamp;

use crate::domain::{
    revision::AuthorName,
    shared::{ArchiveFilter, ProjectId},
};

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
