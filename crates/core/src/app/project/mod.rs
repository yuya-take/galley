//! プロジェクトのユースケース。

mod archive;
mod create;
mod find;
mod list;
mod update;

pub use archive::{ArchiveProject, RestoreProject};
pub use create::CreateProject;
pub use find::FindProject;
pub use list::ListProjects;
pub use update::UpdateProject;

use crate::domain::{
    error::RepositoryError,
    project::{
        CrestColor, Project, ProjectDescription, ProjectName, ProjectProfile, ProjectRepository,
        ProjectSlug,
    },
    shared::ProjectId,
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

pub(super) async fn load(
    projects: &dyn ProjectRepository,
    id: ProjectId,
) -> Result<Project, AppError> {
    projects
        .find_by_id(id)
        .await?
        .ok_or(AppError::ProjectNotFound)
}
