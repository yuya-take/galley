//! Galley のドメイン処理。
//!
//! 画面（`galley-web`）と MCP（`galley-server`）の両方から呼ばれるため、Topcoat・Axum・rmcp
//! には依存しない。中は依存の向きを内側に揃えた3層に分ける（`.claude/rules/clean-architecture.md`）。
//!
//! - [`domain`]: エンティティ、値オブジェクト、リポジトリのトレイト。I/O をしない
//! - [`app`]: ユースケース。domain のトレイト経由で DB を使う
//! - [`adapter`]: domain のトレイトの実装（SQLite など）

pub mod adapter;
pub mod app;
pub mod domain;
