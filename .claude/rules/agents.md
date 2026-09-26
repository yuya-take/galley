# Agent Orchestration

## Available Agents

| Agent | 用途 | トリガー |
|-------|------|---------|
| **code-reviewer** | コード変更全般のレビュー | コード変更完了後 |
| **rust-reviewer** | 所有権・安全性・非同期パターン | Rustファイル変更後 |
| **clean-arch-guard** | 依存方向違反・ユースケースへのビジネスルール流出の検出 | `crates/` 配下の変更後 |
| **security-reviewer** | セキュリティ脆弱性検出 | アップロード検査・資料配信・CSP・CSRF/Host検査・MCPの変更後 |
| **api-spec-reviewer** | 画面の URL・HTTP ルート・資料配信・MCP ツールの一貫性 | `crates/server` の http・mcp、`crates/web` のルート変更後 |
| **database-reviewer** | SQLite のスキーマ・クエリ・トランザクション | `crates/core/src/adapter`・`crates/core/db` の変更後 |
| **migration-checker** | マイグレーション漏れ検知 | Toasty のモデル変更後 |
| **test-runner** | テスト実行と結果分析 | 実装完了後 |
| **refactor-cleaner** | デッドコード・重複検出 | コード整理時 |

## プロアクティブに使用するタイミング

ユーザーの指示がなくても、以下の状況では該当agentを自動的に使用する:

| 状況 | 使用するagent |
|------|-------------|
| Rustコードを書いた/修正した | **rust-reviewer** + **clean-arch-guard** |
| HTTP ルート・MCP ツール・画面の URL を追加/変更した | **api-spec-reviewer** + **security-reviewer** |
| DBモデル/マイグレーションを変更した | **database-reviewer** + **migration-checker** |
| アップロード検査・資料配信・ヘッダー・リクエスト検査を変更した | **security-reviewer** |
| 実装が完了した | **test-runner** |
| リファクタリングを行った | **refactor-cleaner** |

## 並列実行

独立したagentは必ず並列で起動する:

```
# GOOD: 並列実行（core の変更後）
同時に起動:
  1. rust-reviewer — コード品質チェック
  2. clean-arch-guard — 依存方向チェック
  3. security-reviewer — セキュリティチェック

# BAD: 順次実行（不要な待ち時間）
rust-reviewer → 完了を待つ → clean-arch-guard → 完了を待つ → security-reviewer
```

### 変更内容別の並列パターン

**core（domain / app）の変更:**
- rust-reviewer + clean-arch-guard + security-reviewer（並列）
- → test-runner（上記完了後）

**HTTP ルート・MCP ツールの変更:**
- api-spec-reviewer + security-reviewer + clean-arch-guard（並列）
- → test-runner（上記完了後）

**DBモデル/マイグレーションの変更:**
- database-reviewer + migration-checker（並列）
- → test-runner（上記完了後）

**画面（crates/web）の変更:**
- rust-reviewer + clean-arch-guard（並列）
- → test-runner（上記完了後）

**大規模リファクタリング:**
- refactor-cleaner + clean-arch-guard + rust-reviewer（並列）
- → test-runner（上記完了後）
