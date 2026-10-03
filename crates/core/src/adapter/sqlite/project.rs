use std::collections::HashMap;

use async_trait::async_trait;
use toasty::stmt::{Type, Value};
use uuid::Uuid;

use crate::domain::{
    error::RepositoryError,
    project::{Project, ProjectRepository, ProjectSlug, ProjectSummary},
    shared::{ArchiveFilter, ProjectId},
};

use super::{
    WriteLock, backend_error,
    convert::corrupted,
    model::ProjectRecord,
    raw::{int, uuid},
};

#[derive(Debug, Clone)]
pub struct ToastyProjectRepository {
    db: toasty::Db,
    writes: WriteLock,
}

impl ToastyProjectRepository {
    pub(super) fn new(db: toasty::Db, writes: WriteLock) -> Self {
        Self { db, writes }
    }

    /// アーカイブしていない資料の件数をプロジェクトごとに数える。
    async fn document_counts(&self) -> Result<HashMap<Uuid, u64>, RepositoryError> {
        let mut db = self.db.clone();
        let rows = toasty::sql::query(
            "SELECT project_id, COUNT(*) FROM documents \
             WHERE archived_at IS NULL GROUP BY project_id",
        )
        .column_types([Type::Bytes, Type::I64])
        .exec(&mut db)
        .await
        .map_err(backend_error)?;

        rows.into_iter()
            .map(|row| {
                let Value::Record(row) = row else {
                    return Err(corrupted("document count", format!("{row:?}")));
                };
                let mut columns = row.fields.into_iter();
                let id = uuid(columns.next().unwrap_or_default(), "documents.project_id")?;
                let count = int(columns.next().unwrap_or_default(), "document count")?;
                let count = u64::try_from(count).map_err(|e| corrupted("document count", e))?;
                Ok((id, count))
            })
            .collect()
    }
}

#[async_trait]
impl ProjectRepository for ToastyProjectRepository {
    async fn insert(&self, project: &Project) -> Result<(), RepositoryError> {
        let _write = self.writes.acquire().await;
        let mut db = self.db.clone();
        ProjectRecord::create()
            .id(project.id.as_uuid())
            .slug(project.slug.as_str())
            .name(project.name.as_str())
            .description(project.description.as_str())
            .color(project.color.as_str())
            .archived_at(project.archived_at)
            .created_at(project.created_at)
            .exec(&mut db)
            .await
            .map_err(backend_error)?;
        Ok(())
    }

    async fn update(&self, project: &Project) -> Result<(), RepositoryError> {
        let _write = self.writes.acquire().await;
        let mut db = self.db.clone();
        ProjectRecord::filter_by_id(project.id.as_uuid())
            .update()
            .slug(project.slug.as_str())
            .name(project.name.as_str())
            .description(project.description.as_str())
            .color(project.color.as_str())
            .archived_at(project.archived_at)
            .exec(&mut db)
            .await
            .map_err(backend_error)?;
        Ok(())
    }

    async fn find_by_id(&self, id: ProjectId) -> Result<Option<Project>, RepositoryError> {
        let mut db = self.db.clone();
        ProjectRecord::filter_by_id(id.as_uuid())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Project::try_from)
            .transpose()
    }

    async fn find_by_slug(&self, slug: &ProjectSlug) -> Result<Option<Project>, RepositoryError> {
        let mut db = self.db.clone();
        ProjectRecord::filter_by_slug(slug.as_str())
            .first()
            .exec(&mut db)
            .await
            .map_err(backend_error)?
            .map(Project::try_from)
            .transpose()
    }

    async fn list(&self, filter: ArchiveFilter) -> Result<Vec<ProjectSummary>, RepositoryError> {
        let mut db = self.db.clone();
        let archived_at = ProjectRecord::fields().archived_at();
        let condition = match filter {
            ArchiveFilter::Active => archived_at.is_none(),
            ArchiveFilter::Archived => archived_at.is_some(),
        };
        let records = ProjectRecord::filter(condition)
            .order_by(ProjectRecord::fields().created_at().asc())
            .exec(&mut db)
            .await
            .map_err(backend_error)?;

        let counts = self.document_counts().await?;
        records
            .into_iter()
            .map(|record| {
                let document_count = counts.get(&record.id).copied().unwrap_or(0);
                Ok(ProjectSummary {
                    project: Project::try_from(record)?,
                    document_count,
                })
            })
            .collect()
    }
}
