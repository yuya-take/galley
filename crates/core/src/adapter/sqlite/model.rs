//! Toasty のモデル（テーブルの定義）。adapter の外には出さず、domain の型に変換して返す。
//!
//! モデルを変えたら `cargo run -p galley-migrate -- migration generate --name <内容>` で
//! マイグレーションを作る（`crates/core/db/`）。

use jiff::Timestamp;
use uuid::Uuid;

#[derive(Debug, toasty::Model)]
#[table = "projects"]
pub struct ProjectRecord {
    #[key]
    pub id: Uuid,
    #[unique]
    pub slug: String,
    pub name: String,
    pub description: String,
    pub color: String,
    #[index]
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

#[derive(Debug, toasty::Model)]
#[table = "documents"]
#[unique(project_id, slug)]
pub struct DocumentRecord {
    #[key]
    pub id: Uuid,
    pub project_id: Uuid,
    pub slug: String,
    pub title: String,
    pub current_revision_id: Uuid,
    #[index]
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
    #[index]
    pub updated_at: Timestamp,
}

#[derive(Debug, toasty::Model)]
#[table = "revisions"]
#[unique(document_id, number)]
pub struct RevisionRecord {
    #[key]
    pub id: Uuid,
    pub document_id: Uuid,
    pub number: u32,
    pub blob_hash: String,
    pub message: String,
    #[index]
    pub author_name: String,
    pub source: String,
    pub restored_from_number: Option<u32>,
    pub created_at: Timestamp,
}

#[derive(Debug, toasty::Model)]
#[table = "blobs"]
pub struct BlobRecord {
    #[key]
    pub hash: String,
    pub size: u64,
    pub created_at: Timestamp,
}
