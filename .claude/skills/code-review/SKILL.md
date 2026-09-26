---
name: code-review
description: 未コミットの変更に対してセキュリティ・品質・規約のレビューを実行する。
---

# Code Review

未コミットの変更を対象にセキュリティと品質のレビューを実行する。

## 手順

### 1. 変更ファイルの取得

```bash
git diff --name-only HEAD
git diff --name-only --cached
```

変更がない場合はその旨を報告して終了する。

### 2. 各ファイルをレビュー

変更されたファイルを Read で読み、以下の観点でチェックする。

---

### セキュリティ（CRITICAL）

- [ ] ハードコードされた認証情報、APIキー、トークンがない（`MCP_TOKEN` などは環境変数から読む）
- [ ] 資料の HTML がアプリ本体と別オリジン（資料配信ポート / 資料用ホスト名）からしか配信されない
- [ ] 資料配信のレスポンスに CSP・`X-Content-Type-Options: nosniff`・`Content-Type: text/html; charset=utf-8` が必ず付く
- [ ] 資料を埋め込む iframe が `sandbox="allow-scripts"` のみ（`allow-same-origin` などを足していない）
- [ ] アップロード検査（UTF-8、サイズ上限、外部リソース検出）が Web UI と MCP の両方で `galley-core` の同じ処理を通る
- [ ] 状態を変えるリクエストで Origin・独自ヘッダー・`ALLOWED_HOSTS` の検査を迂回していない
- [ ] SQL を文字列結合で組み立てていない
- [ ] ブロブのパスなど、ファイルパスに外部入力を直接使っていない（パストラバーサル）
- [ ] 資料の内容や `MCP_TOKEN` をログに出していない

### 品質（HIGH）

- [ ] `unwrap()` / `expect()` がプロダクションコードで使われていない（`?` 演算子を使用）
- [ ] `unsafe` ブロックがない（必要な場合は `// SAFETY:` で正当な理由があること）
- [ ] エラー型が適切に定義されている（`galley-core` は `thiserror`、起動処理は `anyhow`）
- [ ] 不要な `clone()` がない
- [ ] async の中でブロッキング処理（同期のファイル I/O など）をしていない
- [ ] `println!` / `dbg!` マクロが残っていない（`tracing` を使用）
- [ ] 関数が50行、ファイルが800行を超えていない

### 規約準拠（MEDIUM）

- [ ] `.claude/rules/rust-standards.md` に従う
- [ ] `galley-core` が Topcoat や `galley-web` に依存していない
- [ ] 依存のバージョンをルートの `[workspace.dependencies]` で管理している
- [ ] TODO/FIXME コメントが新たに追加されていない

---

### 3. レポート生成

以下のフォーマットで報告する:

```
## レビュー結果

### CRITICAL（必ず修正）
- `ファイルパス:行番号` — 問題の説明
  → 修正案

### HIGH（修正推奨）
- `ファイルパス:行番号` — 問題の説明
  → 修正案

### MEDIUM（提案）
- `ファイルパス:行番号` — 問題の説明
  → 修正案

### 総合判定
- CRITICAL/HIGHがある場合: コミット前に修正を推奨
- MEDIUM以下のみ: コミットOK
```

## 注意事項

- 変更されたファイルのみをレビュー対象にする（リポジトリ全体ではない）
- 指摘には必ずファイルパスと行番号を含める
- CRITICALまたはHIGHの指摘がある場合はコミット前の修正を強く推奨する
- `.claude/rules/` の規約も参照して整合性を確認する
