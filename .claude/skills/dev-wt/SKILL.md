---
name: dev-wt
description: IssueリンクからWorktreeを作成し、Planモードで設計後に実装する開発ワークフロー。Issue駆動開発に使用。
disable-model-invocation: true
argument-hint: "[Issue URL or #number]"
effort: medium
---

Issueの内容を確認し、Worktreeで隔離された環境で設計・実装を行います。

## 1. Issue情報の取得

引数: `$ARGUMENTS`

- `gh issue view <number> --comments` でIssue情報（タイトル、本文、ラベル、コメント）を取得する。URLの場合はそのまま渡す
- Issue内容を把握し、実装スコープを明確にする

## 2. ブランチ名の決定

Issue情報からブランチ名を生成する:
- フォーマット: `<type>/<issue-number>-<short-description>`
- type: Issueのラベルや内容から判断
  - `feat/` — 新機能
  - `bug/` — バグ修正（Issueがバグ報告の場合）
  - `fix/` — 軽微な修正・改善
  - `refactor/` — リファクタリング
  - `docs/` — ドキュメント
  - `chore/` — その他メンテナンス
- 例: `feat/123-add-user-auth`, `bug/456-connection-timeout`
- ブランチ名は英語、kebab-case、50文字以内

## 3. Worktreeの作成

EnterWorktreeツールを使ってWorktreeを作成する:
- **developブランチをベースにする**
- ステップ2で決めたブランチ名を使用する

EnterWorktree呼び出し時のパラメータ例:
- `branch`: `feat/123-add-user-auth`
- `base_branch`: `develop`（必須。省略すると意図しないブランチから切られるため注意）

EnterWorktreeが使えない場合のコマンド例:
```bash
git worktree add .claude/worktrees/feat/123-add-user-auth develop
```

## 4〜7: 設計・実装・レビュー・報告

以下は `/dev-bc` と共通のワークフロー。

### 4. Planモードで設計

EnterPlanModeツールを使ってPlanモードに入る。

> **🧭 幅広い探索はサブエージェントに委譲する（context 肥大化対策）**
> **3 クエリを超える幅広い探索や調査**は、Agent ツール（`subagent_type=Explore`）に委譲する。Issue 要件を渡して「関連ファイルの場所・該当箇所・既存実装の要点」だけサマリーで返してもらう。サブエージェントの探索ログは親 context に入らず要約だけが戻るため、`/compact` と同等のコンテキスト節約効果が得られる（`/compact` はビルトイン機能で Claude からは発火できないため、この方式で代替する）。
> 一方、**ピンポイントな確認（既知パスの 1〜2 ファイルを読む、特定シンボルを grep する程度）は直接 Read/Grep/Glob してよい**。subagent の起動コストと要約損失のほうが大きいため。
> 探索が複数領域にまたがる場合は Agent を並列で複数起動してよい。

サマリーを受け取ったら、以下を実施する:
- 変更が必要なファイル・箇所を特定し、実装計画を作成する
- ユーザーに計画を提示し、承認を得る
- **承認後** ExitPlanModeで通常モードに戻る

### 5. 実装

承認された計画に基づいて実装する:
- 既存のコード規約（`.claude/rules/`）に従う
- テストがある場合はテストも更新・追加する
- `cargo fmt --all` と `cargo clippy --workspace --all-targets -- -D warnings` を実行する

### 6. セルフレビュー

セルフレビューもサブエージェントに委譲する。Agent ツール（`subagent_type=general-purpose`）を呼び、以下を渡して「指摘事項のみを 200 語以内で返す」よう指示する:

- `git diff` の出力（または変更ファイル一覧と各ファイルの変更概要）
- Issue 要件のサマリー
- チェック項目:
  - [ ] Issueの要件をすべて満たしているか
  - [ ] 不要な変更（デバッグコード、不要なimport等）が含まれていないか
  - [ ] 命名規則・コード規約（`.claude/rules/`）に準拠しているか
  - [ ] エラーハンドリング・セキュリティが適切か
  - [ ] テストが十分か（`cargo test --workspace` で確認）

実装中の中間出力をサブエージェントは見ないため、レビュー作業自体は別 context で走り、親には指摘リストだけが戻る。指摘があれば親側で修正してからユーザーに報告する。

### 7. 完了報告

変更ファイル一覧、実装サマリー、セルフレビュー結果、次のステップの提案（コミット、PR作成など）を報告する。
