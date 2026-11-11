---
name: rv
description: コードレビューの統合入口。ブランチ差分・未コミット変更・PR・既存フォルダのいずれも対象にでき、変更内容に応じて専門 reviewer エージェントを振り分けて並列実行し、重要指摘を裏取りしてから報告する。「レビューして」「PR 見て」「このフォルダ監査して」などで使用。
argument-hint: "[#PR | PR URL | パス] [--base <branch>] [--staged] [--deep] [--inline] [--fix]"
---

# /rv

引数: `$ARGUMENTS`

チェック項目そのものは各 reviewer エージェントが持っている（定義ファイルは `${CLAUDE_SKILL_DIR}/../../agents/`）。このスキルの役割は「何をレビューするか決める → 適切なエージェントに振り分ける → 結果を裏取り・統合する」こと。

プラグインとして導入されている場合、エージェント名は `review:python-reviewer` のように名前空間付きになる。Agent ツールの `subagent_type` には、利用可能なエージェント一覧に出ている名前をそのまま使う。

## 1. 対象の決定

| 引数 | モード | 差分の取り方 |
|------|--------|--------------|
| なし | ブランチ | `git fetch origin <base>` の後、`git diff $(git merge-base origin/<base> HEAD)` ＝ 分岐点からのコミット済み差分＋未コミット分。未追跡ファイルは `git status --porcelain` で拾って全文を対象にする |
| `--staged` | ステージのみ | `git diff --cached`（コミット直前の確認用） |
| `#123` / `123` / PR URL | PR | `gh pr view <PR> --json title,body,baseRefName,headRefName,files` と `gh pr diff <PR>` |
| ファイル/フォルダのパス | 既存コード監査 | 差分ではなく、配下のソースファイル全体（テスト、生成物、`node_modules`/`.venv`/`target`/`__pycache__`、マイグレーション履歴は除く） |

`<base>` は `--base` 指定 → `develop` → リモートのデフォルトブランチの順で決める。ブランチモードで base 自体の上にいて未コミット変更もない場合は、レビュー対象がない旨を伝えて終了する。

PR モードでは、作業ツリーが PR のコードと一致しているとは限らない。`gh pr view <n> --json headRefOid` の SHA と `git rev-parse HEAD` が一致し、未コミット変更もなければ作業ツリーをそのまま読む。一致しなければ `gh pr checkout` は使わず（作業ツリーを書き換えるため）、`git fetch origin pull/<n>/head` で取得した SHA（`git rev-parse FETCH_HEAD`）から `git show <sha>:<path>` で周辺コードを読む。

## 2. 規模の把握と分類

変更ファイルの一覧と変更行数（`git diff --stat` 相当）を出し、ファイルを下表で分類する。1 ファイルが複数の行に該当してよい。パスの例は既定値で、`/setup` でプロジェクトに合わせて更新される。

| 該当するファイル | 起動するエージェント |
|------------------|----------------------|
| `*.rs`、`Cargo.toml` | `rust-reviewer` |
| `crates/` 配下の Rust ファイル、`crates/*/Cargo.toml` | `clean-arch-guard`（依存の向き、ユースケースへのビジネスルールの流出） |
| `crates/server/src/{app,guard,viewer}.rs`、`crates/server/src/mcp/`、`crates/web/src/` のページ・ルート（`#[page]` `#[layout]` `#[route]`） | `api-spec-reviewer` |
| `crates/core/src/adapter/`、`crates/core/db/`、`*.sql` | `database-reviewer` |
| `crates/core/src/adapter/sqlite/model.rs`（Toasty のモデル） | `migration-checker` |
| アップロード検査（`crates/core/src/domain/upload/`）、資料配信（`crates/server/src/viewer.rs`）、Host・CSRF の検査（`guard.rs`）、MCP、ファイル保存（`blob_store.rs`）、Cookie・入力の受け口、`crates/web/assets/*.js`、依存定義（`Cargo.toml`、`Cargo.lock`）、`Dockerfile`・CI | `security-reviewer` |
| `crates/web/assets/*.css`・`*.js`、画面（`crates/web/src/`） | `code-reviewer`（`.claude/rules/web-standards.md` のデザイントークンとアクセシビリティ） |
| 上のどれにも当たらないコード、または横断的な設計変更 | `code-reviewer` |

ドキュメントや lock ファイルだけの変更は、エージェントを起動せずに目視で確認すれば足りる。

## 3. 静的解析（あれば）

静的解析はここで 1 回だけ実行し、結果をエージェントに渡す（各エージェントに個別に実行させると、同じツールが何度も走るため）。プロジェクトにインストール・設定済みのものだけを、変更されたファイルを中心に実行する。ファイルを書き換えるオプション（`--fix`、`--write`）は使わない。

- Python: `ruff check`、`ruff format --check`、型チェッカー（`ty check` / `mypy` / `pyright` のうち設定されているもの）
- TypeScript: `biome check` または `eslint`、`tsc --noEmit`。`npx` は `npx --no-install` で実行する（未インストールだと同名の別パッケージを取得してしまうため）
- Rust: `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`（CI と同じ）

Makefile や package.json に lint / 型チェックのターゲットがあれば、そちらを優先する。PR モードで作業ツリーが PR のコードと一致しない場合は実行しない（別のコードを解析することになるため）。

ツールの警告をそのまま並べるのではなく、変更箇所に関係する意味のあるものだけを指摘の材料にする。

## 4. レビューの実行

規模に応じて実行方法を変える。サブエージェントの起動コストと要約による情報の損失より、並列化の利益が大きいときだけ分割する。

- **小（変更 ~300 行以内かつ該当エージェント 2 つ以下）**: サブエージェントを使わず、自分で該当エージェントの定義ファイル（`${CLAUDE_SKILL_DIR}/../../agents/<name>.md`）を読み、そのチェック観点で直接レビューする
- **中〜大**: 該当エージェントを Agent ツールで並列起動する。同じエージェントをファイル単位やディレクトリ単位で複数に分けない（起動ごとに文脈の読み直しが発生し、結果の統合も重くなるため）
- **`--inline`**: 規模に関係なく、サブエージェントを使わずに小と同じ方法でレビューする（`/dev` のセルフレビュー用）
- **`--deep`**: 規模に関係なく該当エージェントをすべて並列起動し、`security-reviewer`・`code-reviewer`・`refactor-cleaner` を常に加える。`refactor-cleaner` の結果（SAFE / CAREFUL / RISKY）は重大度付きの指摘とは混ぜず、レポート末尾の「整理候補」にまとめる。汎用的なバグ検出も厚くしたい場合は、組み込みの `/code-review` を併用するようユーザーに案内する

各エージェントには次を渡す:

- 担当するファイルの一覧と、差分の取り方（ステップ 1 のコマンド。PR なら PR 番号）
- ファイルの読み方（作業ツリーをそのまま読むか、`git show <sha>:<path>` で読むか）
- 変更の目的（PR のタイトル・本文、`/dev` から呼ばれた場合は Issue の要件と受け入れ条件）
- 静的解析の結果のうち、そのファイルに関係するもの。「静的解析は実施済み（または実行不可）なので再実行しない」と明記する
- 既存コード監査モードの場合は、差分ではなくファイル全体が対象であること（エージェント定義の「変更されていないコードは報告しない」を適用しない）
- この段階の役割は網羅であり、確信が持てない指摘や重大度の低い指摘も確信度を付けてすべて返すこと（絞り込みはステップ 5 で行う）
- 出力はエージェント定義にある統一フォーマット（`[重大度] path:line — 問題 → 修正案 (確信度)`）で返すこと

自分でも、エージェントの担当にない横断的な観点を確認する:

- 変更の目的に対して、実装が過不足ないか（要件漏れ、スコープ外の変更の混入）
- デバッグコード、コメントアウトされたコード、不要な import、新規の TODO/FIXME が残っていないか
- 振る舞いの変更に対してテストが追加・更新されているか
- 公開 API・DB スキーマ・設定の変更が、呼び出し元や他サービスに与える影響

## 5. 裏取りと統合

エージェントは網羅を優先して指摘を返すので、誤検知や重大度の過大評価が混じる。絞り込みはここで行う。

1. **重複の統合**: 同じ箇所・同じ原因の指摘は 1 件にまとめ、重い方の重大度を採用する
2. **CRITICAL / HIGH の裏取り**: 指摘箇所と、その前後・呼び出し元を実際に読み、問題が本当に成立するか確かめる。例えば「SQL インジェクション」なら、入力がどこから来て、途中でバインドやバリデーションされていないかを追う。成立しなければ捨て、条件付きで成立するなら重大度を下げて条件を書く
3. **規約との照合**: 「規約違反」とされたものは、`.claude/rules/` に実際にその規約があるかを確認する。規約にない好みの問題は MEDIUM 以下に下げる
4. 確信度 low の指摘は、指摘箇所を読んで成立するかを確かめる。成立すれば確信度を上げて残し、確かめられなければ捨てる
5. 裏取りしていない確信度 medium の MEDIUM は、「要確認」の印を付けて残す

## 6. レポート

```
## レビュー結果 — <ブランチ名 / PR #n タイトル / フォルダパス>

対象: N ファイル (+X / -Y)  ｜ 実行: python-reviewer, security-reviewer, ...  ｜ 静的解析: ruff ✓ / tsc 2 件

### CRITICAL（マージ前に必須）
- `path:line` — 問題の説明
  → 修正案

### HIGH（修正推奨）
- ...

### MEDIUM（提案）
- ...

### 判定
CRITICAL/HIGH あり → 修正が必要 ｜ MEDIUM 以下のみ → マージ可
```

指摘のない重大度の見出しは省く。全体で指摘がなければ「指摘なし」と、何を確認したかを 1〜2 行で書く。

## 7. 後続アクション

- **`--fix`**（ブランチ / ステージモードのみ。PR や監査モードでは使わない）: CRITICAL と HIGH を修正し、ステップ 3 の静的解析とテストを再実行して結果を報告する。MEDIUM は修正せず、一覧として残す
- **PR モード**: レポートを PR コメント用に整形した下書きを提示し、ユーザーの承認を得てから投稿する
  - 1 回のレビューとしてまとめて投稿する: `gh api repos/{owner}/{repo}/pulls/<n>/reviews --input <json>`（`commit_id` は head の SHA、`event` は既定で `COMMENT`、`body` に総評、`comments[]` に `path` / `line` / `side: "RIGHT"` / `body`）
  - インラインにできるのは差分に含まれる行だけ（それ以外を指定すると API がエラーを返す）。差分外の指摘は `body` に入れる
  - `APPROVE` / `REQUEST_CHANGES` は、ユーザーが指示した場合だけ使う
  - 文中で `#数字` を章番号の意味で使わない（別の Issue/PR に自動リンクされるため）
- **PR の再レビュー**: 著者が修正を push した後に同じ PR で `/rv` を実行したら、`gh api repos/{owner}/{repo}/pulls/<n>/reviews` と `.../pulls/<n>/comments` で前回のレビューと指摘を取得し、前回レビュー時の `commit_id` からの差分（`git diff <前回SHA> <現在SHA>`）を中心に確認する。結果は解消済み／未解消／新規の 3 区分で報告する
