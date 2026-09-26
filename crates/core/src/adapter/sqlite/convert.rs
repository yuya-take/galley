//! Toasty のモデルと domain の型の変換。

use crate::domain::{
    document::{Document, DocumentSlug, DocumentTitle},
    error::RepositoryError,
    project::{CrestColor, Project, ProjectDescription, ProjectName, ProjectSlug},
    shared::{DocumentId, ProjectId, RevisionId},
};

use super::model::{DocumentRecord, ProjectRecord};

/// 保存済みの値を domain の型に読み直せなかったときのエラー。
pub(super) fn corrupted(column: &str, err: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::Corrupted(format!("{column}: {err}"))
}

impl TryFrom<ProjectRecord> for Project {
    type Error = RepositoryError;

    fn try_from(record: ProjectRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ProjectId::from_uuid(record.id),
            slug: ProjectSlug::parse(&record.slug).map_err(|e| corrupted("projects.slug", e))?,
            name: ProjectName::parse(&record.name).map_err(|e| corrupted("projects.name", e))?,
            description: ProjectDescription::parse(&record.description)
                .map_err(|e| corrupted("projects.description", e))?,
            color: CrestColor::parse(&record.color).map_err(|e| corrupted("projects.color", e))?,
            archived_at: record.archived_at,
            created_at: record.created_at,
        })
    }
}

impl TryFrom<DocumentRecord> for Document {
    type Error = RepositoryError;

    fn try_from(record: DocumentRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: DocumentId::from_uuid(record.id),
            project_id: ProjectId::from_uuid(record.project_id),
            slug: DocumentSlug::parse(&record.slug).map_err(|e| corrupted("documents.slug", e))?,
            title: DocumentTitle::parse(&record.title)
                .map_err(|e| corrupted("documents.title", e))?,
            current_revision_id: RevisionId::from_uuid(record.current_revision_id),
            archived_at: record.archived_at,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}
