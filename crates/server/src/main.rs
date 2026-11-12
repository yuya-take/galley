//! Galley の起動処理。
//!
//! 画面と MCP（:8080）、資料配信（:8081）を同じプロセスで立ち上げる。

mod app;
mod guard;
mod mcp;
mod viewer;

use std::{
    future::IntoFuture,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context as _;
use axum::Router;
use galley_core::{
    adapter::{blob_store::ObjectStoreBlobStore, sqlite::SqliteDatabase},
    app::{
        document::{
            CreateDocument, FindDocument, FindDocumentByFileName, FindDocumentById, ListDocuments,
            RestoreDocument,
        },
        project::{
            ArchiveProject, CreateProject, FindProject, ListProjects, RestoreProject, UpdateProject,
        },
        revision::{AddRevision, ListRevisions, ReadRevisionContent, RevertToRevision},
    },
    domain::upload::HtmlSizeLimit,
};
use galley_web::{AppUrl, ViewerUrl};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_APP_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_VIEWER_ADDR: &str = "0.0.0.0:8081";
/// DB と資料の実体を置くディレクトリ。Docker イメージでは `/data` を指定する。
const DEFAULT_DATA_DIR: &str = "data";
/// 止める合図のあと、処理中のリクエストを待つ時間。MCP の SSE のように閉じない接続があっても、
/// `docker stop` が SIGKILL を送る（既定 10 秒）より前に終わるようにする。
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let data_dir =
        PathBuf::from(std::env::var("GALLEY_DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.into()));
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("galley.db");
    let database = SqliteDatabase::open(&db_path).await?;
    tracing::info!("database: {}", db_path.display());
    let blobs = Arc::new(open_blob_store(&data_dir)?);

    let app_addr = std::env::var("GALLEY_APP_ADDR").unwrap_or_else(|_| DEFAULT_APP_ADDR.into());
    let viewer_addr =
        std::env::var("GALLEY_VIEWER_ADDR").unwrap_or_else(|_| DEFAULT_VIEWER_ADDR.into());
    // 画面の iframe に入れる URL
    let viewer_url = viewer_url_from_env(&app_addr, &viewer_addr)?;
    tracing::info!("viewer url: {viewer_url}");
    let allowed_hosts = Arc::new(allowed_hosts_from_env(&viewer_url)?);

    let app_url = app_url_from_env()?;
    tracing::info!("app url: {app_url}");
    let html_size_limit = html_size_limit_from_env()?;
    tracing::info!("max html size: {html_size_limit}");

    let projects = Arc::new(database.projects());
    let documents = Arc::new(database.documents());
    let web = galley_web::service(Arc::new(galley_web::WebApp {
        list_projects: ListProjects::new(projects.clone()),
        find_project: FindProject::new(projects.clone()),
        create_project: CreateProject::new(projects.clone()),
        update_project: UpdateProject::new(projects.clone()),
        archive_project: ArchiveProject::new(projects.clone()),
        restore_project: RestoreProject::new(projects.clone()),
        list_documents: ListDocuments::new(documents.clone()),
        find_document: FindDocument::new(documents.clone()),
        restore_document: RestoreDocument::new(documents.clone()),
        list_revisions: ListRevisions::new(Arc::new(database.revisions())),
        revert_to_revision: RevertToRevision::new(
            documents.clone(),
            Arc::new(database.revisions()),
        ),
        create_document: CreateDocument::new(
            projects,
            documents.clone(),
            blobs.clone(),
            html_size_limit,
        ),
        add_revision: AddRevision::new(
            documents.clone(),
            Arc::new(database.revisions()),
            blobs.clone(),
            html_size_limit,
        ),
        find_document_by_file_name: FindDocumentByFileName::new(documents),
        viewer_url,
        app_url: app_url.clone(),
        html_size_limit,
    }))?;
    let mcp_token = Arc::new(guard::McpToken::from_env(std::env::var("MCP_TOKEN").ok()));
    if mcp_token.is_required() {
        tracing::info!("MCP は Authorization: Bearer のトークンを求めます");
    }
    let app = app::app_router(
        web,
        app::mcp_app(
            app::Ports {
                projects: Arc::new(database.projects()),
                documents: Arc::new(database.documents()),
                revisions: Arc::new(database.revisions()),
                blobs: blobs.clone(),
            },
            app_url,
            html_size_limit,
        ),
        &allowed_hosts,
        mcp_token,
        html_size_limit,
    );
    let viewer = viewer::router(
        Arc::new(viewer::ViewerApp {
            read_revision: ReadRevisionContent::new(Arc::new(database.revisions()), blobs),
            find_document: FindDocumentById::new(Arc::new(database.documents())),
        }),
        allowed_hosts,
    );

    let app_listener = TcpListener::bind(&app_addr).await?;
    let viewer_listener = TcpListener::bind(&viewer_addr).await?;
    tracing::info!("app: http://{app_addr}, viewer: http://{viewer_addr}");

    serve(app_listener, app, viewer_listener, viewer).await
}

/// アプリと資料配信を動かす。止める合図が来たら新しい接続を受け付けずに処理中のリクエストを待ち、
/// `SHUTDOWN_GRACE` を過ぎたら残りを打ち切って終わる。
async fn serve(
    app_listener: TcpListener,
    app: Router,
    viewer_listener: TcpListener,
    viewer: Router,
) -> anyhow::Result<()> {
    let servers = async {
        tokio::try_join!(
            axum::serve(app_listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .into_future(),
            axum::serve(viewer_listener, viewer)
                .with_graceful_shutdown(shutdown_signal())
                .into_future(),
        )
    };
    tokio::pin!(servers);
    tokio::select! {
        result = &mut servers => {
            result?;
        }
        () = shutdown_signal() => {
            tracing::info!(
                "停止します（処理中のリクエストを最大 {} 秒待ちます）",
                SHUTDOWN_GRACE.as_secs()
            );
            match tokio::time::timeout(SHUTDOWN_GRACE, &mut servers).await {
                Ok(result) => {
                    result?;
                }
                Err(_) => tracing::warn!("終わらなかった接続を打ち切ります"),
            }
        }
    }
    Ok(())
}

/// 止める合図（SIGTERM か Ctrl+C）を待つ。
///
/// コンテナでは PID 1 で動くので、ハンドラーが無いと SIGTERM が無視され、`docker stop` は
/// 待ったあとに SIGKILL で止めることになる。合図は待っているすべての `shutdown_signal` に届く。
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!("Ctrl+C を待てません: {error}");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(error) => {
                tracing::error!("SIGTERM を待てません: {error}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}

/// アプリの画面の URL（MCP が返す資料ビューアのリンク）。`GALLEY_APP_URL` が無ければリクエストの Host から作る。
fn app_url_from_env() -> anyhow::Result<AppUrl> {
    match std::env::var("GALLEY_APP_URL") {
        Ok(url) if !url.is_empty() => Ok(AppUrl::parse(&url)?),
        _ => Ok(AppUrl::FromRequest),
    }
}

/// 1ファイルのサイズ上限。`GALLEY_MAX_HTML_SIZE`（「20MB」「512KB」）が無ければ 10MB。
fn html_size_limit_from_env() -> anyhow::Result<HtmlSizeLimit> {
    match std::env::var("GALLEY_MAX_HTML_SIZE") {
        Ok(value) if !value.is_empty() => HtmlSizeLimit::parse(&value)
            .with_context(|| format!("GALLEY_MAX_HTML_SIZE「{value}」は使えません")),
        _ => Ok(HtmlSizeLimit::DEFAULT),
    }
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
