//! ツールの結果（JSON）と、エラーの文言。

use galley_core::{
    app::{
        error::AppError,
        revision::{RegisteredRevision, RevisionContent},
    },
    domain::{
        document::{Document, DocumentListItem},
        project::{Project, ProjectSummary},
        revision::Revision,
        upload::{REJECTION_REASON, UploadError},
    },
};
use galley_web::AppUrl;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ProjectItem {
    slug: String,
    name: String,
    description: String,
    document_count: u64,
}

impl From<&ProjectSummary> for ProjectItem {
    fn from(summary: &ProjectSummary) -> Self {
        Self {
            slug: summary.project.slug.to_string(),
            name: summary.project.name.to_string(),
            description: summary.project.description.to_string(),
            document_count: summary.document_count,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DocumentItem {
    slug: String,
    title: String,
    /// 最新の版の番号。
    revision_number: u32,
    author_name: String,
    updated_at: String,
    url: Option<String>,
}

impl DocumentItem {
    pub fn new(item: &DocumentListItem, app_url: &AppUrl, host: Option<&str>) -> Self {
        Self {
            slug: item.slug.to_string(),
            title: item.title.to_string(),
            revision_number: item.revision_number.get(),
            author_name: item.author_name.to_string(),
            updated_at: item.updated_at.to_string(),
            url: app_url.document_url(host, item.project_slug.as_str(), item.slug.as_str()),
        }
    }
}

/// 資料と、指定した版の HTML。
#[derive(Debug, Serialize)]
pub struct DocumentWithHtml {
    project: String,
    slug: String,
    title: String,
    /// 取り出した版の番号。
    revision_number: u32,
    /// 最新の版の番号。
    latest_revision_number: u32,
    message: String,
    author_name: String,
    created_at: String,
    /// 「この版に戻す」で作った版なら、戻した元の版番号。
    restored_from: Option<u32>,
    archived: bool,
    /// この版を開く URL（最新の版なら版番号の無い URL）。
    url: Option<String>,
    html: String,
}

impl DocumentWithHtml {
    pub fn new(
        project: &Project,
        document: &Document,
        latest: &Revision,
        content: RevisionContent,
        app_url: &AppUrl,
        host: Option<&str>,
    ) -> Self {
        let revision = &content.revision;
        let (project_slug, document_slug) = (project.slug.as_str(), document.slug.as_str());
        let url = if revision.id == latest.id {
            app_url.document_url(host, project_slug, document_slug)
        } else {
            app_url.revision_url(host, project_slug, document_slug, revision.number.get())
        };
        Self {
            project: project.slug.to_string(),
            slug: document.slug.to_string(),
            title: document.title.to_string(),
            revision_number: revision.number.get(),
            latest_revision_number: latest.number.get(),
            message: revision.message.to_string(),
            author_name: revision.author_name.to_string(),
            created_at: revision.created_at.to_string(),
            restored_from: revision.restored_from.map(|n| n.get()),
            archived: document.is_archived(),
            url,
            html: String::from_utf8_lossy(&content.html).into_owned(),
        }
    }
}

/// 登録・更新の結果。
#[derive(Debug, Serialize)]
pub struct Registered {
    /// 「第2版として登録しました」。
    message: String,
    project: String,
    slug: String,
    title: String,
    revision_number: u32,
    /// 資料ビューアの URL。利用者にそのまま伝える。
    url: Option<String>,
}

impl Registered {
    pub fn new(
        project: &Project,
        registered: &RegisteredRevision,
        app_url: &AppUrl,
        host: Option<&str>,
    ) -> Self {
        let document = &registered.document;
        let number = registered.revision.number;
        Self {
            message: format!("「{}」を第{number}版として登録しました", document.title),
            project: project.slug.to_string(),
            slug: document.slug.to_string(),
            title: document.title.to_string(),
            revision_number: number.get(),
            url: app_url.document_url(host, project.slug.as_str(), document.slug.as_str()),
        }
    }
}

/// AI がそのまま直せるエラーの文言。外部リソースで断ったときは、該当箇所と作り直しの依頼文を付ける。
pub fn describe_error(err: &AppError) -> String {
    let AppError::InvalidUpload(UploadError::ExternalResources(resources)) = err else {
        return err.to_string();
    };
    let mut text = format!(
        "{REJECTION_REASON}。外部リソースを {} か所で読み込んでいます。\n",
        resources.total()
    );
    for resource in resources.resources() {
        text.push_str(&format!(
            "- {} 行目（{}）: {}\n",
            resource.line, resource.kind, resource.code
        ));
    }
    if resources.total() > resources.resources().len() {
        text.push_str(&format!(
            "- ほか {} か所\n",
            resources.total() - resources.resources().len()
        ));
    }
    text.push('\n');
    text.push_str(&resources.ai_request());
    text
}
