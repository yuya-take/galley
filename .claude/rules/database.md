---
paths:
  - "crates/core/src/adapter/**"
  - "crates/core/db/**"
---

# Database Standards

## 構成

- **DB**: SQLite（WAL モード）1ファイル。`/data` ボリュームに置く
- **ORM**: Toasty
- **資料の実体**: DB には入れず、`object_store` でファイル（既定は `/data/blobs/ab/cd/<hash>.html`、置き場所は `BlobHash::storage_key`）として保存する。DB はハッシュだけ持つ
- 書き込みは直列になるが、資料の更新頻度なら問題にならない前提

## テーブル

| テーブル | 主な列 | メモ |
| --- | --- | --- |
| projects | id, slug, name, description, color, archived_at, created_at | slug は URL に使う。予約語（`RESERVED_PROJECT_SLUGS`：`archive`、`connect`、`api`、`mcp`、`settings` など）は使えない。color は紋章の色（red / blue / green / gold / purple / silver） |
| documents | id, project_id, slug, title, current_revision_id, archived_at, created_at, updated_at | current_revision_id が「現在版」のポインタ。(project_id, slug) は一意。slug の予約語は `settings`（`RESERVED_DOCUMENT_SLUGS`）。updated_at は版を追加したときだけ変わる |
| revisions | id, document_id, number, blob_hash, message, author_name, source, restored_from_number, created_at | 作成後は変更しない。(document_id, number) は一意。restored_from_number は「この版に戻す」で作った版だけが持つ |
| blobs | hash, size, created_at | 実体はファイル |

slug は英小文字・数字・ハイフンだけ。資料の slug はファイル名から作る（`docs/design.md` の「slug の決め方」）。

ユーザーや権限のテーブルは持たない。

## パターン

- **主キー**: UUID v7（時系列でソートできる）
- **論理削除**: `archived_at`（NULL なら有効）。projects と documents だけ。物理削除はしない
- **不変**: revisions と blobs は INSERT のみ。UPDATE / DELETE しない
- **過去版に戻す**: 過去版と同じ blob_hash を指す新しいリビジョンを作り、`restored_from_number` に元の版番号を入れる（履歴を書き換えない）
- **時刻**: UTC で保存する
- **外部キー**: Toasty は外部キー制約を作らず、接続ごとの PRAGMA（`foreign_keys` など）も設定できない。参照の整合はユースケースで保つ（存在を確かめてから作る、物理削除しない）
- **WAL**: `SqliteDatabase::open` で `PRAGMA journal_mode = WAL` を1回実行する（DB ファイルに記録される）。`busy_timeout` は rusqlite の既定（5秒）

## 保存の順序

1. ブロブをストレージに書く（同じハッシュがあれば書かない）
2. リビジョンの作成と `current_revision_id` の更新を **1トランザクション** で行う

この順序なら「DB にあるのに実体がない」状態にはならない。参照されないブロブは残りうるので、掃除ジョブで消す。

## マイグレーション

- 置き場所は `crates/core/db/`（`history.toml`、`migrations/*.sql`、`snapshots/*.toml`）。設定はリポジトリのルートの `Toasty.toml`
- モデル（`crates/core/src/adapter/sqlite/model.rs`）を変えたら、リポジトリのルートで `cargo run -p galley-migrate -- migration generate --name <内容>` を実行し、できた SQL を確認して3種類のファイルをまとめてコミットする
- マイグレーションは `toasty::embed_migrations!` でバイナリに埋め込み、起動時に自動で適用する（Docker イメージ1つで動かすため）。適用済みかどうかは `__toasty_migrations` テーブルで判定する
- 一度リリースしたマイグレーションは書き換えない。変更は新しいマイグレーションで行う
- 列の削除・型変更は、既存データの移行を同じマイグレーションに含める

## Toasty の注意点（0.11）

- 結合・`EXISTS`・集計が要るクエリは `toasty::sql::query` の生の SQL で書く。値は必ずプレースホルダー（`?1`、`?2` …）で渡し、文字列に埋め込まない
- 生の SQL で `Uuid` や `Timestamp` を渡したり型を指定して受け取ったりすると、SQLite ドライバーがエラーではなくパニック（`todo!()`）になる。`adapter/sqlite/raw.rs` の変換を使い、UUID はバイト列、日時は文字列（小数9桁固定の RFC 3339）で扱う
- 一意制約の違反は Toasty が区別しないので、SQLite のメッセージ（`UNIQUE constraint failed`）で `RepositoryError::Conflict` に変換する
