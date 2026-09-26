---
name: migration-checker
description: Toasty のモデル変更後にマイグレーション漏れを検知する。crates/core/src/adapter 配下のモデル定義が変更された後に使用。
tools: Read, Grep, Glob, Bash
model: haiku
maxTurns: 12
---

# Migration Checker

モデルの変更がマイグレーションに反映されているか検証する。規約は `.claude/rules/database.md`。

## チェック手順

### 1. 変更されたモデルを特定
- `git diff --name-only develop...HEAD` と `git diff --name-only` で変更ファイルを取得
- `crates/core/src/adapter/` 配下で、Toasty のモデル定義（`#[derive(toasty::Model)]` など）を含むファイルを抽出

### 2. 変更内容を分析
- テーブルの新規作成、列の追加・削除・型変更、インデックス・一意制約・外部キーの追加

### 3. マイグレーションを確認
- `crates/core/migrations/` に対応するマイグレーションが追加されているか
- 既存（リリース済み）のマイグレーションを書き換えていないか
- 列の削除・型変更で、既存データの移行が含まれているか
- revisions・blobs の既存行を書き換える内容になっていないか（不変）

### 4. 起動時の適用
- 追加したマイグレーションが起動時の適用対象に含まれているか

## 出力

- 変更されたモデルの一覧
- マイグレーション必要/不要の判定と理由
- 必要な場合の推奨マイグレーション名と内容
