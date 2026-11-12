use async_trait::async_trait;

use crate::domain::{
    blob::BlobHash,
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

    /// blob_hash に索引は無いので、版の表を順に読む。物理削除（管理用のコマンド）でしか使わず、
    /// 版の行は多くないので、索引は足さない。
    async fn list_by_blob_hashes(
        &self,
        hashes: &[BlobHash],
    ) -> Result<Vec<Revision>, RepositoryError> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }
        let mut db = self.db.clone();
        let hashes: Vec<String> = hashes.iter().map(|hash| hash.as_str().to_owned()).collect();
        let fields = RevisionRecord::fields();
        RevisionRecord::filter(fields.blob_hash().in_list(hashes))
            .order_by(fields.number().desc())
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .into_iter()
            .map(Revision::try_from)
            .collect()
    }
}
