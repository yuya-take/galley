---
name: code-review-full
description: 現在のブランチのdevelopからの全コミット差分に対してセキュリティ・品質・規約のレビューを実行する。
---

# Code Review Full（ブランチ全体レビュー）

現在のブランチのdevelopからの全コミット差分を対象に、セキュリティと品質のレビューを実行する。
未コミットの変更も含めてレビューする。

## 手順

### 1. ブランチ情報の確認

```bash
# 現在のブランチ名を確認
git branch --show-current

# developとの分岐点（merge-base）を取得
git merge-base develop HEAD
```

現在のブランチがdevelopの場合は、ブランチレビューの対象外である旨を報告して終了する。

### 2. 変更ファイルの取得

```bash
# developからの全コミット差分のファイル一覧
git diff --name-only $(git merge-base develop HEAD)..HEAD

# 未コミットの変更も含める
git diff --name-only HEAD
git diff --name-only --cached
```

変更がない場合はその旨を報告して終了する。

### 3. コミット履歴の確認

```bash
# developからの全コミットを確認（レビュー対象の変更の全体像を把握する）
git log --oneline $(git merge-base develop HEAD)..HEAD
```

### 4. 差分の確認

```bash
# developからの統合差分を取得（個別コミットではなく最終的な差分を見る）
git diff $(git merge-base develop HEAD)..HEAD
```

差分が大きい場合はファイル単位で分割して確認する:

```bash
git diff $(git merge-base develop HEAD)..HEAD -- <file_path>
```

未コミットの変更がある場合はそちらも確認する:

```bash
git diff HEAD
git diff --cached
```

### 5. 各ファイルをレビュー

変更されたファイルを Read で読み、差分の内容を踏まえて以下の観点でチェックする。

**重要**: 個別コミットの差分ではなく、developからの最終的な差分全体を見てレビューする。途中のコミットで追加されて後のコミットで削除されたコードはレビュー対象外。

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

### 設計・アーキテクチャ（MEDIUM）

- [ ] 新規ファイルが適切なクレート（core / web / server）に置かれている
- [ ] 不要なファイルが追加されていない（デバッグ用ファイル、一時ファイル等）
- [ ] コミット間で一貫性のある設計になっている（途中で方針変更していないか）

---

### 6. レポート生成

以下のフォーマットで報告する:

```
## ブランチレビュー結果

**ブランチ**: `ブランチ名`
**対象コミット数**: N件
**変更ファイル数**: N件

### コミット一覧
- `hash` メッセージ
- ...

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
- CRITICAL/HIGHがある場合: マージ前に修正を推奨
- MEDIUM以下のみ: マージOK
```

## 注意事項

- developからの全差分をレビュー対象にする（未コミットの変更も含む）
- 個別コミットの途中経過ではなく、最終的な差分をレビューする
- 指摘には必ずファイルパスと行番号を含める
- CRITICALまたはHIGHの指摘がある場合はマージ前の修正を強く推奨する
- `.claude/rules/` の規約も参照して整合性を確認する
- 差分が大きい場合は、code-reviewer agentを並列起動してファイルグループごとにレビューする
