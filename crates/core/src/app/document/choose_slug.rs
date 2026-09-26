use std::sync::Arc;

use uuid::Uuid;

use crate::{
    app::error::AppError,
    domain::{
        document::{DocumentRepository, DocumentSlug},
        shared::ProjectId,
    },
};

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
