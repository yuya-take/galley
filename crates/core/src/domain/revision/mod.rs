//! 版（リビジョン）。一度作ったら変更しない。

use std::fmt;

use jiff::Timestamp;

mod repository;
mod text;

pub use repository::RevisionRepository;
pub use text::{AUTHOR_NAME_MAX_CHARS, AuthorName, REVISION_MESSAGE_MAX_CHARS, RevisionMessage};

use crate::domain::{
    blob::BlobHash,
    shared::{DocumentId, RevisionId},
};

/// 版の番号（第N版）。資料ごとに 1 から連番。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RevisionNumber(u32);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("版の番号は 1 以上です")]
pub struct RevisionNumberError;

impl RevisionNumber {
    pub const FIRST: Self = Self(1);

    pub fn new(value: u32) -> Result<Self, RevisionNumberError> {
        if value == 0 {
            return Err(RevisionNumberError);
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }

    /// 次の版の番号。
    pub fn next(self) -> Self {
        // 1つの資料で 40 億回登録することはないので、上限では止めるだけにする
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for RevisionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// どこから登録したか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RevisionSource {
    Web,
    Mcp,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("登録元「{0}」はありません")]
pub struct RevisionSourceError(String);

impl RevisionSource {
    pub fn parse(value: &str) -> Result<Self, RevisionSourceError> {
        match value {
            "web" => Ok(Self::Web),
            "mcp" => Ok(Self::Mcp),
            _ => Err(RevisionSourceError(value.to_owned())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Mcp => "mcp",
        }
    }
}

/// 新しい版を作るときに外から渡す内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRevision {
    pub blob_hash: BlobHash,
    pub message: RevisionMessage,
    pub author_name: AuthorName,
    pub source: RevisionSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revision {
    pub id: RevisionId,
    pub document_id: DocumentId,
    pub number: RevisionNumber,
    pub blob_hash: BlobHash,
    pub message: RevisionMessage,
    pub author_name: AuthorName,
    pub source: RevisionSource,
    /// 「この版に戻す」で作った版だけが持つ、戻した元の版番号。
    pub restored_from: Option<RevisionNumber>,
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_starts_at_one() {
        assert_eq!(RevisionNumber::new(0), Err(RevisionNumberError));
        assert_eq!(RevisionNumber::new(1), Ok(RevisionNumber::FIRST));
    }

    #[test]
    fn next_number_counts_up() {
        assert_eq!(RevisionNumber::FIRST.next().get(), 2);
        assert_eq!(RevisionNumber(u32::MAX).next().get(), u32::MAX);
    }

    #[test]
    fn source_round_trips() {
        for source in [RevisionSource::Web, RevisionSource::Mcp] {
            assert_eq!(RevisionSource::parse(source.as_str()), Ok(source));
        }
        assert!(RevisionSource::parse("cli").is_err());
    }
}
