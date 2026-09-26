//! 資料のユースケース（版の登録と「この版に戻す」は #9）。

mod archive;
mod choose_slug;
mod find;
mod list;
mod rename;

pub use archive::{ArchiveDocument, RestoreDocument};
pub use choose_slug::{ChooseDocumentSlug, SlugSource};
pub use find::{FindDocument, FindDocumentByFileName};
pub use list::ListDocuments;
pub use rename::RenameDocument;

use crate::domain::{
    document::{Document, DocumentRepository},
    shared::DocumentId,
};

use super::error::AppError;

async fn load(documents: &dyn DocumentRepository, id: DocumentId) -> Result<Document, AppError> {
    documents
        .find_by_id(id)
        .await?
        .ok_or(AppError::DocumentNotFound)
}
