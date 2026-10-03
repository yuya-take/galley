use crate::domain::{
    document::DocumentError,
    error::{RepositoryError, StorageError},
    project::{CrestColorError, ProjectError},
    shared::{SlugError, TextError},
    upload::UploadError,
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
    #[error("版が見つかりません")]
    RevisionNotFound,
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    InvalidUpload(#[from] UploadError),
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error("URL「{0}」はほかのプロジェクトが使っています")]
    ProjectSlugTaken(String),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Storage(#[from] StorageError),
}
