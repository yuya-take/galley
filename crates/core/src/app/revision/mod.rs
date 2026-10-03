//! 版のユースケース（新しい版の追加、過去の版に戻す、版の HTML を読む）。新しい資料の第1版は `document::CreateDocument`。

mod add;
mod read;
mod revert;

pub use add::AddRevision;
pub use read::{ReadRevisionContent, RevisionContent};
pub use revert::RevertToRevision;

use jiff::Timestamp;

use crate::domain::{
    blob::{Blob, BlobContent, BlobStore},
    document::Document,
    error::RepositoryError,
    revision::{
        AuthorName, NewRevision, Revision, RevisionMessage, RevisionRepository, RevisionSource,
    },
    upload::HtmlDocument,
};

use super::error::AppError;

/// 同時に別の版が追加されて番号が重なったとき、読み直してやり直す回数。
///
/// やり直すのはほかの更新が先に登録できたときだけなので、同じ資料への同時の更新が
/// この回数までなら、すべて登録できる。それを超えた分は `RepositoryError::Conflict` になる。
const MAX_ATTEMPTS: usize = 5;

/// 画面と MCP から受け取る、版の内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionInput {
    /// アップロードされた HTML。
    pub html: Vec<u8>,
    pub message: String,
    pub author_name: String,
    pub source: RevisionSource,
}

/// 登録した資料と版。画面と MCP はここから資料ビューアの URL と「第N版として登録しました」を作る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredRevision {
    pub document: Document,
    pub revision: Revision,
}

/// 検査を通した、保存する版の内容。
pub(super) struct PreparedRevision {
    pub content: BlobContent,
    pub message: RevisionMessage,
    pub author_name: AuthorName,
    pub source: RevisionSource,
    /// HTML の `<title>`（新しい資料の資料名の初期値）。
    pub html_title: Option<String>,
}

impl PreparedRevision {
    /// 入力を検査する。HTML の検査（外部リソースの検出）より先に、軽い検査を済ませる。
    pub fn parse(input: RevisionInput) -> Result<Self, AppError> {
        let author_name = AuthorName::parse(&input.author_name)?;
        let message = RevisionMessage::parse(&input.message)?;
        let html = HtmlDocument::parse(input.html)?;
        let html_title = html.title().map(str::to_owned);
        Ok(Self {
            content: BlobContent::new(html.into_bytes()),
            message,
            author_name,
            source: input.source,
            html_title,
        })
    }

    /// 実体をストレージに書き、DB に記録するブロブの情報を返す。
    /// DB より先に書くので「DB にあるのに実体がない」状態にはならない。
    pub async fn store(&self, blobs: &dyn BlobStore, now: Timestamp) -> Result<Blob, AppError> {
        blobs.put(&self.content).await?;
        Ok(self.content.to_blob(now))
    }

    pub fn new_revision(&self) -> NewRevision {
        NewRevision {
            blob_hash: self.content.hash().clone(),
            message: self.message.clone(),
            author_name: self.author_name.clone(),
            source: self.source,
        }
    }
}

/// 資料の現在版を読む。
pub(super) async fn load_current(
    revisions: &dyn RevisionRepository,
    document: &Document,
) -> Result<Revision, AppError> {
    revisions
        .find_by_id(document.current_revision_id)
        .await?
        .ok_or_else(|| {
            RepositoryError::Corrupted(format!(
                "documents.current_revision_id: 資料 {} の現在版がありません",
                document.id
            ))
            .into()
        })
}

/// 保存の結果が「番号が重なった」なら、やり直すかどうか。
pub(super) fn should_retry(result: &Result<(), RepositoryError>, attempt: usize) -> bool {
    matches!(result, Err(RepositoryError::Conflict(_))) && attempt < MAX_ATTEMPTS
}
