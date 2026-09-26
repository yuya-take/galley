//! 資料。版（リビジョン）の積み重ねで、`current_revision_id` が「現在版」を指す。

use jiff::Timestamp;

use super::{
    crest_color::CrestColor,
    id::{DocumentId, ProjectId, RevisionId},
    revision::{NewRevision, Revision, RevisionNumber},
    slug::{DocumentSlug, ProjectSlug},
    text::{AuthorName, DocumentTitle, ProjectName, RevisionMessage},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: DocumentId,
    pub project_id: ProjectId,
    pub slug: DocumentSlug,
    pub title: DocumentTitle,
    pub current_revision_id: RevisionId,
    pub archived_at: Option<Timestamp>,
    pub created_at: Timestamp,
    /// 最後に版を追加した日時。資料名の変更やアーカイブでは変わらない。
    pub updated_at: Timestamp,
}

impl Document {
    /// 資料と第1版を作る。
    pub fn create(
        project_id: ProjectId,
        slug: DocumentSlug,
        title: DocumentTitle,
        first: NewRevision,
        now: Timestamp,
    ) -> (Self, Revision) {
        let id = DocumentId::generate();
        let NewRevision {
            blob_hash,
            message,
            author_name,
            source,
        } = first;
        let revision = Revision {
            id: RevisionId::generate(),
            document_id: id,
            number: RevisionNumber::FIRST,
            blob_hash,
            message,
            author_name,
            source,
            restored_from: None,
            created_at: now,
        };
        let document = Self {
            id,
            project_id,
            slug,
            title,
            current_revision_id: revision.id,
            archived_at: None,
            created_at: now,
            updated_at: now,
        };
        (document, revision)
    }

    pub fn rename(&mut self, title: DocumentTitle) {
        self.title = title;
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// アーカイブする。すでにアーカイブ済みなら日時を変えない。
    pub fn archive(&mut self, now: Timestamp) {
        self.archived_at.get_or_insert(now);
    }

    pub fn restore(&mut self) {
        self.archived_at = None;
    }
}

/// 資料の一覧（すべての資料・資料一覧）の1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentListItem {
    pub id: DocumentId,
    pub slug: DocumentSlug,
    pub title: DocumentTitle,
    pub project_id: ProjectId,
    pub project_slug: ProjectSlug,
    pub project_name: ProjectName,
    pub project_color: CrestColor,
    /// 現在版の番号（「第5版」）。
    pub revision_number: RevisionNumber,
    /// 現在版の変更メモ。
    pub revision_message: RevisionMessage,
    /// 現在版の更新者。
    pub author_name: AuthorName,
    pub archived_at: Option<Timestamp>,
    pub updated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{blob::BlobHash, revision::RevisionSource};

    fn first_revision() -> NewRevision {
        NewRevision {
            blob_hash: BlobHash::parse(&"a".repeat(64)).unwrap(),
            message: RevisionMessage::parse("初版").unwrap(),
            author_name: AuthorName::parse("佐藤").unwrap(),
            source: RevisionSource::Web,
        }
    }

    #[test]
    fn create_points_current_revision_to_first() {
        let now = Timestamp::from_second(100).unwrap();
        let (document, revision) = Document::create(
            ProjectId::generate(),
            DocumentSlug::parse("plan").unwrap(),
            DocumentTitle::parse("事業計画").unwrap(),
            first_revision(),
            now,
        );
        assert_eq!(document.current_revision_id, revision.id);
        assert_eq!(revision.document_id, document.id);
        assert_eq!(revision.number, RevisionNumber::FIRST);
        assert_eq!(revision.restored_from, None);
        assert_eq!(document.updated_at, now);
        assert_eq!(revision.created_at, now);
    }

    #[test]
    fn rename_and_archive_keep_updated_at() {
        let now = Timestamp::from_second(100).unwrap();
        let (mut document, _) = Document::create(
            ProjectId::generate(),
            DocumentSlug::parse("plan").unwrap(),
            DocumentTitle::parse("事業計画").unwrap(),
            first_revision(),
            now,
        );
        document.rename(DocumentTitle::parse("事業計画 2026").unwrap());
        document.archive(Timestamp::from_second(200).unwrap());
        assert!(document.is_archived());
        assert_eq!(document.updated_at, now);

        document.restore();
        assert!(!document.is_archived());
    }
}
