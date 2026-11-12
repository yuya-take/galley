//! 画面。画面ごとにモジュールを分ける。資料を読む（#15）などはそれぞれの Issue で作る。

pub mod archive;
pub mod connect;
pub mod documents;
pub mod new_project;
mod project_form;
pub mod project_settings;
#[cfg(test)]
mod test_app;

use topcoat::router::not_found;

// `/{*rest}` は `/{project}` と同じ位置で衝突する（matchit は catch-all と引数を同時に登録できない）ので、
// 2階層目から下を受ける。1階層の見知らぬ URL は `/{project}` がプロジェクトを見つけられずに 404 にする
not_found!("/{project}");
