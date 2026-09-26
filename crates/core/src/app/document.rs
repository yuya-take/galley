//! 資料のユースケース（版の登録と「この版に戻す」は #9）。

use std::sync::Arc;

use jiff::Timestamp;
use uuid::Uuid;

use crate::domain::{
    document::{Document, DocumentListItem},
    id::{DocumentId, ProjectId},
    repository::{DocumentQuery, DocumentRepository},
    slug::DocumentSlug,
    text::DocumentTitle,
};

use super::error::AppError;

async fn load(documents: &dyn DocumentRepository, id: DocumentId) -> Result<Document, AppError> {
    documents
        .find_by_id(id)
        .await?
        .ok_or(AppError::DocumentNotFound)
}

/// 資料の一覧（すべての資料・資料一覧・⌘K の検索）。
pub struct ListDocuments {
    documents: Arc<dyn DocumentRepository>,
}

impl ListDocuments {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, query: &DocumentQuery) -> Result<Vec<DocumentListItem>, AppError> {
        Ok(self.documents.list(query).await?)
    }
}

/// URL の slug から資料を探す（アーカイブしたものも返す）。
pub struct FindDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl FindDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, project_id: ProjectId, slug: &str) -> Result<Document, AppError> {
        let slug = DocumentSlug::parse(slug).map_err(|_| AppError::DocumentNotFound)?;
        self.documents
            .find_by_slug(project_id, &slug)
            .await?
            .ok_or(AppError::DocumentNotFound)
    }
}

/// ファイル名が一致する資料を探す（登録の確認ダイアログの「ファイル名が一致」）。
/// アーカイブした資料は候補にしない。
pub struct FindDocumentByFileName {
    documents: Arc<dyn DocumentRepository>,
}

impl FindDocumentByFileName {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(
        &self,
        project_id: ProjectId,
        file_name: &str,
    ) -> Result<Option<Document>, AppError> {
        let Some(slug) = DocumentSlug::from_file_name(file_name) else {
            return Ok(None);
        };
        let document = self.documents.find_by_slug(project_id, &slug).await?;
        Ok(document.filter(|document| !document.is_archived()))
    }
}

/// 新しい資料の slug をどこから作るか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlugSource {
    /// ファイルから登録した（ファイル名から作る）。
    FileName(String),
    /// MCP などで slug を指定した。
    Requested(String),
    /// 貼り付けなど、手がかりがない。
    None,
}

/// 新しい資料の slug を決める。プロジェクト内で使われていれば `-2` などを付ける。
pub struct ChooseDocumentSlug {
    documents: Arc<dyn DocumentRepository>,
}

impl ChooseDocumentSlug {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(
        &self,
        project_id: ProjectId,
        source: &SlugSource,
    ) -> Result<DocumentSlug, AppError> {
        let base = match source {
            SlugSource::FileName(name) => DocumentSlug::from_file_name(name),
            SlugSource::Requested(slug) => Some(DocumentSlug::parse(slug)?),
            SlugSource::None => None,
        }
        .unwrap_or_else(|| DocumentSlug::generated(random_u32()));

        let taken = self
            .documents
            .slugs_with_prefix(project_id, base.as_str())
            .await?;
        Ok(base.first_available(|slug| taken.contains(slug)))
    }
}

/// slug 用の短い乱数。乱数のためだけに依存を増やさないよう、UUID v4 の乱数部分を使う。
fn random_u32() -> u32 {
    let [a, b, c, d, ..] = Uuid::new_v4().into_bytes();
    u32::from_le_bytes([a, b, c, d])
}

pub struct RenameDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl RenameDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId, title: &str) -> Result<Document, AppError> {
        let title = DocumentTitle::parse(title)?;
        let mut document = load(self.documents.as_ref(), id).await?;
        document.rename(title);
        self.documents.update(&document).await?;
        Ok(document)
    }
}

pub struct ArchiveDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl ArchiveDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId) -> Result<Document, AppError> {
        let mut document = load(self.documents.as_ref(), id).await?;
        document.archive(Timestamp::now());
        self.documents.update(&document).await?;
        Ok(document)
    }
}

pub struct RestoreDocument {
    documents: Arc<dyn DocumentRepository>,
}

impl RestoreDocument {
    pub fn new(documents: Arc<dyn DocumentRepository>) -> Self {
        Self { documents }
    }

    pub async fn execute(&self, id: DocumentId) -> Result<Document, AppError> {
        let mut document = load(self.documents.as_ref(), id).await?;
        document.restore();
        self.documents.update(&document).await?;
        Ok(document)
    }
}
