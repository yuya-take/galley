use std::sync::Arc;

use super::{ProjectInput, load, slug_conflict};
use crate::{
    app::error::AppError,
    domain::{
        project::{Project, ProjectProfile, ProjectRepository},
        shared::ProjectId,
    },
};

/// プロジェクト設定（名前、URL、説明、紋章の色）を変える。
pub struct UpdateProject {
    projects: Arc<dyn ProjectRepository>,
}

impl UpdateProject {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, id: ProjectId, input: &ProjectInput) -> Result<Project, AppError> {
        let profile = ProjectProfile::try_from(input)?;
        let mut project = load(self.projects.as_ref(), id).await?;
        if let Some(other) = self.projects.find_by_slug(&profile.slug).await?
            && other.id != id
        {
            return Err(AppError::ProjectSlugTaken(profile.slug.to_string()));
        }
        project.edit(profile);
        self.projects
            .update(&project)
            .await
            .map_err(slug_conflict(&project.slug))?;
        Ok(project)
    }
}
