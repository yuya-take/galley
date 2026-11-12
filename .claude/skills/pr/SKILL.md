---
name: pr
description: 変更をコミットに分割してプッシュし、Pull Request を作成または更新する。ユーザーが PR の作成・更新を求めたとき、または他のスキルの手順で指示されたときに使う。
argument-hint: "[--base <branch>] [--draft]"
---

# /pr

引数: `$ARGUMENTS`

ベースブランチは `--base` 指定 → `develop` → リモートのデフォルトブランチの順で決める。

## 1. ブランチ

`git branch --show-current` で確認する。ベースブランチ（またはリリース用ブランチ）上にいる場合は、変更内容から `.claude/rules/git-workflow.md` の命名に沿ったブランチ名を決めて `git switch -c <branch>` する。直接プッシュはしない。

## 2. 差分の把握

`git status` と `git diff`（未追跡ファイルも含む）で変更を把握する。`.env` や認証情報らしきファイル、デバッグ用の一時ファイルが含まれていたら、コミット前にユーザーに確認する。

このブランチでまだ `/rv` を通していない、かつ変更が大きい場合は、先に `/rv` を実行するか確認する。

## 3. コミット

論理的な単位で分割する（機能追加・リファクタ・バグ修正・設定変更は別コミット）。メッセージは `.claude/rules/git-workflow.md` の形式（`<type>(<scope>): <説明>`）に従う。既にコミット済みのものは作り直さない。

コミットメッセージにも PR の本文にも、Claude のセッションの URL（`Claude-Session:` の行、`https://claude.ai/code/session_…` へのリンク）は入れない。リポジトリの読者には開けないリンクで、公開リポジトリに残す必要が無いため。

## 4. プッシュ

`git push -u origin <branch>`

## 5. PR の作成・更新

`gh pr list --head <branch> --state open` で既存 PR を確認する。

- **無い場合**: `gh pr create --base <base>`（`--draft` 指定時は付ける）。リポジトリに `.github/pull_request_template.md` があればその形式に、無ければ次の形式で本文を書く。Issue 由来のブランチなら `Closes #<番号>` を入れる

  ```markdown
  ## 概要
  何を・なぜ（1〜3 行）

  ## 変更内容
  - 主な変更点

  ## テスト
  - 実行した検証と確認方法
  ```

- **ある場合**: `gh pr view` で現在の説明を確認し、今回の変更で内容が古くなっていれば `gh pr edit` で更新する

最後に PR の URL を報告する。
