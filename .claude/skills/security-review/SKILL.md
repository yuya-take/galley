---
name: security-review
description: Galley のセキュリティチェックリスト。アップロード検査、資料配信（別オリジン・sandbox・CSP）、CSRF/DNSリバインディング対策、MCP、ファイル保存を変更するときに使用。
---

# Security Review Skill

Galley はログインを持たず、AI が生成した任意の HTML（スクリプトを含む）を保存・表示する。
コード変更がこの前提で安全かを確認する。詳しいチェック項目は `security-reviewer` agent と同じ。

## 対象となる変更

- アップロード検査（`galley-core`）
- 資料配信のルーターとヘッダー
- 資料を埋め込む iframe
- 状態を変えるリクエストのルート、Host/Origin 検査
- MCP のツールと認証
- ブロブ保存と DB 更新

## 脅威モデル

| 脅威 | 経路 | 対策 |
|------|------|------|
| 資料内スクリプトによるアプリ操作 | 資料をアプリと同じオリジンで表示 | 別オリジン配信 + `sandbox="allow-scripts"` |
| 資料からの情報送信・外部読み込み | fetch、画像、CDN | 資料配信の CSP（`default-src 'none'`、`connect-src 'none'`） |
| 外部サイトからの CSRF | 社員が悪意あるサイトを開く | Origin 検査 + 独自ヘッダー付き JSON のみ受け付け |
| DNS リバインディング | 攻撃者のドメインを社内 IP に向ける | Host ヘッダーを `ALLOWED_HOSTS` と照合 |
| パストラバーサル | ブロブのパスに外部入力 | パスはハッシュからだけ作る |
| 巨大ファイルによる枯渇 | アップロード / MCP | 読み込み前のサイズ上限 |

CSP では iframe 自身が別 URL へ遷移すること（URL にデータを載せる送り出し）までは防げない。これは既知の制限として受け入れている。

## チェックリスト

### 1. 資料の隔離

```rust
// NEVER: アプリ側のルーターで資料を返す
app_router.route("/r/{id}", get(serve_revision));

// ALWAYS: 資料配信のサーバー（別ポート / 別ホスト名）で返し、全レスポンスにヘッダーを付ける
let viewer = Router::new()
    .route("/r/{id}", get(serve_revision))
    .layer(SetResponseHeaderLayer::overriding(CONTENT_SECURITY_POLICY, csp()))
    .layer(SetResponseHeaderLayer::overriding(X_CONTENT_TYPE_OPTIONS, nosniff()));
```

- [ ] 資料 HTML はアプリと別オリジンからだけ返る
- [ ] エラー時も含めて CSP・nosniff・Content-Type が付く
- [ ] iframe は `sandbox="allow-scripts"` のみ

### 2. リクエスト検査

- [ ] 状態を変えるルートはすべて Origin・独自ヘッダー・JSON の検査を通る
- [ ] すべてのサーバー（アプリ・資料配信・MCP）で Host を `ALLOWED_HOSTS` と照合する
- [ ] GET で状態を変えない

### 3. アップロード検査

- [ ] Web UI と MCP が `galley-core` の同じ関数を呼ぶ
- [ ] サイズ上限を読み込み前に適用する
- [ ] 外部リソースの検出にテストがある（`srcset`、style 属性の `url()`、`@import`、`<base>`、`<meta http-equiv="refresh">` など）

### 4. 保存

- [ ] ブロブを書いてから DB を更新し、リビジョン作成と現在版の更新は1トランザクション
- [ ] リビジョンは更新・削除しない

### 5. シークレットとログ

- [ ] `MCP_TOKEN` は環境変数から読み、定数時間で比較する
- [ ] 資料本文・トークンをログに出さない

## リリース前チェック

- [ ] `cargo audit` で既知の脆弱性がない
- [ ] 上の脅威モデルの対策にそれぞれテストがある
- [ ] Docker イメージが非 root で動く
- [ ] README に「インターネットに公開しない前提」と `ALLOWED_HOSTS` の設定方法が書かれている
