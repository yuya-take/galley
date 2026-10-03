//! エンティティ、値オブジェクト、リポジトリのトレイト。I/O はしない。
//!
//! 集約（プロジェクト・資料・版）ごとにモジュールを分け、各モジュールの `mod.rs` から公開する。
//! 複数の集約で使うものは [`shared`] に置く。

pub mod blob;
pub mod document;
pub mod error;
pub mod project;
pub mod revision;
pub mod shared;
pub mod upload;
