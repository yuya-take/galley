use std::sync::Arc;

use crate::{
    app::error::AppError,
    domain::{
        document::Document,
        revision::{Revision, RevisionRepository},
    },
};

/// 資料の版の一覧（新しい順）。プレビュー欄の直近の版、版の履歴で使う。
pub struct ListRevisions {
    revisions: Arc<dyn RevisionRepository>,
}

impl ListRevisions {
    pub fn new(revisions: Arc<dyn RevisionRepository>) -> Self {
        Self { revisions }
    }

    /// `limit` を指定すれば、新しいほうからその件数まで。
    pub async fn execute(
        &self,
        document: &Document,
        limit: Option<u32>,
    ) -> Result<Vec<Revision>, AppError> {
        Ok(self.revisions.list_by_document(document.id, limit).await?)
    }
}
