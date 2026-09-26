# コントリビューションガイド

Galley への貢献を歓迎します。バグ報告、機能の提案、ドキュメントの修正、コードの PR のどれでも助かります。

## はじめに

- 設計の全体像は [docs/design.md](docs/design.md) にあります。大きな変更の前に読んでください
- 機能の追加や設計に関わる変更は、PR の前に Issue で相談してください。小さなバグ修正やドキュメントの修正は、そのまま PR を出して構いません
- 脆弱性は公開の Issue にせず、[SECURITY.md](SECURITY.md) の手順で報告してください

## 開発環境

必要なもの：

- [rustup](https://rustup.rs/)：Rust のバージョンは `rust-toolchain.toml` で固定しているので、リポジトリ内で `cargo` を実行すると自動で入ります
- [topcoat-cli](https://crates.io/crates/topcoat-cli)：画面のアセット（スクリプトなど）をバンドルするのに使います

```bash
cargo install topcoat-cli --locked
```

起動：

```bash
cargo build
topcoat asset bundle          # 実行ファイルの隣に assets/ を作る
cargo run -p galley-server
```

アプリは http://localhost:8080 、資料配信は http://localhost:8081 で動きます。ポートが空いていないときは、環境変数 `GALLEY_APP_ADDR` / `GALLEY_VIEWER_ADDR` で変えてください（例：`GALLEY_APP_ADDR=127.0.0.1:18080`）。DB はカレントディレクトリの `data/galley.db` にでき（`GALLEY_DATA_DIR` で変更可）、起動時にマイグレーションが自動で適用されます。

## PR を出す前に

CI と同じ検査を手元で実行してください。

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## クレート構成

| クレート | 役割 |
| --- | --- |
| `crates/core` | ドメイン処理（アップロード検査、リビジョン作成、ストレージ） |
| `crates/web` | 画面（Topcoat） |
| `crates/server` | 起動処理、ルーター（資料配信・MCP）、バイナリ `galley` |
| `crates/migrate` | マイグレーションを作る開発用ツール（配布物には含めない） |

`crates/core` は画面やフレームワーク（Topcoat、Axum、rmcp）に依存してはいけません。CI の `architecture` ジョブで検査しています。詳しくは設計書の「クレート構成」を見てください。

### DB のテーブルを変えるとき

`crates/core/src/adapter/sqlite/model.rs` のモデルを変えたら、リポジトリのルートでマイグレーションを作り、`crates/core/db/` の変更をまとめてコミットしてください。

```bash
cargo run -p galley-migrate -- migration generate --name add_xxx
```

## ブランチ運用

- 開発は `develop` ブランチで行います。**PR は `develop` 宛てに出してください**
- `main` はリリース用です
- ブランチ名の例：`feat/123-short-desc`、`fix/456-short-desc`、`docs/short-desc`、`chore/short-desc`（数字は Issue 番号）

## コミットメッセージ

```
<type>(<scope>): <説明>
```

- type：`feat`、`fix`、`refactor`、`docs`、`test`、`chore`、`perf`、`ci`
- scope（省略可）：`core`、`web`、`server`、`mcp`、`infra`、`docs`
- 説明は日本語で、1行目は70文字くらいまで

例：

```
feat(core): アップロード時に外部リソースの読み込みを検出する
fix(web): 過去版を表示したときに帯が出ない問題を修正
```

## PR のマージ

- CI（`fmt`、`clippy`、`test`、`architecture`）がすべて通る必要があります
- メンテナーの承認が1件必要です
