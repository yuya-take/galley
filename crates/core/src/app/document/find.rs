use std::sync::Arc;

use super::load;
use crate::{
    app::error::AppError,
    domain::{
        document::{Document, DocumentRepository, DocumentSlug},
        shared::{DocumentId, ProjectId},
    },
};

/// URL の slug から資料を探す（アーカイブしたものも返す）。
pub struct FindDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl FindDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, project_id: ProjectId, slug: &str) -> Result<Document, AppError> {
        let slug = DocumentSlug::parse(slug).map_err(|_| AppError::DocumentNotFound)?;
        self.documents
            .find_by_slug(project_id, &slug)
            .await?
            .ok_or(AppError::DocumentNotFound)
    }
}

/// ID から資料を探す（アーカイブしたものも返す）。資料配信が、版の HTML をダウンロードするときの
/// ファイル名（資料の slug）を知るのに使う。
pub struct FindDocumentById {
    documents: Arc<dyn DocumentRepository>,
}

impl FindDocumentById {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId) -> Result<Document, AppError> {
        load(self.documents.as_ref(), id).await
    }
}

/// ファイル名が一致する資料を探す（登録の確認ダイアログの「ファイル名が一致」）。
/// アーカイブした資料は候補にしない。
pub struct FindDocumentByFileName {
    documents: Arc<dyn DocumentRepository>,
}

impl FindDocumentByFileName {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(
        &self,
        project_id: ProjectId,
        file_name: &str,
    ) -> Result<Option<Document>, AppError> {
        let Some(slug) = DocumentSlug::from_file_name(file_name) else {
            return Ok(None);
        };
        let document = self.documents.find_by_slug(project_id, &slug).await?;
        Ok(document.filter(|document| !document.is_archived()))
    }
}
