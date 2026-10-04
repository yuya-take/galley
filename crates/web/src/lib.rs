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

use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    context::Cx,
    router::{Router, Slot, layout, page, route, tower::TowerService},
    runtime::{RouterBuilderRuntimeExt, signal},
    view::{View, view},
};

/// 画面を tower の `Service` として返す。
///
/// アセット（runtime のスクリプトなど）は `topcoat asset bundle` で作ったバンドルを
/// 実行ファイルの隣から読み込む。
pub fn service() -> std::io::Result<TowerService> {
    let router = Router::builder()
        .layout(root_layout)
        .page(home)
        .route(echo)
        .assets(AssetBundle::load()?)
        .runtime()
        .build();
    Ok(TowerService::new(router))
}

#[layout("/")]
async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="ja">
            <head>
                <meta charset="utf-8">
                <title>"Galley"</title>
                topcoat::runtime::script()
            </head>
            <body>(slot)</body>
        </html>
    })
}

// 試作（#5）: runtime の `$()` 式がブラウザで動くか確認する。
#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let count = signal(cx, || 0i32);

    Ok(view! {
        <h1>"Galley"</h1>
        <button @click=$(|_e| count.increment())>"第" $(count.get()) "版"</button>
    })
}

// 試作（#5）: 状態を変えるルートで、Topcoat の OriginPolicy が効くか確認する。
#[route(POST "/api/echo")]
async fn echo() -> Result<&'static str> {
    Ok("ok")
}
