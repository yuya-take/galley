---
name: setup
description: 導入した pack の rules / settings を .claude/ に展開し、プロジェクトの構成（言語・パス・テストコマンド・ブランチ運用）に合わせてカスタマイズする。導入直後、プラグイン更新後、プロジェクト構成が変わったときに実行。
disable-model-invocation: true
---

# /setup

導入方法（install.sh かプラグインか）に関係なく、このスキル 1 つで「展開 → 環境チェック → プロジェクトに合わせた調整」まで行う。再実行しても安全なように、既存の内容は確認なしに上書きしない。

## 1. rules / settings の展開（プラグイン導入時のみ）

`.claude/.yuya-claude-code.json`（install.sh の導入記録）がある場合は、install.sh で展開済みなのでこのステップを飛ばす。pack の追加・削除は、install.sh を `--packs` 付きで再実行すれば行える（`--list` で一覧と現在の選択を表示できる）。

プラグインとして導入した場合、rules と permissions はプラグインの仕組みでは配布されないので、install.sh の rules と settings だけを展開するモードで入れる。

1. `claude plugin list` で、このリポジトリ（marketplace `yuya-claude-code`）から入れている pack を確認する（`all` を入れている場合は全 pack）
2. marketplace の複製（`~/.claude/plugins/marketplaces/yuya-claude-code/`。`CLAUDE_CODE_PLUGIN_CACHE_DIR` を設定している場合はその配下）にある install.sh を、プロジェクトのルートで実行する:

   ```bash
   bash ~/.claude/plugins/marketplaces/yuya-claude-code/install.sh --only rules,settings --packs <確認した pack をカンマ区切りで>
   ```

   複製が見つからない場合は、`curl -fsSL https://raw.githubusercontent.com/yuya-take/yuya-claude-code/main/install.sh | bash -s -- --only rules,settings --packs ...` で実行する
3. 出力にスキップ（手元で変更済み）が出た場合は、差分の概要を示し、`--force` で上書きするかユーザーに確認する

## 2. 環境チェック

結果は問題の有無にかかわらずまとめて報告する。

- **リモート**: `git remote get-url origin` が GitHub でなければ、`/pr`・`/rv`（PR モード）・`/issue-create` などのリモート連携は動かないと伝える
- **gh CLI**: `gh auth status`。未インストールなら https://cli.github.com/ 、未認証なら `gh auth login` を案内する
- **ベースブランチ**: `develop` が存在するか確認する。無い場合は、デフォルトブランチ（`git symbolic-ref refs/remotes/origin/HEAD`）を使う運用かどうかをユーザーに確認し、ステップ 5 で `rules/git-workflow.md` を書き換える
- **Worktree**: `/dev` は `.claude/worktrees/` に Worktree を作る
  - `.gitignore` に `.claude/worktrees/` が無ければ追加を提案する
  - テストや起動に必要な gitignore 済みファイル（`.env` など）があれば、リポジトリルートに `.worktreeinclude`（`.gitignore` と同じ書式）を作って列挙するよう提案する
  - `.claude/` の agents / skills / rules が git にコミットされていないと、Worktree 側には存在しない。コミットされていなければ、コミットを提案する（`settings.local.json` は除く）

## 3. プロジェクト構造の解析

- 言語・ツール: `pyproject.toml`（ruff / ty / mypy / pytest / uv）、`package.json`（biome / eslint / tsc / テストランナー）、`Cargo.toml`、`orval.config.*`、`Makefile` のターゲット
- サービス構成: モノレポなら各サービスのディレクトリと、backend / frontend の分け方
- Clean Architecture のレイヤーディレクトリ（`domain/`, `application/` or `usecase/`, `adapter/`, `presentation/`）の実際のパス
- API ルート、DB モデル、マイグレーション（`alembic/`, `migrations/`）、リポジトリ層の場所
- テスト・lint・format・型チェックのコマンド（サービスごと）
- ログライブラリ（`structlog`, `logging`, `tracing` など）

## 4. 確認

解析結果を次の形で示し、反映してよいか確認する。

```
## プロジェクト解析結果
- 言語: Python / TypeScript / Rust
- ベースブランチ: develop
- サービス: backend → backend/ , frontend → frontend/
- レイヤー: backend/app/{domain,usecase,adapter,presentation}
- API ルート: backend/app/presentation/route/
- DB: モデル backend/app/adapter/model/ , マイグレーション backend/alembic/
- コマンド: lint `make lint` / test `make test` / frontend `npm run biome`, `npm test`
- Orval: あり（frontend/src/api/generated/）
```

## 5. 反映

承認後、`.claude/` 配下を Read → Edit で更新する（一括 `sed` は使わない。ファイルごとに文脈が違うため）。ユーザーが手で編集した箇所は残し、変える前に差分を確認する。

- **rules の `paths:`**: `clean-architecture.md`、`database.md`、`frontend-standards.md`、`naming-conventions.md`、`orval.md`、`python-standards.md`、`rust-standards.md` の frontmatter を実際のディレクトリに合わせる。使わない言語の rules は削除を提案する。`paths:` の無い rules（`agents.md`, `git-workflow.md`）は常時適用なので触らない
- **rules 本文の例示**: サンプルパスやコマンドを実際のものに置き換える。ベースブランチが develop でない場合は `git-workflow.md` を書き換える
- **`/rv` の振り分け表**: `skills/rv/SKILL.md` のステップ 2 の表のパス例を、実際のレイヤー・API・DB のパスに合わせる（install.sh で展開した場合のみ。プラグインのスキルは編集しない）
- **agents**（install.sh で導入した場合のみ）: `test-runner.md` のテストコマンド表、各 reviewer の対象パスを実態に合わせる
- **troubleshooting スキル**（python pack を install.sh で導入した場合のみ）: `skills/troubleshooting/SKILL.md` の Makefile ターゲット・環境変数・ポート番号の例を実際のものに置き換える。対応するものがなければ削除を提案する
- **settings.json**: `permissions.allow` を、プロジェクトで実際に使う lint / test / build コマンドに合わせる。使わないツールのエントリは削除を提案する

## 6. 完了報告

更新したファイルと変更点の要約を報告し、まず試すものとして `/dev`、`/rv`、`/pr` を案内する。
