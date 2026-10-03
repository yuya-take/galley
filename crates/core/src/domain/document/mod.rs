//! 資料。版（リビジョン）の積み重ねで、`current_revision_id` が「現在版」を指す。

mod query;
mod repository;
mod slug;
mod title;

use jiff::Timestamp;

pub use query::{DocumentOrder, DocumentQuery};
pub use repository::DocumentRepository;
pub use slug::{DocumentSlug, RESERVED_DOCUMENT_SLUGS};
pub use title::{DOCUMENT_TITLE_MAX_CHARS, DocumentTitle, UNTITLED_DOCUMENT};

use crate::domain::{
    project::{CrestColor, ProjectName, ProjectSlug},
    revision::{
        AuthorName, NewRevision, Revision, RevisionMessage, RevisionNumber, RevisionSource,
    },
    shared::{DocumentId, ProjectId, RevisionId},
};

/// 資料に版を追加できないとき。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("アーカイブした資料には版を追加できません。先にアーカイブから戻してください")]
    Archived,
    #[error("第{0}版は最新の版なので、戻せません")]
    AlreadyLatest(RevisionNumber),
    /// 渡された版が、この資料の現在版や版ではない（呼び出し側の誤り）。
    #[error("ほかの資料の版です")]
    RevisionMismatch,
}

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

    /// 新しい版を追加できるか。アーカイブした資料には追加できない。
    pub fn ensure_accepts_revisions(&self) -> Result<(), DocumentError> {
        if self.is_archived() {
            return Err(DocumentError::Archived);
        }
        Ok(())
    }

    /// 現在版の次の番号で新しい版を作り、現在版にする。
    ///
    /// 番号は現在版の番号 + 1。同時に別の版が追加されていれば保存のときに番号が重なるので、
    /// 呼び出し側は資料を読み直してやり直す。
    pub fn add_revision(
        &mut self,
        current: &Revision,
        new: NewRevision,
        now: Timestamp,
    ) -> Result<Revision, DocumentError> {
        self.ensure_accepts_revisions()?;
        if current.id != self.current_revision_id || current.document_id != self.id {
            return Err(DocumentError::RevisionMismatch);
        }
        let NewRevision {
            blob_hash,
            message,
            author_name,
            source,
        } = new;
        let revision = Revision {
            id: RevisionId::generate(),
            document_id: self.id,
            number: current.number.next(),
            blob_hash,
            message,
            author_name,
            source,
            restored_from: None,
            created_at: now,
        };
        self.current_revision_id = revision.id;
        self.updated_at = now;
        Ok(revision)
    }

    /// 過去の版に戻す。履歴は書き換えず、過去の版と同じ内容の新しい版を追加する。
    /// 変更メモは「第2版の内容に戻す」にする。最新の版には戻せない。
    pub fn revert_to(
        &mut self,
        current: &Revision,
        target: &Revision,
        author_name: AuthorName,
        source: RevisionSource,
        now: Timestamp,
    ) -> Result<Revision, DocumentError> {
        if target.document_id != self.id {
            return Err(DocumentError::RevisionMismatch);
        }
        if target.id == self.current_revision_id {
            return Err(DocumentError::AlreadyLatest(target.number));
        }
        let mut revision = self.add_revision(
            current,
            NewRevision {
                blob_hash: target.blob_hash.clone(),
                message: RevisionMessage::reverted_to(target.number),
                author_name,
                source,
            },
            now,
        )?;
        revision.restored_from = Some(target.number);
        Ok(revision)
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

    fn new_document() -> (Document, Revision) {
        Document::create(
            ProjectId::generate(),
            DocumentSlug::parse("plan").unwrap(),
            DocumentTitle::parse("事業計画").unwrap(),
            first_revision(),
            Timestamp::from_second(100).unwrap(),
        )
    }

    fn second_revision() -> NewRevision {
        NewRevision {
            blob_hash: BlobHash::parse(&"b".repeat(64)).unwrap(),
            message: RevisionMessage::parse("数字を更新").unwrap(),
            author_name: AuthorName::parse("田中").unwrap(),
            source: RevisionSource::Mcp,
        }
    }

    #[test]
    fn add_revision_numbers_next_and_becomes_current() {
        let (mut document, first) = new_document();
        let now = Timestamp::from_second(200).unwrap();
        let second = document
            .add_revision(&first, second_revision(), now)
            .unwrap();
        assert_eq!(second.number.get(), 2);
        assert_eq!(second.document_id, document.id);
        assert_eq!(second.restored_from, None);
        assert_eq!(document.current_revision_id, second.id);
        assert_eq!(document.updated_at, now);
    }

    #[test]
    fn add_revision_rejects_archived_document() {
        let (mut document, first) = new_document();
        document.archive(Timestamp::from_second(150).unwrap());
        assert_eq!(
            document.add_revision(
                &first,
                second_revision(),
                Timestamp::from_second(200).unwrap()
            ),
            Err(DocumentError::Archived)
        );
    }

    #[test]
    fn add_revision_rejects_stale_current() {
        let (mut document, first) = new_document();
        let now = Timestamp::from_second(200).unwrap();
        document
            .add_revision(&first, second_revision(), now)
            .unwrap();
        assert_eq!(
            document.add_revision(&first, second_revision(), now),
            Err(DocumentError::RevisionMismatch)
        );
    }

    #[test]
    fn revert_adds_copy_of_past_revision() {
        let (mut document, first) = new_document();
        let second = document
            .add_revision(
                &first,
                second_revision(),
                Timestamp::from_second(200).unwrap(),
            )
            .unwrap();
        let now = Timestamp::from_second(300).unwrap();
        let third = document
            .revert_to(
                &second,
                &first,
                AuthorName::parse("鈴木").unwrap(),
                RevisionSource::Web,
                now,
            )
            .unwrap();
        assert_eq!(third.number.get(), 3);
        assert_eq!(third.blob_hash, first.blob_hash);
        assert_eq!(third.restored_from, Some(RevisionNumber::FIRST));
        assert_eq!(third.message.as_str(), "第1版の内容に戻す");
        assert_eq!(third.author_name.as_str(), "鈴木");
        assert_eq!(document.current_revision_id, third.id);
        assert_eq!(document.updated_at, now);
    }

    #[test]
    fn revert_to_latest_is_rejected() {
        let (mut document, first) = new_document();
        assert_eq!(
            document.revert_to(
                &first,
                &first,
                AuthorName::parse("鈴木").unwrap(),
                RevisionSource::Web,
                Timestamp::from_second(300).unwrap()
            ),
            Err(DocumentError::AlreadyLatest(RevisionNumber::FIRST))
        );
    }

    #[test]
    fn revert_rejects_revision_of_other_document() {
        let (mut document, first) = new_document();
        let (_, other) = new_document();
        assert_eq!(
            document.revert_to(
                &first,
                &other,
                AuthorName::parse("鈴木").unwrap(),
                RevisionSource::Web,
                Timestamp::from_second(300).unwrap()
            ),
            Err(DocumentError::RevisionMismatch)
        );
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
