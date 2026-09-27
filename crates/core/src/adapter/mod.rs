//! domain のトレイトの実装（DB、ストレージ）。組み立ては `galley-server` の `main.rs` で行う。

pub mod blob_store;
pub mod sqlite;
