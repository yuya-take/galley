# Git Workflow

## リモート連携 CLI

GitHub 前提。リモート操作（PR / Issue / ラベル）は `gh` CLI を使う。

## ブランチ運用

- **developブランチから作成する**
- PRのターゲットブランチはdevelop
- リリース用ブランチ（main等）が分離されている場合は、直接ブランチを切ったりpushしない

### ブランチ命名

```
feat/123-short-desc      # 機能追加（Issue番号付き）
fix/456-short-desc       # バグ修正
refactor/short-desc      # リファクタリング
chore/short-desc         # 設定・CI・依存更新等
docs/short-desc          # ドキュメント
```

## コミットメッセージ

### フォーマット

```
<type>(<scope>): <description>

<optional body>
```

### type

`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

### scope（省略可）

変更対象のクレート/領域名（`core`, `web`, `server`, `mcp`, `infra`, `docs`）

### 例

```
feat(core): アップロード時に外部リソースの読み込みを検出する
fix(web): 過去版を表示したときに帯が出ない問題を修正
refactor(server): 資料配信のヘッダー付与をミドルウェアにまとめる
chore: /explain スキルを追加（コードベース説明用）
docs: READMEにClaude Code開発ガイドを追加
```

### ルール

- 日本語で記述（プロジェクト慣習）
- 1行目は簡潔に（70文字目安）
- `Co-Authored-By` は不要（settings.jsonで無効化済み）

## コミット分割

変更内容に応じて論理的な単位でコミットを分割する:
- 機能追加、リファクタ、バグ修正は別コミット
- 設定変更とコード変更は別コミット
- 関連するファイルはまとめて1コミット

## Pull Request

PR ワークフローは `/pr` スキルで実行する。手順:

1. ブランチ確認・作成（developから）
2. 差分確認（`git status` + `git diff`）
3. コミット分割
4. `git push -u origin ブランチ名`
5. `gh pr create` / `gh pr edit`

### PR Description

```markdown
## 概要
変更内容の要約（1-3行）

## 変更内容
- 具体的な変更点をリスト化

## テスト
- テスト方法・確認事項
```

## 禁止事項

- リリース用ブランチ（main等）への直接push / ブランチ作成
- `--force` push（`--force-with-lease` も確認してから）
- `--no-verify` でのhookスキップ
- `git reset --hard` / `git checkout .` / `git clean -f`（確認なしでの実行）
- `git rebase -i`（インタラクティブモードは非対応）
- 機密ファイル（`.env`, credentials）のコミット
