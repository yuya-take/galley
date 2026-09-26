use std::sync::Arc;

use jiff::Timestamp;

use super::load;
use crate::{
    app::error::AppError,
    domain::{
        project::{Project, ProjectRepository},
        shared::ProjectId,
    },
};

pub struct ArchiveProject {
    projects: Arc<dyn ProjectRepository>,
}

impl ArchiveProject {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, id: ProjectId) -> Result<Project, AppError> {
        let mut project = load(self.projects.as_ref(), id).await?;
        project.archive(Timestamp::now());
        self.projects.update(&project).await?;
        Ok(project)
    }
}

/// アーカイブしたプロジェクトを元に戻す。
pub struct RestoreProject {
    projects: Arc<dyn ProjectRepository>,
}

impl RestoreProject {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, id: ProjectId) -> Result<Project, AppError> {
        let mut project = load(self.projects.as_ref(), id).await?;
        project.restore();
        self.projects.update(&project).await?;
        Ok(project)
    }
}
