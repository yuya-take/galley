//! 全画面で共通の骨組み（`<head>`、サイドバー、名前のダイアログ、見つからないときの表示）。

use std::sync::Arc;

use http::StatusCode;
use topcoat::{
    Result,
    asset::{Asset, asset},
    context::{Cx, app_context},
    font,
    router::{Slot, error::NotFoundError, layout, request},
    view::{View, error_boundary, view},
};

use crate::{WebApp, author, fonts, sidebar::sidebar};

const APP_CSS: Asset = asset!("assets/app.css");
const APP_JS: Asset = asset!("assets/app.js");
const FAVICON: Asset = asset!("assets/favicon.svg");

#[layout("/")]
pub async fn root_layout(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let author = author::from_headers(request::headers(cx));
    let projects = app.sidebar_projects().await?;
    Ok(view! {
        <!DOCTYPE html>
        <html lang="ja">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Galley"</title>
                <link rel="icon" type="image/svg+xml" href=(FAVICON)>
                font::link(font: fonts::LOGO)
                font::link(font: fonts::HEADING)
                font::link(font: fonts::BODY)
                font::link(font: fonts::MONO)
                <link rel="stylesheet" href=(APP_CSS)>
                topcoat::runtime::script()
                <script src=(APP_JS) defer=""></script>
            </head>
            <body>
                <div class="app">
                    sidebar(projects: &projects, author: author.as_ref())
                    <main class="app-main">
                        error_boundary(
                            fallback: |error| {
                                if error.downcast_ref::<NotFoundError>().is_none() {
                                    return Err(error);
                                }
                                Ok(view! {
                                    (StatusCode::NOT_FOUND)
                                    <section class="page-title">
                                        <h1>"ページが見つかりません"</h1>
                                        <p class="page-description">"URL が変わったか、資料やプロジェクトがアーカイブされた可能性があります。"</p>
                                        <p class="page-description"><a href="/">"すべての資料へ戻る"</a></p>
                                    </section>
                                })
                            },
                            (slot)
                        )
                    </main>
                </div>
                author_dialog(author: author.as_ref().map(|a| a.as_str()))
            </body>
        </html>
    })
}

/// 更新者名を聞くダイアログ（「名前を変える」）。保存は `assets/app.js`。
#[topcoat::view::component]
async fn author_dialog(author: Option<&str>) -> Result<impl View> {
    Ok(view! {
        <dialog id="author-dialog" class="dialog" aria-labelledby="author-dialog-title">
            <form method="dialog" class="dialog-body">
                <h2 id="author-dialog-title">"あなたの名前"</h2>
                <p>"登録した資料の更新者として残ります。このブラウザに保存されます。"</p>
                <label class="field">
                    "名前"
                    <input class="input" name="author" maxlength="50" autocomplete="name" value=(author.unwrap_or_default()) required="">
                </label>
                <p class="field-error" data-error="" hidden="">"名前を入れてください（50文字まで）"</p>
                <div class="dialog-actions">
                    <button type="button" class="button button--outline" data-action="cancel">"やめる"</button>
                    <button type="submit" class="button button--primary">"保存する"</button>
                </div>
            </form>
        </dialog>
    })
}
