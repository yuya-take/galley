---
paths:
  - "crates/**/*.rs"
---

# Naming Conventions

Rust の標準（[RFC 430](https://rust-lang.github.io/rfcs/0430-finalizing-naming-conventions.html)、[API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html)）に従う。

- **モジュール・関数・変数**: `snake_case`
- **型・トレイト・enum のバリアント**: `UpperCamelCase`
- **定数・static**: `SCREAMING_SNAKE_CASE`
- **コレクション**: 複数形（`documents`、`revisions`）。`_list` は付けない
- **真偽値**: `is_` / `has_` で始める（`is_archived`、`has_external_resources`）
- **変換**: `as_`（コストなし）、`to_`（コストあり）、`into_`（所有権を消費）、`from_` / `From` 実装
- **ゲッター**: `get_` を付けない（`document.title()`）
- **エラー型**: `〜Error`（`UploadError`、`CoreError`）
- **リポジトリ**: トレイトは `〜Repository`（domain）、実装は `Toasty〜Repository`（adapter）
- **ストレージ**: トレイトは `BlobStore`（domain）、実装は `ObjectStoreBlobStore`（adapter）
- **ユースケース**: 動詞で始める構造体（`RegisterRevision`、`RevertToRevision`）に `execute` メソッド

## 用語

コードは英語、画面は日本語で、次の対応を崩さない。

| コード | 画面 | 意味 |
|--------|------|------|
| project | プロジェクト | 資料をまとめる入れ物 |
| document | 資料 | リビジョンの積み重ね |
| revision | 版（第N版） | 不変の1版 |
| blob | — | HTML ファイルの実体 |
| archive | アーカイブ | 論理削除 |
| author_name | 更新者 | 自己申告の名前 |
