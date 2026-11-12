use std::sync::Arc;

use jiff::Timestamp;

use super::load;
use crate::{
    app::error::AppError,
    domain::{
        document::{Document, DocumentRepository},
        shared::DocumentId,
    },
};

/// 資料をアーカイブする。資料が無い、資料が属するプロジェクトがアーカイブ済み、資料がアーカイブ済みのどれかなら断る。
pub struct ArchiveDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl ArchiveDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId) -> Result<Document, AppError> {
        let (mut document, project) = self
            .documents
            .find_with_project(id)
            .await?
            .ok_or(AppError::DocumentNotFound)?;
        project.ensure_documents_archivable()?;
        document.archive(Timestamp::now())?;
        self.documents.update(&document).await?;
        Ok(document)
    }
}

/// アーカイブした資料を元に戻す。
pub struct RestoreDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl RestoreDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId) -> Result<Document, AppError> {
        let mut document = load(self.documents.as_ref(), id).await?;
        document.restore();
        self.documents.update(&document).await?;
        Ok(document)
    }
}
