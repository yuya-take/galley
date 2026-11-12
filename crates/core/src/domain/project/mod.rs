//! プロジェクト。資料をまとめる入れ物で、アクセス制御の単位ではない。

mod crest_color;
mod repository;
mod slug;
mod text;

use jiff::Timestamp;

pub use crest_color::{CrestColor, CrestColorError};
pub use repository::ProjectRepository;
pub use slug::{ProjectSlug, RESERVED_PROJECT_SLUGS};
pub use text::{
    PROJECT_DESCRIPTION_MAX_CHARS, PROJECT_NAME_MAX_CHARS, ProjectDescription, ProjectName,
};

use crate::domain::shared::ProjectId;

/// プロジェクトに資料を登録できない・資料に版を追加できない・資料をアーカイブできないとき。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    #[error(
        "アーカイブしたプロジェクトには、資料の登録も新しい版の追加もできません。先にプロジェクトをアーカイブから戻してください"
    )]
    Archived,
    /// アーカイブしたプロジェクトの資料を、1件ずつアーカイブしようとした。
    #[error(
        "アーカイブしたプロジェクトの資料は、1件ずつはアーカイブできません。プロジェクトごとアーカイブしてあるので、資料はすでに一覧に出ていません"
    )]
    ArchivedWithDocuments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: ProjectId,
    pub slug: ProjectSlug,
    pub name: ProjectName,
    pub description: ProjectDescription,
    pub color: CrestColor,
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

/// プロジェクト設定で変えられる項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProfile {
    pub slug: ProjectSlug,
    pub name: ProjectName,
    pub description: ProjectDescription,
    pub color: CrestColor,
}

impl Project {
    pub fn create(profile: ProjectProfile, now: Timestamp) -> Self {
        let ProjectProfile {
            slug,
            name,
            description,
            color,
        } = profile;
        Self {
            id: ProjectId::generate(),
            slug,
            name,
            description,
            color,
            archived_at: None,
            created_at: now,
        }
    }

    pub fn edit(&mut self, profile: ProjectProfile) {
        let ProjectProfile {
            slug,
            name,
            description,
            color,
        } = profile;
        self.slug = slug;
        self.name = name;
        self.description = description;
        self.color = color;
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// 新しい資料の登録と、資料への新しい版の追加（過去の版に戻すことも含む）を受け付けるか。
    /// アーカイブしたプロジェクトでは、どちらもできない。
    pub fn ensure_accepts_documents(&self) -> Result<(), ProjectError> {
        if self.is_archived() {
            return Err(ProjectError::Archived);
        }
        Ok(())
    }

    /// 資料を1件ずつアーカイブできるか。アーカイブしたプロジェクトでは、資料はプロジェクトごと一覧に出ていないので断る。
    pub fn ensure_documents_archivable(&self) -> Result<(), ProjectError> {
        if self.is_archived() {
            return Err(ProjectError::ArchivedWithDocuments);
        }
        Ok(())
    }

    /// アーカイブする。すでにアーカイブ済みなら日時を変えない。
    pub fn archive(&mut self, now: Timestamp) {
        self.archived_at.get_or_insert(now);
    }

    pub fn restore(&mut self) {
        self.archived_at = None;
    }
}

/// サイドバーのプロジェクト一覧の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub project: Project,
    /// アーカイブしていない資料の件数。
    pub document_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(slug: &str) -> ProjectProfile {
        ProjectProfile {
            slug: ProjectSlug::parse(slug).unwrap(),
            name: ProjectName::parse("営業部").unwrap(),
            description: ProjectDescription::parse("").unwrap(),
            color: CrestColor::Red,
        }
    }

    #[test]
    fn archive_keeps_first_timestamp() {
        let mut project = Project::create(profile("sales"), Timestamp::UNIX_EPOCH);
        assert!(!project.is_archived());

        let first = Timestamp::from_second(100).unwrap();
        project.archive(first);
        project.archive(Timestamp::from_second(200).unwrap());
        assert_eq!(project.archived_at, Some(first));

        project.restore();
        assert!(!project.is_archived());
    }

    #[test]
    fn archived_project_rejects_documents() {
        let mut project = Project::create(profile("sales"), Timestamp::UNIX_EPOCH);
        assert_eq!(project.ensure_accepts_documents(), Ok(()));

        project.archive(Timestamp::from_second(100).unwrap());
        assert_eq!(
            project.ensure_accepts_documents(),
            Err(ProjectError::Archived)
        );

        project.restore();
        assert_eq!(project.ensure_accepts_documents(), Ok(()));
    }

    #[test]
    fn archived_project_rejects_archiving_documents() {
        let mut project = Project::create(profile("sales"), Timestamp::UNIX_EPOCH);
        assert_eq!(project.ensure_documents_archivable(), Ok(()));

        project.archive(Timestamp::from_second(100).unwrap());
        assert_eq!(
            project.ensure_documents_archivable(),
            Err(ProjectError::ArchivedWithDocuments)
        );
    }

    #[test]
    fn edit_replaces_profile() {
        let mut project = Project::create(profile("sales"), Timestamp::UNIX_EPOCH);
        let id = project.id;
        project.edit(profile("sales-2026"));
        assert_eq!(project.slug.as_str(), "sales-2026");
        assert_eq!(project.id, id);
    }
}
