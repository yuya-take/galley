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

### AI から登録する（MCP）

Claude Code などの MCP クライアントから、資料の一覧・取得・登録・更新ができます。更新者名は `X-Author-Name` ヘッダーで渡します（登録した資料の更新者として残ります）。

```bash
claude mcp add --transport http galley http://galley.example.com:8080/mcp \
  --header "X-Author-Name: 佐藤"
```

- サーバーに `MCP_TOKEN` を設定したときは、`--header "Authorization: Bearer <トークン>"` も付けます
- ほかの PC から使うときは、サーバーの `ALLOWED_HOSTS` にホスト名（上の例なら `galley.example.com`）を入れます
- 登録できるのは、外部リソース（CDN のスクリプト、Web フォント、外部の画像など）を読み込まない1ファイルの HTML だけです

| ツール | 内容 |
| --- | --- |
| `list_projects` | プロジェクトの一覧 |
| `list_documents` | プロジェクト内の資料の一覧（資料名、最新の版番号、URL） |
| `get_document` | 資料の情報と、指定した版（`revision`。省略時は最新）の HTML。過去の版なら URL も `/v/<番号>` 付き |
| `create_document` | 新しい資料を第1版として登録する（`slug` は省略可） |
| `update_document` | 既存の資料に新しい版を追加する（`message` に何を変えたか） |

## ドキュメント

- [設計書](docs/design.md)：目的、データモデル、システム構成、セキュリティ、画面、MCP
- [コントリビューションガイド](CONTRIBUTING.md)：開発環境、ブランチ運用、PR の出し方
- [セキュリティポリシー](SECURITY.md)：脆弱性の報告方法

## 技術スタック

Rust（[Axum](https://github.com/tokio-rs/axum)、[Topcoat](https://github.com/tokio-rs/topcoat)、[rmcp](https://crates.io/crates/rmcp)、[Toasty](https://github.com/tokio-rs/toasty)）、SQLite

## ライセンス

[MIT](LICENSE)
