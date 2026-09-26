use async_trait::async_trait;

use super::{Project, ProjectSlug, ProjectSummary};
use crate::domain::{
    error::RepositoryError,
    shared::{ArchiveFilter, ProjectId},
};

/// プロジェクトの永続化。実装は `adapter`。
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn insert(&self, project: &Project) -> Result<(), RepositoryError>;

    async fn update(&self, project: &Project) -> Result<(), RepositoryError>;

    async fn find_by_id(&self, id: ProjectId) -> Result<Option<Project>, RepositoryError>;

    async fn find_by_slug(&self, slug: &ProjectSlug) -> Result<Option<Project>, RepositoryError>;

    /// 作成順に返す（名前順は漢字の読みの順にならないため）。
    /// 資料の件数はアーカイブしていない資料だけ数える。
    async fn list(&self, filter: ArchiveFilter) -> Result<Vec<ProjectSummary>, RepositoryError>;
}
