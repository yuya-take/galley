//! 画面。画面ごとにモジュールを分ける。版の履歴（#16）などはそれぞれの Issue で作る。

pub mod archive;
pub mod connect;
pub mod documents;
pub mod history;
pub mod new_project;
mod project_form;
pub mod project_settings;
#[cfg(test)]
mod test_app;
pub mod viewer;

use topcoat::router::not_found;

// `/{*rest}` は `/{project}` と同じ位置で衝突する（matchit は catch-all と引数を同時に登録できない）ので、
// 3階層目から下を受ける。1・2階層の見知らぬ URL は、`/{project}` と `/{project}/{document}` が
// プロジェクトや資料を見つけられずに 404 にする
not_found!("/{project}/{document}");
