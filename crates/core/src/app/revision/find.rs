use std::sync::Arc;

use super::load_current;
use crate::{
    app::error::AppError,
    domain::{
        document::Document,
        revision::{Revision, RevisionNumber, RevisionRepository},
    },
};

/// 資料の版を探す。番号を指定しなければ現在版（最新）。
pub struct FindRevision {
    revisions: Arc<dyn RevisionRepository>,
}

impl FindRevision {
    pub fn new(revisions: Arc<dyn RevisionRepository>) -> Self {
        Self { revisions }
    }

    pub async fn execute(
        &self,
        document: &Document,
        number: Option<u32>,
    ) -> Result<Revision, AppError> {
        let Some(number) = number else {
            return load_current(self.revisions.as_ref(), document).await;
        };
        let number = RevisionNumber::new(number).map_err(|_| AppError::RevisionNotFound)?;
        self.revisions
            .find_by_number(document.id, number)
            .await?
            .ok_or(AppError::RevisionNotFound)
    }
}
