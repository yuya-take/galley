---
paths:
  - "crates/**/*.rs"
  - "crates/*/Cargo.toml"
---

# Clean Architecture

Galley は依存の向きを内側（ドメイン）に揃える。Topcoat は実験段階で破壊的変更が前提なので、画面の変更がドメインに波及しない構成にする。

```
┌──────────────────────────────────────────────────────┐
│ presentation                                         │
│ - crates/web              Topcoat の画面             │
│ - crates/server/src/http  Axum のハンドラー、資料配信 │
│ - crates/server/src/mcp   MCP のツール               │
├──────────────────────────────────────────────────────┤
│ crates/core/src/app       ユースケース（登録、戻す等）│
├──────────────────────────────────────────────────────┤
│ crates/core/src/domain    エンティティ、値オブジェクト、│
│                           アップロード検査、リポジトリ │
│                           とストレージのトレイト      │
├──────────────────────────────────────────────────────┤
│ crates/core/src/adapter   Toasty（SQLite）と          │
│                           object_store の実装         │
└──────────────────────────────────────────────────────┘
crates/server/src/main.rs   組み立て（adapter を生成して app に渡す）
```

## ディレクトリ構成

domain と app は集約（プロジェクト・資料・版など）ごとにフォルダを分け、ユースケースは1ファイルに1つ置く。各フォルダの `mod.rs` で `pub use` して、呼び出し側は `domain::project::ProjectSlug`、`app::project::CreateProject` のように1階層で使う。

```
crates/core/src/
├── domain/
│   ├── shared/        複数の集約で使うもの（ID、slug と文字列の共通の検査、ArchiveFilter）
│   ├── project/       mod.rs（エンティティ）、slug.rs、crest_color.rs、text.rs、repository.rs
│   ├── document/      mod.rs、slug.rs、title.rs、query.rs（一覧の条件）、repository.rs
│   ├── revision/      mod.rs、text.rs（更新者名、変更メモ）、repository.rs
│   ├── blob/          mod.rs（BlobHash、BlobContent）、store.rs（BlobStore）
│   ├── upload/        mod.rs（HtmlDocument、UploadError）、scan.rs（HTML）、css.rs、script.rs、url.rs、resource.rs
│   └── error.rs       RepositoryError、StorageError
├── app/
│   ├── error.rs       AppError
│   ├── project/       create.rs、update.rs、archive.rs、find.rs、list.rs
│   ├── document/      create.rs（第1版の登録）、list.rs、find.rs、choose_slug.rs、rename.rs、archive.rs
│   └── revision/      add.rs（新しい版）、revert.rs（この版に戻す）
└── adapter/
    ├── blob_store.rs  ObjectStoreBlobStore（ローカル・S3 互換・メモリ）
    └── sqlite/        model.rs（Toasty のモデル）、集約ごとのリポジトリ、convert.rs、raw.rs
```

- 新しい集約は domain と app に同じ名前のフォルダを作る（例：アップロード検査は `domain/upload/`、版の登録は `app/revision/register.rs`）
- ユースケースのファイル名は動詞（`create.rs`、`archive.rs`）。同じ集約のユースケースで共有する処理（`load` など）はその集約の `mod.rs` に置く
- 1つの集約でしか使わない値オブジェクトは、その集約のフォルダに置く。`shared/` には2つ以上の集約で使うものだけ置く

## 依存方向のルール

- **依存の向き**: adapter → domain ← app ← presentation（常に内側へ）
- **domain**: I/O をしない。`toasty`・`object_store`・`axum`・`topcoat`・`rmcp`・`tokio` に依存しない
  - 許可: `serde`、`uuid`、`thiserror`、`sha2`、`base64`、HTML・CSS の解析（`lol_html`、`cssparser`、`htmlize`）など、純粋な計算のクレート
- **app**: domain のトレイト経由でだけ DB とストレージを使う。`adapter` を直接使わない
- **presentation**（web、server の http・mcp）: `app` のユースケースを呼ぶだけ。`adapter` や `toasty` を直接使わない
- **組み立て**: `crates/server/src/main.rs` だけが adapter を生成し、app に注入してよい
- **クレート間**: `galley-core` は `galley-web`・`galley-server`・Topcoat に依存しない

## app（ユースケース）の責務

オーケストレーション専用。以下のみ許可:
1. リポジトリ・ストレージのトレイトの呼び出し
2. domain のメソッドへの委譲
3. トランザクション制御（ブロブを書いてから、リビジョン作成と現在版の更新を1トランザクションで行う）
4. エラーの変換

### app に書いてはいけないもの（domain へ移す）

| 禁止パターン | Galley での例 | 移動先 |
|-------------|--------------|--------|
| **ビジネス条件分岐** | `if html.len() > MAX_SIZE` / 予約語の slug 判定 | domain の値オブジェクト（`Slug::parse`、`HtmlDocument::parse`） |
| **検査ロジック** | 外部リソースの検出 | `domain::upload` |
| **採番** | 次の版番号の計算 | `Revision` / `Document` のメソッド |
| **パス生成** | `/data/blobs/ab/cd/<hash>.html` の組み立て | `BlobHash` のメソッド |
| **閾値・定数** | サイズ上限、予約語の一覧 | domain の定数 |
| **ハッシュ計算** | SHA-256 | `BlobHash::of(&[u8])` |

```rust
// ❌ BAD: ユースケースでビジネス判定
impl RegisterRevision {
    pub async fn execute(&self, input: Input) -> Result<Output, AppError> {
        if input.html.len() > 10 * 1024 * 1024 {
            return Err(AppError::TooLarge);
        }
        // ...
    }
}

// ✅ GOOD: 値オブジェクトに委譲
impl RegisterRevision {
    pub async fn execute(&self, input: Input) -> Result<Output, AppError> {
        let html = HtmlDocument::parse(input.html)?; // サイズ・文字コード・外部参照を検査
        let hash = BlobHash::of(html.as_bytes());
        self.blobs.put(&hash, html.as_bytes()).await?;
        self.documents.add_revision(input.document_id, &hash, input.meta).await?;
        // ...
    }
}
```

## 型の変換

- presentation の入出力型（serde の Request / Response、MCP ツールの引数）は presentation に置き、`app` の入出力型に変換して渡す
- adapter のモデル（Toasty の型）は adapter の外に出さず、domain の型に変換して返す
- 変換は `From` / `TryFrom` で書く
