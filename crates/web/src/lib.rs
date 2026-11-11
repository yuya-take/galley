//! Galley の画面。
//!
//! Topcoat は実験段階のため、Topcoat への依存はこのクレートに閉じ込める。
//! `galley-server` は [`service`] の戻り値を Axum の fallback に載せるだけで、
//! Topcoat の API を直接使わない。

mod app_url;
pub mod host;
mod viewer_url;

pub use app_url::{AppUrl, AppUrlError};
pub use viewer_url::{ViewerUrl, ViewerUrlError};

use std::sync::Arc;

use galley_core::{
    app::{error::AppError, project::ListProjects},
    domain::{project::ProjectSummary, shared::ArchiveFilter},
};
use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    font::RouterBuilderFontExt,
    router::{Router, tower::TowerService},
    runtime::RouterBuilderRuntimeExt,
};

mod author;
mod fonts;
mod icon;
mod layout;
mod pages;
mod sidebar;

/// 画面から使うユースケース。起動時に1つ作り、Topcoat の app context で共有する。
pub struct WebApp {
    pub list_projects: ListProjects,
}

impl WebApp {
    /// サイドバーのプロジェクト（アーカイブしたものを除く、作成順）。
    async fn sidebar_projects(&self) -> Result<Vec<ProjectSummary>, AppError> {
        self.list_projects.execute(ArchiveFilter::Active).await
    }
}

/// 画面を tower の `Service` として返す。
///
/// アセット（runtime のスクリプト、CSS、書体など）は `topcoat asset bundle` で作ったバンドルを
/// 実行ファイルの隣から読み込む。
pub fn service(app: Arc<WebApp>) -> std::io::Result<TowerService> {
    let mut builder = Router::builder()
        .app_context(app)
        .layout(layout::root_layout)
        .page(pages::home)
        .page(pages::not_found);
    for font in fonts::ALL {
        builder = builder.font(font);
    }
    let router = builder.assets(AssetBundle::load()?).runtime().build();
    Ok(TowerService::new(router))
}
