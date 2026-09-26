use std::sync::Arc;

use crate::{
    app::error::AppError,
    domain::{
        project::{ProjectRepository, ProjectSummary},
        shared::ArchiveFilter,
    },
};

/// サイドバーとアーカイブ画面のプロジェクト一覧。
pub struct ListProjects {
    projects: Arc<dyn ProjectRepository>,
}

impl ListProjects {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, filter: ArchiveFilter) -> Result<Vec<ProjectSummary>, AppError> {
        Ok(self.projects.list(filter).await?)
    }
}
