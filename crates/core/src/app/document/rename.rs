use std::sync::Arc;

use super::load;
use crate::{
    app::error::AppError,
    domain::{
        document::{Document, DocumentRepository, DocumentTitle},
        shared::DocumentId,
    },
};

pub struct RenameDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl RenameDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId, title: &str) -> Result<Document, AppError> {
        let title = DocumentTitle::parse(title)?;
        let mut document = load(self.documents.as_ref(), id).await?;
        document.rename(title);
        self.documents.update(&document).await?;
        Ok(document)
    }
}
