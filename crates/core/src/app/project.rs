//! プロジェクトのユースケース。

use std::sync::Arc;

use jiff::Timestamp;

use crate::domain::{
    crest_color::CrestColor,
    id::ProjectId,
    project::{Project, ProjectProfile, ProjectSummary},
    repository::{ArchiveFilter, ProjectRepository, RepositoryError},
    slug::ProjectSlug,
    text::{ProjectDescription, ProjectName},
};

use super::error::AppError;

/// プロジェクトの作成・設定画面から受け取る値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInput {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub color: String,
}

impl TryFrom<&ProjectInput> for ProjectProfile {
    type Error = AppError;

    fn try_from(input: &ProjectInput) -> Result<Self, Self::Error> {
        Ok(Self {
            slug: ProjectSlug::parse(&input.slug)?,
            name: ProjectName::parse(&input.name)?,
            description: ProjectDescription::parse(&input.description)?,
            color: CrestColor::parse(&input.color)?,
        })
    }
}

/// 一意制約の違反を「URL が使われている」に変換する。
fn slug_conflict(slug: &ProjectSlug) -> impl FnOnce(RepositoryError) -> AppError + '_ {
    move |err| match err {
        RepositoryError::Conflict(_) => AppError::ProjectSlugTaken(slug.to_string()),
        err => err.into(),
    }
}

async fn load(projects: &dyn ProjectRepository, id: ProjectId) -> Result<Project, AppError> {
    projects
        .find_by_id(id)
        .await?
        .ok_or(AppError::ProjectNotFound)
}

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
