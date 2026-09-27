use async_trait::async_trait;

use super::{BlobContent, BlobHash};
use crate::domain::error::StorageError;

/// 資料の実体を置くストレージ。同じハッシュの実体は1回だけ書く。
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// 実体を書く。同じハッシュの実体が既にあれば何もしない。
    async fn put(&self, content: &BlobContent) -> Result<(), StorageError>;

    /// ハッシュで実体を読む。無ければ `None`。
    ///
    /// 読んだ内容のハッシュが一致しないときは [`StorageError::Corrupted`] を返す。
    async fn get(&self, hash: &BlobHash) -> Result<Option<BlobContent>, StorageError>;
}
