//! object_store による資料の実体の保存を、ローカルのディレクトリとメモリで確かめる。

use std::{error::Error, path::PathBuf};

use galley_core::{
    adapter::blob_store::ObjectStoreBlobStore,
    domain::{
        blob::{BlobContent, BlobHash, BlobStore},
        error::StorageError,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

/// テストごとの一時ディレクトリ。終わったら消す。
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("galley-blob-test-{}", uuid::Uuid::new_v4())))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn html(body: &str) -> BlobContent {
    BlobContent::new(format!("<!doctype html><p>{body}</p>").into_bytes())
}

#[tokio::test]
async fn local_store_writes_under_hash_directories() -> TestResult {
    let dir = TempDir::new();
    let store = ObjectStoreBlobStore::local(&dir.0)?;
    let content = html("こんにちは");

    store.put(&content).await?;

    let path = dir.0.join(content.hash().storage_key());
    assert_eq!(std::fs::read(&path)?, content.as_bytes());
    let hash = content.hash().as_str();
    assert!(path.ends_with(format!("{}/{}/{hash}.html", &hash[..2], &hash[2..4])));
    assert_eq!(store.get(content.hash()).await?, Some(content));
    Ok(())
}

#[tokio::test]
async fn put_skips_existing_hash() -> TestResult {
    let dir = TempDir::new();
    let store = ObjectStoreBlobStore::local(&dir.0)?;
    let content = html("同じ内容");
    store.put(&content).await?;
    let path = dir.0.join(content.hash().storage_key());
    let written_at = std::fs::metadata(&path)?.modified()?;

    store.put(&content).await?;

    assert_eq!(std::fs::metadata(&path)?.modified()?, written_at);
    Ok(())
}

#[tokio::test]
async fn get_returns_none_for_missing_hash() -> TestResult {
    let dir = TempDir::new();
    let store = ObjectStoreBlobStore::local(&dir.0)?;
    assert_eq!(store.get(&BlobHash::of(b"missing")).await?, None);
    Ok(())
}

#[tokio::test]
async fn get_rejects_tampered_file() -> TestResult {
    let dir = TempDir::new();
    let store = ObjectStoreBlobStore::local(&dir.0)?;
    let content = html("元の内容");
    store.put(&content).await?;
    std::fs::write(dir.0.join(content.hash().storage_key()), "書き換えた内容")?;

    let result = store.get(content.hash()).await;

    assert!(matches!(result, Err(StorageError::Corrupted(_))));
    Ok(())
}

#[tokio::test]
async fn in_memory_store_round_trips() -> TestResult {
    let store = ObjectStoreBlobStore::in_memory();
    let content = html("メモリ");
    store.put(&content).await?;
    store.put(&content).await?;
    assert_eq!(store.get(content.hash()).await?, Some(content));
    Ok(())
}
