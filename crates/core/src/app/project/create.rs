use std::sync::Arc;

use jiff::Timestamp;

use super::{ProjectInput, slug_conflict};
use crate::{
    app::error::AppError,
    domain::project::{Project, ProjectProfile, ProjectRepository},
};

pub struct CreateProject {
    projects: Arc<dyn ProjectRepository>,
}

impl CreateProject {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
    }

    pub async fn execute(&self, input: &ProjectInput) -> Result<Project, AppError> {
        let profile = ProjectProfile::try_from(input)?;
        if self.projects.find_by_slug(&profile.slug).await?.is_some() {
            return Err(AppError::ProjectSlugTaken(profile.slug.to_string()));
        }
        let project = Project::create(profile, Timestamp::now());
        self.projects
            .insert(&project)
            .await
            .map_err(slug_conflict(&project.slug))?;
        Ok(project)
    }
}
