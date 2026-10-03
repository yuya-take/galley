//! 資料のユースケース。新しい版の追加と「この版に戻す」は `revision`。

mod archive;
mod choose_slug;
mod create;
mod find;
mod list;
mod rename;

pub use archive::{ArchiveDocument, RestoreDocument};
pub use choose_slug::{ChooseDocumentSlug, SlugSource};
pub use create::{CreateDocument, CreateDocumentInput};
pub use find::{FindDocument, FindDocumentByFileName};
pub use list::ListDocuments;
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
