//! リポジトリ（DB）とストレージのエラー。

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

/// ストレージ（資料の実体の保存先）のエラー。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// 保存されている実体がハッシュと一致しない。
    #[error("保存されている資料の実体が壊れています: {0}")]
    Corrupted(String),
    #[error(transparent)]
    Backend(Box<dyn std::error::Error + Send + Sync>),
}
