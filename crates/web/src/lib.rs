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
    app::{
        document::{
            CreateDocument, FindDocument, FindDocumentByFileName, ListDocuments, RestoreDocument,
        },
        error::AppError,
        project::{
            ArchiveProject, CreateProject, FindProject, ListProjects, RestoreProject, UpdateProject,
        },
        revision::{AddRevision, ListRevisions, RevertToRevision},
    },
    domain::{project::ProjectSummary, shared::ArchiveFilter, upload::HtmlSizeLimit},
};
use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    font::RouterBuilderFontExt,
    router::{Router, RouterBuilder, tower::TowerService},
    runtime::RouterBuilderRuntimeExt,
};

mod author;
mod date;
mod fonts;
mod icon;
mod layout;
mod mcp_command;
mod pages;
mod search;
mod sidebar;
mod upload;

/// 画面から使うユースケース。起動時に1つ作り、Topcoat の app context で共有する。
pub struct WebApp {
    pub list_projects: ListProjects,
    pub find_project: FindProject,
    pub create_project: CreateProject,
    pub update_project: UpdateProject,
    pub archive_project: ArchiveProject,
    pub restore_project: RestoreProject,
    pub list_documents: ListDocuments,
    pub find_document: FindDocument,
    pub restore_document: RestoreDocument,
    pub list_revisions: ListRevisions,
    /// 資料ビューアの「この版に戻す」。
    pub revert_to_revision: RevertToRevision,
    /// 登録（新しい資料）。
    pub create_document: CreateDocument,
    /// 登録（既存の資料の新しい版）。
    pub add_revision: AddRevision,
    /// 登録の確認ダイアログの「ファイル名が一致」。
    pub find_document_by_file_name: FindDocumentByFileName,
    /// プレビューの iframe に入れる資料配信の URL。
    pub viewer_url: ViewerUrl,
    /// アプリの URL（空のプロジェクトに出す MCP の登録コマンド、URL の欄の前の「<ホスト>/」）。
    pub app_url: AppUrl,
    /// 1ファイルのサイズ上限（登録の API のボディの上限と文言、ダイアログでの事前の確認）。
    /// `create_document`・`add_revision` に渡したものと同じ値にする。
    pub html_size_limit: HtmlSizeLimit,
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
    let html_size_limit = app.html_size_limit;
    let mut builder = routes(Router::builder().app_context(app), html_size_limit);
    for font in fonts::ALL {
        builder = builder.font(font);
    }
    let router = builder.assets(AssetBundle::load()?).runtime().build();
    Ok(TowerService::new(router))
}

/// レイアウト・画面・procedure を登録する。
fn routes(builder: RouterBuilder, html_size_limit: HtmlSizeLimit) -> RouterBuilder {
    upload::routes(builder, html_size_limit)
        .layout(layout::root_layout)
        .page(pages::documents::all_documents)
        .page(pages::documents::project_documents)
        .route(pages::documents::document_preview)
        .page(pages::viewer::document_viewer)
        .page(pages::viewer::revision_viewer)
        .page(pages::history::revision_history)
        .route(pages::history::revision_preview)
        .page(pages::not_found)
        .page(pages::new_project::new_project)
        .page(pages::project_settings::project_settings)
        .page(pages::archive::archive)
        .page(pages::connect::connect)
        .route(pages::new_project::create_project)
        .route(pages::project_settings::update_project)
        .route(pages::project_settings::archive_project)
        .route(pages::archive::restore_project)
        .route(pages::archive::restore_document)
        .route(pages::viewer::revert_to_revision)
        .route(search::document_search)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ルートが衝突すると起動時に panic するので、テストで先に見つける。
    #[test]
    fn routes_do_not_conflict() {
        let _router = routes(Router::builder(), HtmlSizeLimit::DEFAULT).build();
    }
}
