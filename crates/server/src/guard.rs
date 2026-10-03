//! 外部のサイトからブラウザー経由で送られるリクエストへの対策。
//!
//! - DNS リバインディング：Host ヘッダーを `ALLOWED_HOSTS` と照合する（アプリ・資料配信・MCP のすべて）
//! - CSRF：状態を変えるリクエストは、Origin がアプリ自身のものだけ受け付け、POST は JSON に限る
//!
//! JSON の POST は CORS の単純リクエストではないので、ブラウザーは別のサイトから送る前に
//! preflight を行い、許可しなければ送らない。Topcoat の画面の操作（手続きの呼び出しなど）は
//! 独自ヘッダーを付けずに JSON の POST で送るので、独自ヘッダーは求めない。

use std::{fmt, sync::Arc};

use galley_web::host::hostname;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// 受け付けるホスト名（ポートは見ない）。小文字で持ち、IPv6 は `[::1]` の形。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedHosts {
    hosts: Vec<String>,
    /// `ALLOWED_HOSTS` が未設定で、localhost だけを受け付けている。
    is_default: bool,
}

/// `ALLOWED_HOSTS` が未設定のときに受け付けるホスト名。
const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "ALLOWED_HOSTS の「{0}」はホスト名として読めません（カンマ区切りで galley.example.com のように書く）"
)]
pub struct AllowedHostsError(String);

impl AllowedHosts {
    /// `ALLOWED_HOSTS`（カンマ区切り）を読む。空なら localhost だけにする。
    /// ポートが付いていれば無視する（`galley.internal:8080` → `galley.internal`）。
    pub fn parse(value: Option<&str>) -> Result<Self, AllowedHostsError> {
        let entries: Vec<&str> = value
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .collect();
        if entries.is_empty() {
            return Ok(Self {
                hosts: LOCAL_HOSTS.iter().map(|h| (*h).to_owned()).collect(),
                is_default: true,
            });
        }
        let hosts = entries
            .into_iter()
            .map(|entry| hostname(entry).ok_or_else(|| AllowedHostsError(entry.to_owned())))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            hosts,
            is_default: false,
        })
    }

    /// ホスト名を加える（`GALLEY_VIEWER_URL` のホスト名など）。
    #[must_use]
    pub fn with(mut self, host: &str) -> Self {
        if let Some(host) = hostname(host)
            && !self.hosts.contains(&host)
        {
            self.hosts.push(host);
        }
        self
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Host ヘッダー（`host[:port]`）を受け付けるか。
    pub fn allows(&self, host_header: &str) -> bool {
        hostname(host_header).is_some_and(|host| self.hosts.contains(&host))
    }

    /// rmcp の `allowed_hosts` に渡す形（IPv6 は角括弧を外す）。
    pub fn for_rmcp(&self) -> Vec<String> {
        self.hosts
            .iter()
            .map(|h| h.trim_start_matches('[').trim_end_matches(']').to_owned())
            .collect()
    }
}

impl fmt::Display for AllowedHosts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hosts.join(", "))
    }
}

/// リクエストの Host（HTTP/2 では `:authority`）。Host ヘッダーが複数あれば `None`
/// （前にプロキシを置いたとき、どちらを見るかが食い違わないように）。
fn request_host(request: &Request) -> Option<&str> {
    let mut hosts = request.headers().get_all(header::HOST).iter();
    match (hosts.next(), hosts.next()) {
        (Some(host), None) => host.to_str().ok(),
        (None, _) => request.uri().authority().map(|a| a.as_str()),
        (Some(_), Some(_)) => None,
    }
}

/// Host ヘッダーが `ALLOWED_HOSTS` に無ければ 403 を返す。
pub async fn check_host(
    State(allowed): State<Arc<AllowedHosts>>,
    request: Request,
    next: Next,
) -> Response {
    match request_host(&request) {
        Some(host) if allowed.allows(host) => next.run(request).await,
        host => {
            tracing::warn!(
                host = host.unwrap_or("（なし）"),
                "許可していないホスト名へのリクエストを拒否しました"
            );
            forbidden(
                "このホスト名ではアクセスできません。管理者は ALLOWED_HOSTS を確認してください",
            )
        }
    }
}

/// 状態を変えるリクエスト（GET・HEAD・OPTIONS 以外）を、アプリ自身からのものに限る。
pub async fn check_csrf(request: Request, next: Next) -> Response {
    if let Err(reason) = verify_same_origin(&request) {
        tracing::warn!(method = %request.method(), path = request.uri().path(), reason, "別のサイトからのリクエストを拒否しました");
        return forbidden("このリクエストは受け付けられません");
    }
    next.run(request).await
}

fn verify_same_origin(request: &Request) -> Result<(), &'static str> {
    let method = request.method();
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let headers = request.headers();
    // Origin があれば（ブラウザーからなら必ずある）、アプリ自身のものに限る
    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin = origin.to_str().map_err(|_| "Origin を読めない")?;
        let host = request_host(request).ok_or("Host がない")?;
        if !is_same_origin(origin, host) {
            return Err("Origin がアプリ自身ではない");
        }
    }
    if headers
        .get("sec-fetch-site")
        .is_some_and(|site| site != "same-origin" && site != "none")
    {
        return Err("Sec-Fetch-Site が same-origin ではない");
    }
    // フォームから送れる POST（単純リクエスト）を受け付けない。PUT・DELETE などは単純リクエストに
    // ならず、ブラウザーは必ず preflight を行う（CORS を許可していないので送られない）ため、POST だけ見る。
    // CORS のヘッダーを返すようにするときは、ここを見直す
    if *method == Method::POST && !is_json(headers) {
        return Err("POST が JSON ではない");
    }
    Ok(())
}

/// `Origin`（`scheme://host[:port]`）が、リクエストの Host と同じか。既定のポートは省いて比べる。
fn is_same_origin(origin: &str, host: &str) -> bool {
    let origin = origin.to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    let authority = if let Some(rest) = origin.strip_prefix("http://") {
        rest.strip_suffix(":80").unwrap_or(rest)
    } else if let Some(rest) = origin.strip_prefix("https://") {
        rest.strip_suffix(":443").unwrap_or(rest)
    } else {
        // `null`（sandbox の iframe やファイルから）など
        return false;
    };
    let host = host
        .strip_suffix(":80")
        .or_else(|| host.strip_suffix(":443"))
        .unwrap_or(&host);
    authority == host
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| {
            let mime = mime.trim();
            mime.eq_ignore_ascii_case("application/json")
        })
}

/// MCP のトークン（環境変数 `MCP_TOKEN`）。設定したときだけ `Authorization: Bearer` を求める。
#[derive(Clone)]
pub struct McpToken(Option<String>);

impl McpToken {
    pub fn from_env(value: Option<String>) -> Self {
        Self(value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()))
    }

    pub fn is_required(&self) -> bool {
        self.0.is_some()
    }

    fn accepts(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = &self.0 else {
            return true;
        };
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|given| constant_time_eq(given.trim().as_bytes(), expected.as_bytes()))
    }
}

impl fmt::Debug for McpToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // トークンをログに出さない
        f.write_str(if self.is_required() {
            "McpToken(****)"
        } else {
            "McpToken(None)"
        })
    }
}

/// 比べる時間から一致した長さを推測されないよう、すべてのバイトを比べる。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// `MCP_TOKEN` を設定していれば、`Authorization: Bearer <トークン>` が無いリクエストを 401 で断る。
pub async fn check_mcp_token(
    State(token): State<Arc<McpToken>>,
    request: Request,
    next: Next,
) -> Response {
    if token.accepts(request.headers()) {
        return next.run(request).await;
    }
    (
        StatusCode::UNAUTHORIZED,
        [
            (header::WWW_AUTHENTICATE, "Bearer"),
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
        ],
        "MCP のトークンが違います。接続設定の Authorization: Bearer を確認してください",
    )
        .into_response()
}

fn forbidden(message: &'static str) -> Response {
    (
        StatusCode::FORBIDDEN,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        message,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, middleware, routing::get};
    use http::Request as HttpRequest;
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn default_allows_only_localhost() {
        let hosts = AllowedHosts::parse(None).unwrap_or_else(|e| panic!("{e}"));
        assert!(hosts.is_default());
        for host in [
            "localhost",
            "localhost:8080",
            "127.0.0.1:8081",
            "[::1]:8080",
            "LOCALHOST",
        ] {
            assert!(hosts.allows(host), "{host}");
        }
        for host in [
            "galley.internal",
            "192.168.1.10:8080",
            "evil.example.com",
            "",
            "localhost.evil.com",
            "127.0.0.1.nip.io",
        ] {
            assert!(!hosts.allows(host), "{host}");
        }
    }

    #[test]
    fn parse_reads_comma_separated_hosts() {
        let hosts = AllowedHosts::parse(Some(" Galley.Internal , 192.168.1.10:8080,[::1] "))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!hosts.is_default());
        assert!(hosts.allows("galley.internal:8080"));
        assert!(hosts.allows("192.168.1.10"));
        assert!(hosts.allows("[::1]:8081"));
        assert!(!hosts.allows("localhost"));
        assert_eq!(
            hosts.for_rmcp(),
            vec!["galley.internal", "192.168.1.10", "::1"]
        );
        assert_eq!(
            AllowedHosts::parse(Some(" , ")).map(|h| h.is_default()),
            Ok(true)
        );
        assert!(AllowedHosts::parse(Some("galley.internal/x")).is_err());
        assert!(AllowedHosts::parse(Some("a b")).is_err());
        assert!(AllowedHosts::parse(Some("host:99999")).is_err());
    }

    #[test]
    fn with_adds_viewer_host() {
        let hosts = AllowedHosts::parse(Some("galley.internal"))
            .unwrap_or_else(|e| panic!("{e}"))
            .with("galley-docs.internal");
        assert!(hosts.allows("galley-docs.internal:443"));
    }

    #[test]
    fn same_origin_ignores_default_ports() {
        assert!(is_same_origin("http://localhost:8080", "localhost:8080"));
        assert!(is_same_origin(
            "https://galley.example.com",
            "galley.example.com"
        ));
        assert!(is_same_origin(
            "https://galley.example.com:443",
            "galley.example.com"
        ));
        assert!(is_same_origin(
            "http://galley.example.com",
            "galley.example.com:80"
        ));
        assert!(!is_same_origin("http://localhost:8081", "localhost:8080"));
        assert!(!is_same_origin("http://evil.example.com", "localhost:8080"));
        assert!(!is_same_origin("null", "localhost:8080"));
        assert!(!is_same_origin(
            "http://localhost:8080.evil.com",
            "localhost:8080"
        ));
    }

    fn app() -> Router {
        let allowed = Arc::new(AllowedHosts::parse(None).unwrap_or_else(|e| panic!("{e}")));
        Router::new()
            .route(
                "/",
                get(|| async { "ok" })
                    .post(|| async { "ok" })
                    .delete(|| async { "ok" }),
            )
            .layer(middleware::from_fn(check_csrf))
            .layer(middleware::from_fn_with_state(allowed, check_host))
    }

    async fn status(request: HttpRequest<Body>) -> StatusCode {
        match app().oneshot(request).await {
            Ok(response) => response.status(),
            Err(never) => match never {},
        }
    }

    fn request(method: &str, headers: &[(&str, &str)]) -> HttpRequest<Body> {
        let mut builder = HttpRequest::builder().method(method).uri("/");
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder
            .body(Body::empty())
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[tokio::test]
    async fn rejects_unknown_hosts() {
        assert_eq!(
            status(request("GET", &[("host", "localhost:8080")])).await,
            StatusCode::OK
        );
        assert_eq!(
            status(request("GET", &[("host", "evil.example.com")])).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(status(request("GET", &[])).await, StatusCode::FORBIDDEN);
        let duplicated = [("host", "localhost:8080"), ("host", "evil.example.com")];
        assert_eq!(
            status(request("GET", &duplicated)).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn accepts_same_origin_json_post() {
        let headers = [
            ("host", "localhost:8080"),
            ("origin", "http://localhost:8080"),
            ("content-type", "application/json; charset=utf-8"),
            ("sec-fetch-site", "same-origin"),
        ];
        assert_eq!(status(request("POST", &headers)).await, StatusCode::OK);
        // curl や MCP のクライアントは Origin を付けない
        let headers = [
            ("host", "localhost:8080"),
            ("content-type", "application/json"),
        ];
        assert_eq!(status(request("POST", &headers)).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_cross_site_requests() {
        let cases: [&[(&str, &str)]; 6] = [
            // 別のサイトからの JSON（preflight を通ったとしても Origin で断る）
            &[
                ("host", "localhost:8080"),
                ("origin", "http://evil.example.com"),
                ("content-type", "application/json"),
            ],
            // 資料配信（sandbox の iframe）から
            &[
                ("host", "localhost:8080"),
                ("origin", "null"),
                ("content-type", "application/json"),
            ],
            &[
                ("host", "localhost:8080"),
                ("origin", "http://localhost:8081"),
                ("content-type", "application/json"),
            ],
            // フォームから送れる POST
            &[
                ("host", "localhost:8080"),
                ("content-type", "application/x-www-form-urlencoded"),
            ],
            &[("host", "localhost:8080"), ("content-type", "text/plain")],
            &[
                ("host", "localhost:8080"),
                ("content-type", "application/json"),
                ("sec-fetch-site", "cross-site"),
            ],
        ];
        for headers in cases {
            assert_eq!(
                status(request("POST", headers)).await,
                StatusCode::FORBIDDEN,
                "{headers:?}"
            );
        }
        assert_eq!(
            status(request(
                "DELETE",
                &[
                    ("host", "localhost:8080"),
                    ("origin", "http://evil.example.com")
                ]
            ))
            .await,
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn mcp_token_is_optional_and_compared_exactly() {
        let headers = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::AUTHORIZATION,
                value.parse().unwrap_or_else(|e| panic!("{e}")),
            );
            headers
        };
        let none = McpToken::from_env(None);
        assert!(none.accepts(&HeaderMap::new()));
        assert!(!McpToken::from_env(Some("  ".to_owned())).is_required());

        let token = McpToken::from_env(Some("s3cret".to_owned()));
        assert!(token.accepts(&headers("Bearer s3cret")));
        assert!(!token.accepts(&HeaderMap::new()));
        for value in [
            "Bearer s3cre",
            "Bearer s3cret2",
            "Basic s3cret",
            "s3cret",
            "Bearer ",
        ] {
            assert!(!token.accepts(&headers(value)), "{value}");
        }
        assert_eq!(format!("{token:?}"), "McpToken(****)");
    }

    #[tokio::test]
    async fn allows_delete_without_origin_for_mcp_clients() {
        assert_eq!(
            status(request("DELETE", &[("host", "localhost:8080")])).await,
            StatusCode::OK
        );
    }
}
