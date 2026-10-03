//! Toasty のモデルと domain の型の変換。

use crate::domain::{
    blob::BlobHash,
    document::{Document, DocumentSlug, DocumentTitle},
    error::RepositoryError,
    project::{CrestColor, Project, ProjectDescription, ProjectName, ProjectSlug},
    revision::{AuthorName, Revision, RevisionMessage, RevisionNumber, RevisionSource},
    shared::{DocumentId, ProjectId, RevisionId},
};

use super::model::{DocumentRecord, ProjectRecord, RevisionRecord};

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

impl TryFrom<RevisionRecord> for Revision {
    type Error = RepositoryError;

    fn try_from(record: RevisionRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            id: RevisionId::from_uuid(record.id),
            document_id: DocumentId::from_uuid(record.document_id),
            number: RevisionNumber::new(record.number)
                .map_err(|e| corrupted("revisions.number", e))?,
            blob_hash: BlobHash::parse(&record.blob_hash)
                .map_err(|e| corrupted("revisions.blob_hash", e))?,
            message: RevisionMessage::parse(&record.message)
                .map_err(|e| corrupted("revisions.message", e))?,
            author_name: AuthorName::parse(&record.author_name)
                .map_err(|e| corrupted("revisions.author_name", e))?,
            source: RevisionSource::parse(&record.source)
                .map_err(|e| corrupted("revisions.source", e))?,
            restored_from: record
                .restored_from_number
                .map(RevisionNumber::new)
                .transpose()
                .map_err(|e| corrupted("revisions.restored_from_number", e))?,
            created_at: record.created_at,
        })
    }
}
