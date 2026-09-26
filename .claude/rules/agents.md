# Agent Orchestration

## Available Agents

| Agent | 用途 | トリガー |
|-------|------|---------|
| **code-reviewer** | コード変更全般のレビュー | コード変更完了後 |
| **rust-reviewer** | 所有権・安全性・非同期パターン | Rustファイル変更後 |
| **security-reviewer** | セキュリティ脆弱性検出 | アップロード検査・資料配信・CSP・CSRF/Host検査・MCPの変更後 |
| **test-runner** | テスト実行と結果分析 | 実装完了後 |
| **refactor-cleaner** | デッドコード・重複検出 | コード整理時 |

## プロアクティブに使用するタイミング

ユーザーの指示がなくても、以下の状況では該当agentを自動的に使用する:

| 状況 | 使用するagent |
|------|-------------|
| Rustコードを書いた/修正した | **rust-reviewer** |
| アップロード検査・資料配信・ヘッダー・リクエスト検査・MCPを変更した | **security-reviewer** |
| 実装が完了した | **test-runner** |
| リファクタリングを行った | **refactor-cleaner** |

## 並列実行

独立したagentは必ず並列で起動する:

**Rust変更:**
- rust-reviewer + security-reviewer（並列）
- → test-runner（上記完了後）

**大規模リファクタリング:**
- refactor-cleaner + rust-reviewer（並列）
- → test-runner（上記完了後）
