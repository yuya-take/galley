//! 資料配信（アプリ本体と別オリジン）。

use axum::{Router, extract::Path, http::header, response::IntoResponse, routing::get};

const CSP: &str = "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; form-action 'none'; base-uri 'none'";

pub fn router() -> Router {
    Router::new().route("/r/{revision_id}", get(serve_revision))
}

// 試作（#5）: 別ポートで資料を返せるか確認する。実装は #10。
async fn serve_revision(Path(revision_id): Path<String>) -> impl IntoResponse {
    let html = format!(
        "<!DOCTYPE html><meta charset=\"utf-8\"><h1>revision {}</h1><script>document.body.append(' / script ok')</script>",
        revision_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    );
    (
        [
            (header::CONTENT_SECURITY_POLICY, CSP),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
        ],
        html,
    )
}
