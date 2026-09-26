---
paths:
  - "crates/core/src/adapter/**"
  - "crates/core/migrations/**"
---

# Database Standards

## 構成

- **DB**: SQLite（WAL モード）1ファイル。`/data` ボリュームに置く
- **ORM**: Toasty
- **資料の実体**: DB には入れず、`object_store` でファイル（既定は `/data/blobs/ab/cd/<hash>.html`）として保存する。DB はハッシュだけ持つ
- 書き込みは直列になるが、資料の更新頻度なら問題にならない前提

## テーブル

| テーブル | 主な列 | メモ |
| --- | --- | --- |
| projects | id, slug, name, description, color, archived_at, created_at | slug は URL に使う。予約語（`archive`、`connect`、`api`、`mcp`、`settings` など）は使えない。color は紋章の色（red / blue / green / gold / purple / silver） |
| documents | id, project_id, slug, title, current_revision_id, archived_at, created_at, updated_at | current_revision_id が「現在版」のポインタ。(project_id, slug) は一意 |
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
- **外部キー**: `PRAGMA foreign_keys = ON` で有効にする

## 保存の順序

1. ブロブをストレージに書く（同じハッシュがあれば書かない）
2. リビジョンの作成と `current_revision_id` の更新を **1トランザクション** で行う

この順序なら「DB にあるのに実体がない」状態にはならない。参照されないブロブは残りうるので、掃除ジョブで消す。

## マイグレーション

- 置き場所と実行方法は #6 で決め、決まったらここに追記する
- マイグレーションは起動時に自動で適用する（Docker イメージ1つで動かすため）
- 一度リリースしたマイグレーションは書き換えない。変更は新しいマイグレーションで行う
- 列の削除・型変更は、既存データの移行を同じマイグレーションに含める
