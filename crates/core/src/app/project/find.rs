use std::sync::Arc;

use crate::{
    app::error::AppError,
    domain::project::{Project, ProjectRepository, ProjectSlug},
};

/// URL の slug からプロジェクトを探す（アーカイブしたものも返す）。
pub struct FindProject {
    projects: Arc<dyn ProjectRepository>,
}

impl FindProject {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, slug: &str) -> Result<Project, AppError> {
        let slug = ProjectSlug::parse(slug).map_err(|_| AppError::ProjectNotFound)?;
        self.projects
            .find_by_slug(&slug)
            .await?
            .ok_or(AppError::ProjectNotFound)
    }
}
