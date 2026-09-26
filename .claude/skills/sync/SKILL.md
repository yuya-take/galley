---
name: sync
description: developブランチに切り替えて最新状態に更新する。Worktree内で実行された場合は状態を判定し、安全ならクリーンアップ、作業中なら対話モードで確認する。
disable-model-invocation: true
---

developブランチを最新化する。Worktree内から実行された場合は状態に応じて処理を分岐する。

## 1. 状態の取得

以下を実行して現在のコンテキストを把握する:

- `git rev-parse --git-common-dir` と `git rev-parse --git-dir` — パスが異なれば補助Worktree
- `git rev-parse --show-toplevel` — 現在のWorktreeルート
- `git branch --show-current` — 現在のブランチ
- `git status --porcelain` — 未コミット変更・未追跡ファイル
- `git rev-parse --abbrev-ref '@{upstream}' 2>/dev/null` — リモート追跡の有無
- 追跡がある場合 `git rev-list --count '@{upstream}..HEAD'` — 未プッシュコミット数

## 2. 分岐

### Case A: メインWorktree（`git-common-dir` と `git-dir` が一致）

そのまま実行:
1. `git checkout develop`（既にdevelopなら省略可）
2. `git pull`
3. 結果を簡潔に報告

### Case B: 補助Worktree

#### B-1: クリーン状態 → 自動クリーンアップ

判定条件（すべて満たす）:
- 未コミット変更・未追跡ファイルなし
- リモート追跡ブランチが存在し、未プッシュコミットが0件

手順:
1. 現在のWorktreeパスとブランチ名を控える
2. ExitWorktreeツール（無ければメインリポジトリのパスへ `cd`）
3. メインから `git worktree remove <パス>` でWorktree削除
4. `git branch -d <ブランチ名>` でブランチ削除
   - 未マージ等で削除に失敗した場合は強制削除せず、そのまま残してユーザーに報告
5. `git checkout develop` → `git pull`
6. 結果を簡潔に報告（削除したWorktree/ブランチ、developの更新）

#### B-2: 作業中 → 対話モードで確認

判定条件（いずれかを満たす）:
- 未コミット変更または未追跡ファイルあり
- 未プッシュコミットあり
- リモート追跡ブランチが未設定（ローカルのみのブランチ）

AskUserQuestionで対話モードに入り、状況を提示してから選択肢を出す。

提示する情報:
- 現在のWorktree: `<パス>`
- ブランチ: `<ブランチ名>`
- 未コミット変更: `<件数>` 件
- 未プッシュコミット: `<件数>` 件（追跡なしの場合は「追跡ブランチなし」）

選択肢:
1. **このWorktreeに留まる**（/syncをキャンセル。デフォルト推奨）
2. **スタッシュしてクリーンアップ** — `git stash push -u -m "sync: <ブランチ名>"` → Worktree削除 → developへ。スタッシュ参照を報告する
3. **強制クリーンアップ** — 変更を破棄してWorktree削除 → developへ。選んだ場合は最終確認をもう一度取ってから `git worktree remove --force` を実行

ユーザーの選択に従って実行する。

## 3. 結果報告

簡潔に以下を伝える:
- 実行したアクション（クリーンアップした場合は削除したパス・ブランチ）
- developの更新内容（`git pull` の取り込みコミット数）
- スタッシュした場合はスタッシュ参照（`git stash list` で確認できる旨）
