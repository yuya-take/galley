//! Galley の起動処理。
//!
//! 画面と MCP（:8080）、資料配信（:8081）を同じプロセスで立ち上げる。

mod mcp;
mod viewer;

use std::future::IntoFuture;

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_APP_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_VIEWER_ADDR: &str = "0.0.0.0:8081";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let web = galley_web::service()?;
    let mcp = StreamableHttpService::new(
        || Ok(mcp::GalleyMcp::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default(),
    );

    // /mcp 以外はすべて Topcoat の画面に渡す
    let app = axum::Router::new()
        .nest_service("/mcp", mcp)
        .fallback_service(web);
    let viewer = viewer::router();

    let app_addr = std::env::var("GALLEY_APP_ADDR").unwrap_or_else(|_| DEFAULT_APP_ADDR.into());
    let viewer_addr =
        std::env::var("GALLEY_VIEWER_ADDR").unwrap_or_else(|_| DEFAULT_VIEWER_ADDR.into());
    let app_listener = TcpListener::bind(&app_addr).await?;
    let viewer_listener = TcpListener::bind(&viewer_addr).await?;
    tracing::info!("app: http://{app_addr}, viewer: http://{viewer_addr}");

    tokio::try_join!(
        axum::serve(app_listener, app).into_future(),
        axum::serve(viewer_listener, viewer).into_future(),
    )?;
    Ok(())
}
