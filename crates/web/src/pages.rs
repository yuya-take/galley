//! 画面。資料の一覧（#14）、資料を読む（#15）などはそれぞれの Issue で作る。

use topcoat::{
    Result,
    router::{not_found, page},
    view::{View, view},
};

not_found!("/");

/// すべての資料。一覧は #14 で作る。
#[page("/")]
pub async fn home() -> Result<impl View> {
    Ok(view! {
        <header class="page-header">
            <div class="page-title">
                <div class="page-title-line"><h1>"すべての資料"</h1></div>
                <p class="page-description">"すべてのプロジェクトの資料を、更新が新しい順に並べています"</p>
            </div>
            <button type="button" class="button button--primary" aria-disabled="true" title="登録は準備中です">"＋ HTMLを登録"</button>
        </header>
        <div class="panel">
            <p class="page-description">"資料の一覧は準備中です。"</p>
        </div>
    })
}
