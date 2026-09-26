//! マイグレーションを作る開発用ツール。リポジトリのルートで実行する（`Toasty.toml` を読む）。
//!
//! ```sh
//! cargo run -p galley-migrate -- migration generate --name <内容>
//! ```
//!
//! モデル（`galley_core::adapter::sqlite::model`）とスナップショットの差分から SQL を作り、
//! `crates/core/db/` に書き出す。適用はアプリの起動時に自動で行う。

use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    // SQL の方言を決めるためだけに接続する。差分はスナップショットと比べるので DB の中身は使わない
    let db = toasty::Db::builder()
        .models(galley_core::adapter::sqlite::models())
        .connect("sqlite::memory:")
        .await?;
    ToastyCli::with_config(db, config).parse_and_run().await?;
    Ok(())
}
