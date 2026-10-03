//! 資料配信（アプリ本体と別オリジン）。
//!
//! 資料の HTML を `GET /r/<revision_id>` で返す。資料内のスクリプトがアプリを操作したり外部と
//! 通信したりしないよう、sandbox 付きの CSP を付ける。404 などを含むすべてのレスポンスに
//! 同じヘッダーを付ける（URL を直接開かれても同じ制限がかかるように）。

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use galley_core::{
    app::{error::AppError, revision::ReadRevisionContent},
    domain::shared::RevisionId,
};

pub const CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; form-action 'none'; base-uri 'none'";

/// 版は変更されないので、ブラウザーに長く持たせる。
const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub fn router(read: Arc<ReadRevisionContent>) -> Router {
    Router::new()
        .route("/r/{revision_id}", get(serve_revision))
        .fallback(not_found)
        .with_state(read)
        .layer(middleware::map_response(security_headers))
}

/// すべてのレスポンスに付けるヘッダー。
async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // 資料内のリンクから移動したとき、版の URL を移動先に渡さない
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}

async fn serve_revision(
    State(read): State<Arc<ReadRevisionContent>>,
    Path(revision_id): Path<String>,
) -> Response {
    let Ok(id) = RevisionId::parse(&revision_id) else {
        return not_found().await;
    };
    match read.execute(id).await {
        Ok(content) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, CACHE_IMMUTABLE),
            ],
            content.html,
        )
            .into_response(),
        Err(AppError::RevisionNotFound) => not_found().await,
        Err(err) => {
            tracing::error!(%id, "版の HTML を読めません: {err}");
            text(
                StatusCode::INTERNAL_SERVER_ERROR,
                "資料を読み込めませんでした",
            )
        }
    }
}

async fn not_found() -> Response {
    text(StatusCode::NOT_FOUND, "資料が見つかりません")
}

fn text(status: StatusCode, body: &'static str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use axum::body::Body;
    use galley_core::{
        adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
        app::{
            document::{CreateDocument, CreateDocumentInput, SlugSource},
            project::{CreateProject, ProjectInput},
            revision::RevisionInput,
        },
        domain::revision::RevisionSource,
    };
    use http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    const HTML: &str =
        "<!doctype html><title>計画</title><script>document.body.append('ok')</script>";

    /// 資料を1つ登録した資料配信と、その版の ID。
    async fn viewer() -> Result<(Router, RevisionId), Box<dyn Error>> {
        let database = SqliteDatabase::in_memory().await?;
        let blobs = Arc::new(ObjectStoreBlobStore::in_memory());
        let project = CreateProject::new(Arc::new(database.projects()))
            .execute(&ProjectInput {
                slug: "sales".to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "blue".to_owned(),
            })
            .await?;
        let created = CreateDocument::new(
            Arc::new(database.projects()),
            Arc::new(database.documents()),
            blobs.clone(),
        )
        .execute(CreateDocumentInput {
            project_id: project.id,
            title: None,
            slug: SlugSource::None,
            revision: RevisionInput {
                html: HTML.as_bytes().to_vec(),
                message: String::new(),
                author_name: "佐藤".to_owned(),
                source: RevisionSource::Web,
            },
        })
        .await?;
        let read = Arc::new(ReadRevisionContent::new(
            Arc::new(database.revisions()),
            blobs,
        ));
        Ok((router(read), created.revision.id))
    }

    async fn send(router: &Router, method: &str, uri: &str) -> Result<Response, Box<dyn Error>> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())?;
        Ok(router.clone().oneshot(request).await?)
    }

    #[track_caller]
    fn assert_security_headers(response: &Response) {
        let headers = response.headers();
        assert_eq!(
            headers
                .get(header::CONTENT_SECURITY_POLICY)
                .map(|v| v.as_bytes()),
            Some(CSP.as_bytes())
        );
        assert_eq!(
            headers
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .map(|v| v.as_bytes()),
            Some(&b"nosniff"[..])
        );
        assert_eq!(
            headers.get(header::REFERRER_POLICY).map(|v| v.as_bytes()),
            Some(&b"no-referrer"[..])
        );
    }

    #[tokio::test]
    async fn serves_revision_html_with_headers() -> Result<(), Box<dyn Error>> {
        let (router, id) = viewer().await?;
        let response = send(&router, "GET", &format!("/r/{id}")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_security_headers(&response);
        let headers = response.headers();
        assert_eq!(
            headers.get(header::CONTENT_TYPE).map(|v| v.as_bytes()),
            Some(&b"text/html; charset=utf-8"[..])
        );
        assert_eq!(
            headers.get(header::CACHE_CONTROL).map(|v| v.as_bytes()),
            Some(CACHE_IMMUTABLE.as_bytes())
        );
        let body = response.into_body().collect().await?.to_bytes();
        assert_eq!(body, HTML.as_bytes());
        Ok(())
    }

    #[tokio::test]
    async fn head_returns_headers_without_body() -> Result<(), Box<dyn Error>> {
        let (router, id) = viewer().await?;
        let response = send(&router, "HEAD", &format!("/r/{id}")).await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert_security_headers(&response);
        Ok(())
    }

    #[tokio::test]
    async fn every_error_response_has_headers() -> Result<(), Box<dyn Error>> {
        let (router, id) = viewer().await?;
        let cases = [
            (
                "GET",
                format!("/r/{}", RevisionId::generate()),
                StatusCode::NOT_FOUND,
            ),
            (
                "GET",
                format!("/r/{}", id.to_string().to_uppercase()),
                StatusCode::NOT_FOUND,
            ),
            ("GET", "/r/not-an-id".to_owned(), StatusCode::NOT_FOUND),
            ("GET", "/r/".to_owned(), StatusCode::NOT_FOUND),
            ("GET", "/".to_owned(), StatusCode::NOT_FOUND),
            ("GET", "/r/a/b".to_owned(), StatusCode::NOT_FOUND),
            ("POST", format!("/r/{id}"), StatusCode::METHOD_NOT_ALLOWED),
            ("DELETE", format!("/r/{id}"), StatusCode::METHOD_NOT_ALLOWED),
        ];
        for (method, uri, status) in cases {
            let response = send(&router, method, &uri).await?;
            assert_eq!(response.status(), status, "{method} {uri}");
            assert_security_headers(&response);
            assert_eq!(
                response
                    .headers()
                    .get(header::CACHE_CONTROL)
                    .map(|v| v.as_bytes()),
                Some(&b"no-store"[..]),
                "{method} {uri}"
            );
        }
        Ok(())
    }
}
