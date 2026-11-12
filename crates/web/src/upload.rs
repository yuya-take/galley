//! 資料の登録（HTML ファイルのドラッグ&ドロップ、⌘V の貼り付け、「HTMLを登録」「新しい版を登録」）。
//!
//! どの画面でも登録できるよう、ダイアログ（[`upload_dialogs`]）はレイアウトに置く。画面の動き
//! （ドロップ、貼り付け、ファイルの読み込み）は `assets/upload.js` で、サーバーとは JSON の API
//! （`/api/upload` の下）でやりとりする。
//!
//! 登録先の見込みは、画面の「HTMLを登録」「新しい版を登録」のボタンに付けた
//! `data-upload-project`（プロジェクトの slug）と `data-upload-document`（資料の slug）から読む。
//! どちらも無い画面（すべての資料など）では、ダイアログでプロジェクトを選ぶ。

mod api;
mod dialog;

pub use dialog::upload_dialogs;

use galley_core::domain::upload::MAX_HTML_BYTES;
use topcoat::router::{BodyLimit, RouterBuilder};

/// 登録の API が受け付けるボディの大きさ。HTML の上限に、JSON のエスケープで増える分の余裕を持たせる
/// （MCP と同じ式）。Topcoat の既定（2MiB）では上限いっぱいの HTML を送れないため、この下だけ広げる。
const BODY_LIMIT: usize = MAX_HTML_BYTES * 2 + 1024 * 1024;

/// 登録の API の置き場所。ボディの上限はこの下にだけ付ける。
const API_PATH: &str = "/api/upload";

/// 登録の API を登録する。
pub fn routes(builder: RouterBuilder) -> RouterBuilder {
    builder
        .layer(BodyLimit::max(BODY_LIMIT).at(API_PATH))
        .layer(api::json_errors)
        .route(api::check_upload)
        .route(api::upload_targets)
        .route(api::register_upload)
}

#[cfg(test)]
mod tests {
    use http::{Request, StatusCode};
    use serde_json::{Value, json};
    use topcoat::router::{Body, Router, to_bytes};

    use super::*;

    /// JSON を POST し、状態と本文（JSON でなければ `Value::Null`）を返す。
    async fn post_json(router: &Router, path: &str, body: Vec<u8>) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header("host", "localhost")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|e| panic!("{e}"));
        let response = router.handle(request).await;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Topcoat の既定（2MiB）を超える HTML も、上限（10MB）までなら受け付ける。
    #[tokio::test]
    async fn api_accepts_html_above_default_body_limit() {
        let router = routes(Router::builder()).build();
        let html = format!("<p>{}</p>", "a".repeat(3 * 1024 * 1024));
        let body = serde_json::to_vec(&json!({ "html": html })).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            post_json(&router, "/api/upload/check", body).await.0,
            StatusCode::OK
        );
    }

    /// ボディの上限を超えたとき・JSON として読めないときも、ほかのエラーと同じ形の JSON で返す。
    #[tokio::test]
    async fn api_errors_before_handler_are_json() {
        let router = routes(Router::builder()).build();
        let (status, body) =
            post_json(&router, "/api/upload/check", vec![b' '; BODY_LIMIT + 1]).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body["error"]["code"], "too_large");
        assert_eq!(
            body["error"]["message"],
            "HTML が大きすぎて送れません（上限は 10 MB）"
        );

        let (status, body) = post_json(&router, "/api/upload", b"{".to_vec()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "bad_request");
    }
}
