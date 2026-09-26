use crate::domain::{
    crest_color::CrestColorError, repository::RepositoryError, slug::SlugError, text::TextError,
};

/// ユースケースのエラー。画面と MCP はこれを利用者向けの表示に変換する。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    InvalidSlug(#[from] SlugError),
    #[error(transparent)]
    InvalidText(#[from] TextError),
    #[error(transparent)]
    InvalidCrestColor(#[from] CrestColorError),
    #[error("プロジェクトが見つかりません")]
    ProjectNotFound,
    #[error("資料が見つかりません")]
    DocumentNotFound,
    #[error("URL「{0}」はほかのプロジェクトが使っています")]
    ProjectSlugTaken(String),
    #[error(transparent)]
    Repository(RepositoryError),
}

impl From<RepositoryError> for AppError {
    fn from(err: RepositoryError) -> Self {
        Self::Repository(err)
    }
}
