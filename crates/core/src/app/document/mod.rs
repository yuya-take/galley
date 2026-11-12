//! 資料のユースケース。新しい版の追加と「この版に戻す」は `revision`。
//! 物理削除（[`PurgeDocument`]）は、コンテナ内で実行するコマンドだけが使う。

mod archive;
mod choose_slug;
mod create;
mod find;
mod list;
mod purge;
mod rename;

pub use archive::{ArchiveDocument, RestoreDocument};
pub use choose_slug::{ChooseDocumentSlug, SlugSource};
pub use create::{CreateDocument, CreateDocumentInput};
pub use find::{FindDocument, FindDocumentByFileName, FindDocumentById};
pub use list::ListDocuments;
pub use purge::{KeptBlob, PurgeDocument, PurgeOutcome, PurgePlan, RevisionLocation};
pub use rename::RenameDocument;

use crate::domain::{
    document::{Document, DocumentRepository},
    shared::DocumentId,
};

use super::error::AppError;

pub(super) async fn load(
    documents: &dyn DocumentRepository,
    id: DocumentId,
) -> Result<Document, AppError> {
    documents
        .find_by_id(id)
        .await?
        .ok_or(AppError::DocumentNotFound)
}
