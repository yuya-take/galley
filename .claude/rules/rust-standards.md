---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---

# Rust Development Standards

## Commands

- `cargo build` - Build
- `cargo run -p galley-server` - Run the app
- `cargo test --workspace` - Run tests
- `cargo fmt --all` - Format code
- `cargo clippy --workspace --all-targets -- -D warnings` - Run linter（CI と同じ）

## Crates

| クレート | 役割 |
|---------|------|
| `crates/core` (`galley-core`) | アップロード検査、リビジョン作成、ストレージ。**Topcoat と `galley-web` に依存してはいけない** |
| `crates/web` (`galley-web`) | Topcoat の画面 |
| `crates/server` (`galley-server`) | 起動処理、Axum のルーター（資料配信・MCP）、バイナリ `galley` |

- ドメイン処理は `galley-core` に置き、画面（web）と MCP（server）の両方から呼ぶ
- Topcoat は実験段階で破壊的変更が前提なので、Topcoat の型を `galley-core` の公開 API に出さない
- 依存のバージョンはルートの `[workspace.dependencies]` で管理し、各クレートは `workspace = true` で参照する

## Libraries

- **Async Runtime**: `tokio`
- **Web**: `axum`（資料配信・MCP）、Topcoat（画面）
- **MCP**: `rmcp`
- **DB**: SQLite + `toasty`
- **Storage**: `object_store`
- **Error Handling**: `thiserror`（`galley-core` のエラー型）、`anyhow`（`galley-server` の起動処理）
- **Serialization**: `serde` + `serde_json`
- **Logging**: `tracing` + `tracing-subscriber`（`println!` / `dbg!` は残さない）
