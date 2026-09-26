# Galley

AI が生成した HTML 資料を、プロジェクト単位で保存・閲覧・バージョン管理できるセルフホスト型の Web アプリです。
社内ネットワークに置いて、ログインなしでチームの誰でも資料を登録・閲覧できます。

> [!WARNING]
> 開発中です。まだ動くリリースはありません。進み具合は [MVP マイルストーン](https://github.com/yuya-take/galley/milestone/2) を見てください。

## 特徴

- **資料を1か所に集める**：AI に作らせた HTML 資料を、プロジェクトごとに保存して一覧できます
- **版を重ねる**：更新するたびに「第N版」として残り、過去の版を見たり、その版に戻したりできます
- **安全に表示する**：資料はアプリと別のオリジンで、sandbox 付きの iframe と CSP で隔離して表示します。資料内のスクリプトがアプリを操作したり、外部に通信したりできません
- **AI から直接保存する**：MCP サーバーを内蔵していて、Claude Code などの AI エージェントから資料を作成・更新できます
- **Docker イメージ1つで動く**：DB は SQLite、資料はボリューム上のファイルなので、ほかのサーバーは要りません

資料は「外部リソースを読み込まない、1ファイルで完結した HTML」に限ります。CSS と JavaScript は HTML 内に書き、画像は data URI で埋め込んでください。

## 使い方（予定）

```bash
docker run -d -p 8080:8080 -p 8081:8081 -v galley-data:/data ghcr.io/yuya-take/galley
```

- `:8080`：アプリ本体（画面と MCP の `/mcp`）
- `:8081`：資料の配信（アプリと別オリジンにするため、ポートを分けています）

インターネットには公開せず、社内ネットワークで使う前提です。環境変数などの詳しい使い方は、リリースに合わせて書きます。

## ドキュメント

- [設計書](docs/design.md)：目的、データモデル、システム構成、セキュリティ、画面、MCP
- [コントリビューションガイド](CONTRIBUTING.md)：開発環境、ブランチ運用、PR の出し方
- [セキュリティポリシー](SECURITY.md)：脆弱性の報告方法

## 技術スタック

Rust（[Axum](https://github.com/tokio-rs/axum)、[Topcoat](https://github.com/tokio-rs/topcoat)、[rmcp](https://crates.io/crates/rmcp)、[Toasty](https://github.com/tokio-rs/toasty)）、SQLite

## ライセンス

[MIT](LICENSE)
