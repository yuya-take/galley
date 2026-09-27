//! Galley の起動処理。
//!
//! 画面と MCP（:8080）、資料配信（:8081）を同じプロセスで立ち上げる。

mod mcp;
mod viewer;

use std::{
    future::IntoFuture,
    path::{Path, PathBuf},
};

use galley_core::adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase};

use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_APP_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_VIEWER_ADDR: &str = "0.0.0.0:8081";
/// DB と資料の実体を置くディレクトリ。Docker イメージでは `/data` を指定する。
const DEFAULT_DATA_DIR: &str = "data";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let data_dir =
        PathBuf::from(std::env::var("GALLEY_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.into()));
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("galley.db");
    // 画面と MCP にユースケースを渡すのは #12〜#18。いまは DB のマイグレーションと保存先の準備だけ行う
    let _database = SqliteDatabase::open(&db_path).await?;
    tracing::info!("database: {}", db_path.display());
    let _blobs = open_blob_store(&data_dir)?;

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

/// 資料の実体の保存先を開く。`GALLEY_BLOB_STORE` に `s3://` の URL があれば S3 互換のストレージ、
/// 無ければ `<GALLEY_DATA_DIR>/blobs` に置く。
fn open_blob_store(data_dir: &Path) -> anyhow::Result<ObjectStoreBlobStore> {
    match std::env::var("GALLEY_BLOB_STORE") {
        Ok(url) if !url.is_empty() => {
            tracing::info!("blobs: {url}");
            Ok(ObjectStoreBlobStore::s3(&url)?)
        }
        _ => {
            let root = data_dir.join("blobs");
            tracing::info!("blobs: {}", root.display());
            Ok(ObjectStoreBlobStore::local(&root)?)
        }
    }
}
