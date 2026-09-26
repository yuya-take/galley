---
name: clean-arch-guard
description: Clean Architectureの依存方向違反とユースケース層へのビジネスルール流出を検出する。crates/ 配下の Rust ファイルや Cargo.toml の変更後に使用。use 文・Cargo.toml の解析とユースケース内のロジック検査を行う。
tools: Read, Grep, Glob
model: haiku
maxTurns: 15
---

# Clean Architecture Guard

`Cargo.toml` の依存と `use` 文を解析し、依存方向の違反を検出する。また、ユースケース（`crates/core/src/app`）にビジネスルールが流出していないかを検査する。
ルールの詳細は `.claude/rules/clean-architecture.md`。

## 依存方向ルール

```
許可される依存方向:
  presentation（crates/web、crates/server の http・mcp）→ app → domain ← adapter

禁止される依存:
  domain → app / adapter / presentation
  domain → toasty / object_store / axum / topcoat / rmcp / tokio（I/O・フレームワーク）
  app → adapter（トレイト経由で使う）
  presentation → adapter / toasty（ユースケース経由で使う）
  galley-core → galley-web / galley-server / topcoat
```

例外: `crates/server/src/main.rs`（組み立て）は adapter を生成して app に渡してよい。

## チェック手順

### 1. 変更ファイルの特定
- `git diff --name-only develop...HEAD` と `git diff --name-only` で変更された `crates/**/*.rs` と `Cargo.toml` を取得

### 2. クレート間の依存
- `crates/core/Cargo.toml` の `[dependencies]` に `galley-web`・`galley-server`・`topcoat`・`axum`・`rmcp` がないか

### 3. use 文の解析

**`crates/core/src/domain/` 内のファイル**:
- `use crate::app` / `use crate::adapter` → 違反
- `use toasty` / `use object_store` / `use tokio` / `use axum` → 違反

**`crates/core/src/app/` 内のファイル**:
- `use crate::adapter` → 違反
- `use toasty` / `use object_store` → 違反（DB・ストレージを直接使っている）

**`crates/web/`、`crates/server/src/{http,mcp}/` 内のファイル**:
- `use galley_core::adapter` / `use toasty` → 違反

### 4. ユースケースへのビジネスルール流出

**対象**: `crates/core/src/app/` 配下の変更ファイル

| パターン | 検出方法 | 移動先 |
|---------|---------|--------|
| **ビジネス条件分岐** | サイズ上限、予約語、版番号などを if で直接比較 | 値オブジェクト（`HtmlDocument::parse`、`Slug::parse`） |
| **検査ロジック** | HTML を走査して外部参照を探している | `domain::upload` |
| **採番・計算** | 次の版番号、ハッシュの計算 | `Document` / `BlobHash` のメソッド |
| **パス生成** | `format!` でブロブのパスを組み立て | `BlobHash` のメソッド |
| **閾値・定数** | ユースケース内の `const` | domain の定数 |

#### 判定基準
- **違反**: ユースケース内の分岐がビジネス判定をして、その結果でエラーを返したり値を決めたりしている
- **許容**: リポジトリの戻り値の `None` チェック（`ok_or(AppError::NotFound)`）、トランザクション制御、ログ出力、エラーの変換

### 5. presentation へのビジネスルール流出
- Axum のハンドラー、Topcoat の画面、MCP のツールで、検査・採番・保存順序の制御をしていないか（ユースケースを呼ぶだけになっているか）

## 出力

各違反について以下を報告:

- 違反ファイル・行番号・該当コード
- 違反の種類（クレート依存 / use の依存方向 / ビジネスルール流出）
- 重要度（高/中/低）
- 修正方法の提案（移動先: 値オブジェクト / エンティティのメソッド / domain のトレイト）
