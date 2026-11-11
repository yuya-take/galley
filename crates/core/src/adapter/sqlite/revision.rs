use async_trait::async_trait;

use crate::domain::{
    error::RepositoryError,
    revision::{Revision, RevisionNumber, RevisionRepository},
    shared::{DocumentId, RevisionId},
};

use super::{backend_error, model::RevisionRecord};

#[derive(Debug, Clone)]
pub struct ToastyRevisionRepository {
    db: toasty::Db,
}

impl ToastyRevisionRepository {
    pub(super) fn new(db: toasty::Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RevisionRepository for ToastyRevisionRepository {
    async fn find_by_id(&self, id: RevisionId) -> Result<Option<Revision>, RepositoryError> {
        let mut db = self.db.clone();
        RevisionRecord::filter_by_id(id.as_uuid())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Revision::try_from)
            .transpose()
    }

    async fn find_by_number(
        &self,
        document_id: DocumentId,
        number: RevisionNumber,
    ) -> Result<Option<Revision>, RepositoryError> {
        let mut db = self.db.clone();
        RevisionRecord::filter_by_document_id_and_number(document_id.as_uuid(), number.get())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Revision::try_from)
            .transpose()
    }

    async fn list_by_document(
        &self,
        document_id: DocumentId,
        limit: Option<u32>,
    ) -> Result<Vec<Revision>, RepositoryError> {
        let mut db = self.db.clone();
        let fields = RevisionRecord::fields();
        let mut query = RevisionRecord::filter(fields.document_id().eq(document_id.as_uuid()))
            .order_by(fields.number().desc());
        if let Some(limit) = limit {
            // u32 は usize に収まる（Galley は 32 ビット以上で動かす）
            query = query.limit(limit as usize);
        }
        query
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .into_iter()
            .map(Revision::try_from)
            .collect()
    }
}
