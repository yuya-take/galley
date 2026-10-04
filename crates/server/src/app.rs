//! アプリ本体（:8080）のルーターの組み立て。画面（Topcoat）と `/mcp` を載せ、
//! 外側に Host の検査と CSRF 対策を置く。

use std::{convert::Infallible, sync::Arc};

use axum::{Router, extract::Request, middleware, response::IntoResponse};
use galley_core::{
    app::{
        document::{CreateDocument, FindDocument, ListDocuments},
        project::{FindProject, ListProjects},
        revision::{AddRevision, FindRevision, ReadRevisionContent},
    },
    domain::{
        blob::BlobStore, document::DocumentRepository, project::ProjectRepository,
        revision::RevisionRepository, upload::MAX_HTML_BYTES,
    },
};
use galley_web::AppUrl;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tower::{Service, ServiceBuilder};
use tower_http::limit::RequestBodyLimitLayer;

use crate::{
    guard::{self, AllowedHosts, McpToken},
    mcp::{GalleyMcp, McpApp},
};

/// MCP のリクエストの大きさの上限。HTML の上限に、JSON のエスケープで増える分の余裕を持たせる。
const MCP_BODY_LIMIT: usize = MAX_HTML_BYTES * 2 + 1024 * 1024;

/// ユースケースに渡すリポジトリとストレージ。実装（adapter）は `main.rs` で作る。
#[derive(Clone)]
pub struct Ports {
    pub projects: Arc<dyn ProjectRepository>,
    pub documents: Arc<dyn DocumentRepository>,
    pub revisions: Arc<dyn RevisionRepository>,
    pub blobs: Arc<dyn BlobStore>,
}

/// MCP から使うユースケースを組み立てる。
pub fn mcp_app(ports: Ports, app_url: AppUrl) -> Arc<McpApp> {
    let Ports {
        projects,
        documents,
        revisions,
        blobs,
    } = ports;
    Arc::new(McpApp {
        list_projects: ListProjects::new(projects.clone()),
        find_project: FindProject::new(projects.clone()),
        list_documents: ListDocuments::new(documents.clone()),
        find_document: FindDocument::new(documents.clone()),
        find_revision: FindRevision::new(revisions.clone()),
        read_revision: ReadRevisionContent::new(revisions.clone(), blobs.clone()),
        create_document: CreateDocument::new(projects, documents.clone(), blobs.clone()),
        add_revision: AddRevision::new(documents, revisions, blobs),
        app_url,
    })
}

/// `/mcp` 以外はすべて画面（`web`）に渡す。Host の検査を一番外に置く。
pub fn app_router<W>(
    web: W,
    mcp_app: Arc<McpApp>,
    allowed_hosts: &Arc<AllowedHosts>,
    mcp_token: Arc<McpToken>,
) -> Router
where
    W: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
    W::Response: IntoResponse,
    W::Future: Send + 'static,
{
    let mcp = StreamableHttpService::new(
        move || Ok(GalleyMcp::new(mcp_app.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_allowed_hosts(allowed_hosts.for_rmcp()),
    );
    let mcp = ServiceBuilder::new()
        .layer(middleware::from_fn_with_state(
            mcp_token,
            guard::check_mcp_token,
        ))
        .layer(RequestBodyLimitLayer::new(MCP_BODY_LIMIT))
        .service(mcp);
    Router::new()
        .nest_service("/mcp", mcp)
        .fallback_service(web)
        .layer(middleware::from_fn(guard::check_csrf))
        .layer(middleware::from_fn_with_state(
            allowed_hosts.clone(),
            guard::check_host,
        ))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use axum::{body::Body, http::StatusCode};
    use galley_core::{
        adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
        app::project::{CreateProject, ProjectInput},
    };
    use http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    struct Mcp {
        router: Router,
        session: Option<String>,
    }

    impl Mcp {
        async fn new(token: Option<&str>) -> TestResult<Self> {
            let database = SqliteDatabase::in_memory().await?;
            CreateProject::new(Arc::new(database.projects()))
                .execute(&ProjectInput {
                    slug: "sales".to_owned(),
                    name: "営業部".to_owned(),
                    description: "営業の資料".to_owned(),
                    color: "blue".to_owned(),
                })
                .await?;
            let web = tower::service_fn(|_: Request| async {
                Ok::<_, Infallible>(StatusCode::NOT_FOUND)
            });
            let router = app_router(
                web,
                mcp_app(
                    Ports {
                        projects: Arc::new(database.projects()),
                        documents: Arc::new(database.documents()),
                        revisions: Arc::new(database.revisions()),
                        blobs: Arc::new(ObjectStoreBlobStore::in_memory()),
                    },
                    AppUrl::FromRequest,
                ),
                &Arc::new(AllowedHosts::parse(None)?),
                Arc::new(McpToken::from_env(token.map(str::to_owned))),
            );
            Ok(Self {
                router,
                session: None,
            })
        }

        async fn post(
            &self,
            body: &Value,
            headers: &[(&str, &str)],
        ) -> TestResult<(StatusCode, http::HeaderMap, String)> {
            let mut request = HttpRequest::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "localhost:8080")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream");
            if let Some(session) = &self.session {
                request = request.header("mcp-session-id", session);
            }
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            let bytes = serde_json::to_vec(body)?;
            let response = self
                .router
                .clone()
                .oneshot(
                    request
                        .header("content-length", bytes.len())
                        .body(Body::from(bytes))?,
                )
                .await?;
            let status = response.status();
            let headers = response.headers().clone();
            let body =
                String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?;
            Ok((status, headers, body))
        }
    }

    impl Mcp {
        /// SSE の `data:` から JSON-RPC の応答を取り出す。
        fn message(body: &str) -> TestResult<Value> {
            let data = body
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .find(|data| !data.trim().is_empty())
                .ok_or("応答に data がない")?;
            Ok(serde_json::from_str(data)?)
        }

        async fn connect(token: Option<&str>) -> TestResult<Self> {
            let mut mcp = Self::new(token).await?;
            let auth = token.map(|t| format!("Bearer {t}"));
            let headers: Vec<(&str, &str)> =
                auth.iter().map(|a| ("authorization", a.as_str())).collect();
            let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}});
            let (status, response_headers, body) = mcp.post(&init, &headers).await?;
            assert_eq!(status, StatusCode::OK, "{body}");
            let info = Self::message(&body)?;
            assert_eq!(info["result"]["serverInfo"]["name"], "galley");
            mcp.session = response_headers
                .get("mcp-session-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let (status, _, _) = mcp
                .post(
                    &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    &headers,
                )
                .await?;
            assert_eq!(status, StatusCode::ACCEPTED);
            Ok(mcp)
        }

        /// ツールを呼び、結果のテキストとエラーかどうかを返す。
        async fn call(
            &self,
            tool: &str,
            arguments: Value,
            headers: &[(&str, &str)],
        ) -> TestResult<(String, bool)> {
            let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":tool,"arguments":arguments}});
            let (status, _, body) = self.post(&request, headers).await?;
            assert_eq!(status, StatusCode::OK, "{body}");
            let message = Self::message(&body)?;
            let result = &message["result"];
            let text = result["content"][0]["text"]
                .as_str()
                .ok_or_else(|| format!("テキストがない: {message}"))?;
            Ok((text.to_owned(), result["isError"] == true))
        }
    }

    const AUTHOR: (&str, &str) = ("x-author-name", "佐藤");
    const HTML_V1: &str = "<!doctype html><title>計画</title><p>第1版";
    const HTML_V2: &str = "<!doctype html><title>計画</title><p>第2版";

    #[tokio::test]
    async fn lists_tools_with_single_html_rule() -> TestResult {
        let mcp = Mcp::connect(None).await?;
        let (_, _, body) = mcp
            .post(&json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}), &[])
            .await?;
        let tools = Mcp::message(&body)?;
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .ok_or("tools がない")?
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for name in [
            "list_projects",
            "list_documents",
            "get_document",
            "create_document",
            "update_document",
        ] {
            assert!(names.contains(&name), "{name} がない: {names:?}");
        }
        assert!(body.contains("1ファイルで完結した HTML"));
        Ok(())
    }

    #[tokio::test]
    async fn create_update_and_read_document() -> TestResult {
        let mcp = Mcp::connect(None).await?;

        let (text, is_error) = mcp.call("list_projects", json!({}), &[]).await?;
        assert!(!is_error, "{text}");
        let projects: Value = serde_json::from_str(&text)?;
        assert_eq!(projects[0]["slug"], "sales");

        let (text, is_error) = mcp
            .call(
                "create_document",
                json!({"project":"sales","title":"事業計画","html":HTML_V1,"slug":"plan"}),
                &[AUTHOR],
            )
            .await?;
        assert!(!is_error, "{text}");
        let created: Value = serde_json::from_str(&text)?;
        assert_eq!(created["slug"], "plan");
        assert_eq!(created["revision_number"], 1);
        assert_eq!(created["url"], "http://localhost:8080/sales/plan");
        assert_eq!(created["message"], "「事業計画」を第1版として登録しました");

        let (text, is_error) = mcp
            .call(
                "update_document",
                json!({"project":"sales","document":"plan","html":HTML_V2,"message":"数字を更新"}),
                &[AUTHOR],
            )
            .await?;
        assert!(!is_error, "{text}");
        assert_eq!(serde_json::from_str::<Value>(&text)?["revision_number"], 2);

        let (text, _) = mcp
            .call("list_documents", json!({"project":"sales"}), &[])
            .await?;
        let documents: Value = serde_json::from_str(&text)?;
        assert_eq!(documents[0]["revision_number"], 2);
        assert_eq!(documents[0]["author_name"], "佐藤");

        let (text, _) = mcp
            .call(
                "get_document",
                json!({"project":"sales","document":"plan"}),
                &[],
            )
            .await?;
        let latest: Value = serde_json::from_str(&text)?;
        assert_eq!(latest["html"], HTML_V2);
        assert_eq!(latest["message"], "数字を更新");
        assert_eq!(latest["url"], "http://localhost:8080/sales/plan");
        assert_eq!(latest["latest_revision_number"], 2);

        let (text, _) = mcp
            .call(
                "get_document",
                json!({"project":"sales","document":"plan","revision":1}),
                &[],
            )
            .await?;
        let first: Value = serde_json::from_str(&text)?;
        assert_eq!(first["html"], HTML_V1);
        assert_eq!(first["revision_number"], 1);
        assert_eq!(first["url"], "http://localhost:8080/sales/plan/v/1");
        Ok(())
    }

    #[tokio::test]
    async fn registration_requires_author_name() -> TestResult {
        let mcp = Mcp::connect(None).await?;
        let (text, is_error) = mcp
            .call(
                "create_document",
                json!({"project":"sales","title":"計画","html":HTML_V1}),
                &[],
            )
            .await?;
        assert!(is_error);
        assert!(text.contains("X-Author-Name"), "{text}");
        Ok(())
    }

    #[tokio::test]
    async fn external_resources_are_reported_for_ai() -> TestResult {
        let mcp = Mcp::connect(None).await?;
        let html =
            "<!doctype html>\n<script src=\"https://cdn.jsdelivr.net/npm/chart.js@4\"></script>";
        let (text, is_error) = mcp
            .call(
                "create_document",
                json!({"project":"sales","title":"グラフ","html":html}),
                &[AUTHOR],
            )
            .await?;
        assert!(is_error);
        assert!(
            text.starts_with("外部リソースを含まない単一 HTML にしてください"),
            "{text}"
        );
        assert!(text.contains("2 行目（スクリプト）"), "{text}");
        assert!(text.contains("chart.js を外部から読み込まず"), "{text}");
        Ok(())
    }

    #[tokio::test]
    async fn unknown_project_or_document_is_a_tool_error() -> TestResult {
        let mcp = Mcp::connect(None).await?;
        let (text, is_error) = mcp
            .call("list_documents", json!({"project":"nope"}), &[])
            .await?;
        assert!(is_error);
        assert_eq!(text, "プロジェクトが見つかりません");
        let (text, is_error) = mcp
            .call(
                "update_document",
                json!({"project":"sales","document":"nope","html":HTML_V1}),
                &[AUTHOR],
            )
            .await?;
        assert!(is_error);
        assert_eq!(text, "資料が見つかりません");
        Ok(())
    }

    #[tokio::test]
    async fn token_is_required_when_configured() -> TestResult {
        let mcp = Mcp::new(Some("s3cret")).await?;
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}});
        let (status, headers, _) = mcp.post(&init, &[]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            headers.get("www-authenticate").map(|v| v.as_bytes()),
            Some(&b"Bearer"[..])
        );
        let (status, _, _) = mcp
            .post(&init, &[("authorization", "Bearer wrong")])
            .await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let mcp = Mcp::connect(Some("s3cret")).await?;
        let (_, is_error) = mcp
            .call(
                "list_projects",
                json!({}),
                &[("authorization", "Bearer s3cret")],
            )
            .await?;
        assert!(!is_error);
        Ok(())
    }

    #[tokio::test]
    async fn oversized_request_is_rejected() -> TestResult {
        let mcp = Mcp::connect(None).await?;
        let html = "a".repeat(MCP_BODY_LIMIT);
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_document","arguments":{"project":"sales","title":"大","html":html}}});
        let (status, _, _) = mcp.post(&request, &[AUTHOR]).await?;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        Ok(())
    }
}
