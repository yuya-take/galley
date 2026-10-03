//! Galley の起動処理。
//!
//! 画面と MCP（:8080）、資料配信（:8081）を同じプロセスで立ち上げる。

mod guard;
mod mcp;
mod viewer;

use std::{
    future::IntoFuture,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use axum::middleware;
use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::revision::ReadRevisionContent,
};
use galley_web::ViewerUrl;

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
    // 画面と MCP にユースケースを渡すのは #12〜#18
    let database = SqliteDatabase::open(&db_path).await?;
    tracing::info!("database: {}", db_path.display());
    let blobs = Arc::new(open_blob_store(&data_dir)?);

    let app_addr = std::env::var("GALLEY_APP_ADDR").unwrap_or_else(|_| DEFAULT_APP_ADDR.into());
    let viewer_addr =
        std::env::var("GALLEY_VIEWER_ADDR").unwrap_or_else(|_| DEFAULT_VIEWER_ADDR.into());
    // 画面の iframe に入れる URL。画面に渡すのは #15
    let viewer_url = viewer_url_from_env(&app_addr, &viewer_addr)?;
    tracing::info!("viewer url: {viewer_url}");
    let allowed_hosts = Arc::new(allowed_hosts_from_env(&viewer_url)?);

    let web = galley_web::service()?;
    let mcp = StreamableHttpService::new(
        || Ok(mcp::GalleyMcp::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_allowed_hosts(allowed_hosts.for_rmcp()),
    );

    // /mcp 以外はすべて Topcoat の画面に渡す。Host の検査を一番外に置く
    let app = axum::Router::new()
        .nest_service("/mcp", mcp)
        .fallback_service(web)
        .layer(middleware::from_fn(guard::check_csrf))
        .layer(middleware::from_fn_with_state(
            allowed_hosts.clone(),
            guard::check_host,
        ));
    let viewer = viewer::router(
        Arc::new(ReadRevisionContent::new(
            Arc::new(database.revisions()),
            blobs,
        )),
        allowed_hosts,
    );

    let app_listener = TcpListener::bind(&app_addr).await?;
    let viewer_listener = TcpListener::bind(&viewer_addr).await?;
    tracing::info!("app: http://{app_addr}, viewer: http://{viewer_addr}");

    tokio::try_join!(
        axum::serve(app_listener, app).into_future(),
        axum::serve(viewer_listener, viewer).into_future(),
    )?;
    Ok(())
}

/// 受け付けるホスト名（DNS リバインディング対策）。`ALLOWED_HOSTS` が未設定なら localhost だけ。
/// `GALLEY_VIEWER_URL` を指定していれば、そのホスト名も加える。
fn allowed_hosts_from_env(viewer_url: &ViewerUrl) -> anyhow::Result<guard::AllowedHosts> {
    let configured = std::env::var("ALLOWED_HOSTS").ok();
    let mut hosts = guard::AllowedHosts::parse(configured.as_deref())?;
    if let Some(authority) = viewer_url.fixed_authority() {
        hosts = hosts.with(authority);
    }
    if hosts.is_default() {
        tracing::warn!(
            "ALLOWED_HOSTS が未設定のため、localhost からのアクセスだけを受け付けます。\
             ほかの PC から使うときは ALLOWED_HOSTS=galley.example.com のようにホスト名を指定してください"
        );
    }
    tracing::info!("allowed hosts: {hosts}");
    Ok(hosts)
}

/// 資料配信の URL。`GALLEY_VIEWER_URL` があればそれを、無ければアプリと同じホスト名で
/// 資料配信のポートにする。
///
/// 資料をアプリと同じオリジンで表示すると隔離が効かないので、同じポートなら起動を止める。
fn viewer_url_from_env(app_addr: &str, viewer_addr: &str) -> anyhow::Result<ViewerUrl> {
    if let Ok(url) = std::env::var("GALLEY_VIEWER_URL")
        && !url.is_empty()
    {
        return Ok(ViewerUrl::parse(&url)?);
    }
    let port = |name: &str, addr: &str| {
        addr.parse::<SocketAddr>()
            .map(|a| a.port())
            .with_context(|| format!("{name}「{addr}」を読めません"))
    };
    let app_port = port("GALLEY_APP_ADDR", app_addr)?;
    let viewer_port = port("GALLEY_VIEWER_ADDR", viewer_addr)?;
    anyhow::ensure!(
        app_port != viewer_port,
        "資料配信はアプリと別のポートにしてください（どちらも {app_port}）"
    );
    Ok(ViewerUrl::SameHost { port: viewer_port })
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
