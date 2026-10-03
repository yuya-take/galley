//! SQLite（Toasty）の実装。
//!
//! Toasty は外部キー制約を作らず、接続ごとの PRAGMA も設定できないため、
//! 参照の整合（資料のプロジェクトが存在するかなど）はユースケース側で保つ。
//! WAL モードは DB ファイルに記録されるので、開くときに1回設定すれば足りる。

mod convert;
mod document;
pub mod model;
mod project;
mod raw;
mod revision;

use std::{path::Path, sync::Arc};

pub use document::ToastyDocumentRepository;
pub use project::ToastyProjectRepository;
pub use revision::ToastyRevisionRepository;

use crate::domain::error::RepositoryError;

/// 起動時に適用するマイグレーション（`crates/core/db/`）。
static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!("db");

#[derive(Debug, thiserror::Error)]
#[error("データベースを開けません: {0}")]
pub struct OpenDatabaseError(#[from] toasty::Error);

/// Toasty のモデル一覧。マイグレーションを作るツール（`galley-migrate`）からも使う。
pub fn models() -> toasty::ModelSet {
    toasty::models!(crate::*)
}

/// SQLite のデータベース。複製しても同じ接続プールを使う。
#[derive(Debug, Clone)]
pub struct SqliteDatabase {
    db: toasty::Db,
    writes: WriteLock,
}

/// 書き込みを1本ずつにする。
///
/// Toasty の SQLite ドライバーは接続ごとのタスクの中で同期的に SQLite を呼ぶので、ロックを待つ接続が
/// tokio のワーカーを塞ぐ。待つ接続が増えるとロックを持つ接続が進めず、待ち時間（5秒）を過ぎて
/// 「database is locked」になる。アプリの中で書き込みを順番待ちにして、SQLite のロックを待たせない。
/// 読み出しは WAL なので書き込みを待たない。
#[derive(Debug, Clone, Default)]
pub(super) struct WriteLock(Arc<tokio::sync::Mutex<()>>);

impl WriteLock {
    pub(super) async fn acquire(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.0.lock().await
    }
}

impl SqliteDatabase {
    /// ファイルの DB を開き、WAL モードにして、未適用のマイグレーションを適用する。
    pub async fn open(path: &Path) -> Result<Self, OpenDatabaseError> {
        let db = toasty::Db::builder()
            .models(models())
            .connect(&format!("sqlite:{}", path.display()))
            .await?;
        let mut database = Self {
            db,
            writes: WriteLock::default(),
        };
        database.enable_wal().await?;
        database.migrate().await?;
        Ok(database)
    }

    /// メモリ上の DB を開き、マイグレーションを適用する。テスト用。
    pub async fn in_memory() -> Result<Self, OpenDatabaseError> {
        let db = toasty::Db::builder()
            .models(models())
            .connect("sqlite::memory:")
            .await?;
        let database = Self {
            db,
            writes: WriteLock::default(),
        };
        database.migrate().await?;
        Ok(database)
    }

    pub fn projects(&self) -> ToastyProjectRepository {
        ToastyProjectRepository::new(self.db.clone(), self.writes.clone())
    }

    pub fn documents(&self) -> ToastyDocumentRepository {
        ToastyDocumentRepository::new(self.db.clone(), self.writes.clone())
    }

    pub fn revisions(&self) -> ToastyRevisionRepository {
        ToastyRevisionRepository::new(self.db.clone())
    }

    /// 現在のジャーナルモード（`wal` など）。
    pub async fn journal_mode(&self) -> Result<String, RepositoryError> {
        let mut db = self.db.clone();
        let rows = toasty::sql::query("PRAGMA journal_mode")
            .column_types([toasty::stmt::Type::String])
            .exec(&mut db)
            .await
            .map_err(backend_error)?;
        match rows.into_iter().next() {
            Some(toasty::stmt::Value::Record(row)) => match row.fields.into_iter().next() {
                Some(toasty::stmt::Value::String(mode)) => Ok(mode),
                other => Err(RepositoryError::Corrupted(format!(
                    "journal_mode: {other:?}"
                ))),
            },
            other => Err(RepositoryError::Corrupted(format!(
                "journal_mode: {other:?}"
            ))),
        }
    }

    async fn enable_wal(&mut self) -> toasty::Result<()> {
        // journal_mode は結果の行を返すので statement ではなく query で実行する
        toasty::sql::query("PRAGMA journal_mode = WAL")
            .exec(&mut self.db)
            .await?;
        Ok(())
    }

    async fn migrate(&self) -> toasty::Result<()> {
        let report = MIGRATIONS.apply(&self.db).await?;
        tracing::info!(applied = report.applied(), "マイグレーションを適用しました");
        Ok(())
    }
}

/// Toasty のエラーを domain のエラーに変換する。
///
/// Toasty は一意制約の違反を区別しないので、SQLite のメッセージで判定する。
fn backend_error(err: toasty::Error) -> RepositoryError {
    let message = err.to_string();
    if message.contains("UNIQUE constraint failed") {
        return RepositoryError::Conflict(message);
    }
    RepositoryError::Backend(Box::new(err))
}
