use std::sync::Arc;

use crate::{
    app::error::AppError,
    domain::document::{DocumentListItem, DocumentQuery, DocumentRepository},
};

/// 資料の一覧（すべての資料・資料一覧・⌘K の検索）。
pub struct ListDocuments {
    documents: Arc<dyn DocumentRepository>,
}

impl ListDocuments {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, query: &DocumentQuery) -> Result<Vec<DocumentListItem>, AppError> {
        Ok(self.documents.list(query).await?)
    }
}
