//! MCP サーバー（`/mcp`、Streamable HTTP）。
//!
//! AI エージェントが作った資料を、そのままプロジェクトに保存・更新できるようにする。
//! 資料は「プロジェクトの slug」と「資料の slug」で指定する。登録・更新は画面と同じ検査
//! （外部リソースの検出など）を通し、更新者名は接続設定の `X-Author-Name` ヘッダーで受け取る。

mod output;
mod params;

use std::sync::Arc;

use galley_core::{
    app::{
        document::{CreateDocument, CreateDocumentInput, FindDocument, ListDocuments, SlugSource},
        error::AppError,
        project::{FindProject, ListProjects},
        revision::{AddRevision, FindRevision, ReadRevisionContent, RevisionInput},
    },
    domain::{
        document::DocumentQuery, revision::RevisionSource, shared::ArchiveFilter,
        upload::REJECTION_REASON,
    },
};
use galley_web::AppUrl;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    service::RequestContext,
    tool, tool_handler, tool_router,
};

use output::{DocumentItem, DocumentWithHtml, ProjectItem, Registered, describe_error};
use params::{
    CreateDocumentParams, DocumentParams, GetDocumentParams, ProjectParams, UpdateDocumentParams,
};

/// MCP から使うユースケース。起動時に1つ作り、接続ごとの [`GalleyMcp`] で共有する。
pub struct McpApp {
    pub list_projects: ListProjects,
    pub find_project: FindProject,
    pub list_documents: ListDocuments,
    pub find_document: FindDocument,
    pub find_revision: FindRevision,
    pub read_revision: ReadRevisionContent,
    pub create_document: CreateDocument,
    pub add_revision: AddRevision,
    pub app_url: AppUrl,
}

#[derive(Clone)]
pub struct GalleyMcp {
    app: Arc<McpApp>,
    tool_router: ToolRouter<Self>,
}

impl GalleyMcp {
    pub fn new(app: Arc<McpApp>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }
}

/// ツールの結果。ユースケースのエラーはツールのエラー（AI が読める文言）にする。
type ToolResult = Result<CallToolResult, ErrorData>;

fn json(value: &impl serde::Serialize) -> ToolResult {
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

fn tool_error(err: &AppError) -> ToolResult {
    // 利用者向けの文言だと分かっているものだけ返す。DB やストレージのエラーは内部の情報を含みうるので隠す
    let is_for_user = match err {
        AppError::InvalidSlug(_)
        | AppError::InvalidText(_)
        | AppError::InvalidCrestColor(_)
        | AppError::ProjectNotFound
        | AppError::DocumentNotFound
        | AppError::RevisionNotFound
        | AppError::Project(_)
        | AppError::InvalidUpload(_)
        | AppError::Document(_)
        | AppError::ProjectSlugTaken(_) => true,
        AppError::Repository(_) | AppError::Storage(_) => false,
    };
    if is_for_user {
        return Ok(CallToolResult::error(vec![ContentBlock::text(
            describe_error(err),
        )]));
    }
    tracing::error!("MCP のツールでエラー: {err}");
    Ok(CallToolResult::error(vec![ContentBlock::text(
        "Galley の内部でエラーが起きました。時間をおいてやり直してください",
    )]))
}

/// 接続設定の `X-Author-Name`。ASCII 以外（日本語の名前）も読めるよう UTF-8 として読む。
fn author_name(ctx: &RequestContext<RoleServer>) -> Option<String> {
    ctx.extensions
        .get::<http::request::Parts>()
        .and_then(|parts| parts.headers.get("x-author-name"))
        .and_then(|value| std::str::from_utf8(value.as_bytes()).ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
}

fn request_host(ctx: &RequestContext<RoleServer>) -> Option<String> {
    ctx.extensions
        .get::<http::request::Parts>()
        .and_then(|parts| parts.headers.get(http::header::HOST))
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

const MISSING_AUTHOR: &str = "更新者名がありません。MCP の接続設定に X-Author-Name ヘッダー（例：X-Author-Name: 佐藤）を入れてください";

#[tool_router]
impl GalleyMcp {
    #[tool(
        description = "Galley のプロジェクトの一覧を返す（アーカイブしたものは除く）。資料を登録するプロジェクトの slug を調べるのに使う"
    )]
    async fn list_projects(&self) -> ToolResult {
        match self.app.list_projects.execute(ArchiveFilter::Active).await {
            Ok(projects) => json(&projects.iter().map(ProjectItem::from).collect::<Vec<_>>()),
            Err(err) => tool_error(&err),
        }
    }

    #[tool(
        description = "プロジェクト内の資料の一覧を返す（資料名、slug、最新の版番号、資料ビューアの URL）。更新が新しい順"
    )]
    async fn list_documents(
        &self,
        Parameters(params): Parameters<ProjectParams>,
        ctx: RequestContext<RoleServer>,
    ) -> ToolResult {
        let result = async {
            let project = self.app.find_project.execute(&params.project).await?;
            let query = DocumentQuery {
                project_id: Some(project.id),
                ..DocumentQuery::default()
            };
            self.app.list_documents.execute(&query).await
        }
        .await;
        let host = request_host(&ctx);
        match result {
            Ok(items) => json(
                &items
                    .iter()
                    .map(|item| DocumentItem::new(item, &self.app.app_url, host.as_deref()))
                    .collect::<Vec<_>>(),
            ),
            Err(err) => tool_error(&err),
        }
    }

    #[tool(
        description = "資料の情報と、指定した版の HTML 本文を返す。revision を省略すると最新の版"
    )]
    async fn get_document(
        &self,
        Parameters(params): Parameters<GetDocumentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> ToolResult {
        let result = async {
            let project = self.app.find_project.execute(&params.project).await?;
            let document = self
                .app
                .find_document
                .execute(project.id, &params.document)
                .await?;
            let revision = self
                .app
                .find_revision
                .execute(&document, params.revision)
                .await?;
            let latest = self.app.find_revision.execute(&document, None).await?;
            let content = self.app.read_revision.execute(revision.id).await?;
            Ok::<_, AppError>((project, document, latest, content))
        }
        .await;
        match result {
            Ok((project, document, latest, content)) => json(&DocumentWithHtml::new(
                &project,
                &document,
                &latest,
                content,
                &self.app.app_url,
                request_host(&ctx).as_deref(),
            )),
            Err(err) => tool_error(&err),
        }
    }

    #[tool(
        description = "新しい資料を第1版として登録する。html は外部リソース（CDN のスクリプト、Web フォント、外部の画像など）を一切読み込まない、1ファイルで完結した HTML にすること（CSS と JavaScript は HTML 内に直接書き、画像は data URI で埋め込む）。外部リソースがあると登録できない。slug を省略すると自動で決める"
    )]
    async fn create_document(
        &self,
        Parameters(params): Parameters<CreateDocumentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> ToolResult {
        let Some(author_name) = author_name(&ctx) else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                MISSING_AUTHOR,
            )]));
        };
        let result = async {
            let project = self.app.find_project.execute(&params.project).await?;
            let registered = self
                .app
                .create_document
                .execute(CreateDocumentInput {
                    project_id: project.id,
                    title: Some(params.title),
                    slug: params.slug.map_or(SlugSource::None, SlugSource::Requested),
                    revision: RevisionInput {
                        html: params.html.into_bytes(),
                        message: params.message.unwrap_or_default(),
                        author_name,
                        source: RevisionSource::Mcp,
                    },
                })
                .await?;
            Ok::<_, AppError>((project, registered))
        }
        .await;
        match result {
            Ok((project, registered)) => json(&Registered::new(
                &project,
                &registered,
                &self.app.app_url,
                request_host(&ctx).as_deref(),
            )),
            Err(err) => tool_error(&err),
        }
    }

    #[tool(
        description = "既存の資料に、html を新しい版として追加する。html は外部リソース（CDN のスクリプト、Web フォント、外部の画像など）を一切読み込まない、1ファイルで完結した HTML にすること。外部リソースがあると登録できない。message には何を変えたかを短く書く"
    )]
    async fn update_document(
        &self,
        Parameters(params): Parameters<UpdateDocumentParams>,
        ctx: RequestContext<RoleServer>,
    ) -> ToolResult {
        let Some(author_name) = author_name(&ctx) else {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                MISSING_AUTHOR,
            )]));
        };
        let DocumentParams { project, document } = params.target;
        let result = async {
            let project = self.app.find_project.execute(&project).await?;
            let document = self
                .app
                .find_document
                .execute(project.id, &document)
                .await?;
            let registered = self
                .app
                .add_revision
                .execute(
                    document.id,
                    RevisionInput {
                        html: params.html.into_bytes(),
                        message: params.message.unwrap_or_default(),
                        author_name,
                        source: RevisionSource::Mcp,
                    },
                )
                .await?;
            Ok::<_, AppError>((project, registered))
        }
        .await;
        match result {
            Ok((project, registered)) => json(&Registered::new(
                &project,
                &registered,
                &self.app.app_url,
                request_host(&ctx).as_deref(),
            )),
            Err(err) => tool_error(&err),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GalleyMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("galley", env!("CARGO_PKG_VERSION")))
            .with_instructions(
            format!(
                "Galley は社内で HTML の資料を版ごとに保存・共有するサービスです。\
                 資料は外部リソースを読み込まない1ファイルの HTML だけ登録できます（{REJECTION_REASON}）。\
                 登録・更新の結果に含まれる url を利用者に伝えてください。"
            ),
        )
    }
}
