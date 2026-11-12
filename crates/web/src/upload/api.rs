//! 登録の API（`assets/upload.js` から呼ぶ）。
//!
//! | ルート | 使う場面 |
//! |--------|----------|
//! | `POST /api/upload/check` | ファイルを選んだとき・貼り付けたとき。HTML を検査し、資料名の初期値（`<title>`）を返す |
//! | `GET /api/upload/targets?project=&file_name=` | 登録先の候補（プロジェクトの資料と、ファイル名が一致した資料） |
//! | `POST /api/upload` | 登録する。成功したら資料ビューアの URL（登録直後の通知付き）を返す |
//!
//! POST はサーバーの CSRF 対策（同じオリジンの JSON の POST だけ通す）を通ってくる。更新者名は
//! Cookie から読む（ダイアログの「更新者」は、送る前に `assets/upload.js` が Cookie に保存する）。
//!
//! 失敗したときは `{"error": {"code": "…", "message": "…"}}` を返す。外部リソースを読み込んでいて
//! 断ったとき（`external_resource`）は、該当箇所と AI への依頼文を `details` に添える。

use std::sync::Arc;

use galley_core::{
    app::{
        document::{CreateDocumentInput, SlugSource},
        error::AppError,
        revision::{RegisteredRevision, RevisionInput},
    },
    domain::{
        document::{DocumentError, DocumentListItem, DocumentQuery},
        project::{Project, ProjectError},
        revision::{AuthorName, RevisionSource},
        shared::ArchiveFilter,
        upload::{ExternalResources, HtmlDocument, HtmlSizeLimit, UploadError},
    },
};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        Body, Next,
        content::Json,
        error::{BadRequestError, ContentTooLargeError},
        layer, query_params, request,
        response::{IntoResponse, Response},
        route,
    },
};

use crate::{WebApp, author, pages::viewer::registered_path};

/// `POST /api/upload/check` の本文。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRequest {
    html: String,
}

/// 検査を通った HTML。
#[derive(Debug, PartialEq, Eq, Serialize)]
struct CheckResponse {
    /// `<title>` の中身。新しい資料の資料名の初期値にする。
    title: Option<String>,
}

/// `GET /api/upload/targets` のクエリ。
#[query_params]
struct TargetsQuery {
    project: Option<String>,
    file_name: Option<String>,
}

/// 登録先の候補。
#[derive(Debug, PartialEq, Eq, Serialize)]
struct TargetsResponse {
    /// プロジェクトの資料（アーカイブしたものを除く、更新が新しい順）。「ほかの資料の新しい版」で選ぶ。
    documents: Vec<TargetDocument>,
    /// ファイル名が一致した資料の slug（「ファイル名が一致」）。
    suggested: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct TargetDocument {
    slug: String,
    title: String,
    /// 現在版の番号。登録すると、この次の版になる。
    revision_number: u32,
}

impl From<&DocumentListItem> for TargetDocument {
    fn from(item: &DocumentListItem) -> Self {
        Self {
            slug: item.slug.to_string(),
            title: item.title.to_string(),
            revision_number: item.revision_number.get(),
        }
    }
}

/// `POST /api/upload` の本文。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterRequest {
    /// プロジェクトの slug。
    project: String,
    target: Target,
    /// ファイルから登録したときのファイル名。新しい資料の slug を作る。貼り付けなら無し。
    #[serde(default)]
    file_name: Option<String>,
    html: String,
    /// 何を変えたか（任意）。
    #[serde(default)]
    message: String,
}

/// 登録先。
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Target {
    /// 新しい資料（第1版）。資料名が空なら HTML の `<title>` を使う。
    NewDocument {
        #[serde(default)]
        title: String,
    },
    /// 既存の資料（slug）の新しい版。
    Revision { document: String },
}

/// 登録した資料と版。名前は MCP の登録の結果（`Registered`）と揃える。
#[derive(Debug, PartialEq, Eq, Serialize)]
struct RegisterResponse {
    /// プロジェクトの slug。
    project: String,
    /// 資料の slug（新しい資料なら、決まった slug）。
    slug: String,
    /// 資料名。
    title: String,
    /// 登録した版の番号。
    revision_number: u32,
    /// 資料ビューアの URL（`/<プロジェクト>/<資料>?registered=<版の番号>`）。開くと登録直後の通知が出る。
    url: String,
}

/// 失敗したときの応答。本文は `{"error": {"code", "message", "details"}}`。
#[derive(Debug, PartialEq, Eq)]
struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ErrorDetail {
    code: ErrorCode,
    /// 画面に出す文言。
    message: String,
    /// 外部リソースを読み込んでいて断ったときの、該当箇所と AI への依頼文。
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ExternalDetails>,
}

/// 失敗の種類。画面はこれで出し分ける（`external_resource` なら登録できない資料のダイアログ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    /// 外部リソースを読み込んでいる。
    ExternalResource,
    /// HTML が大きすぎる（ボディの上限を超えたときも）。
    TooLarge,
    /// HTML が空、または UTF-8 ではない。
    InvalidHtml,
    /// 資料名・変更メモ・更新者名などが正しくない。
    InvalidInput,
    /// 更新者名（Cookie）が無い。
    AuthorRequired,
    /// プロジェクト・資料が見つからない。
    NotFound,
    /// アーカイブしたプロジェクト・資料には登録できない。
    Archived,
    /// リクエストの形が正しくない。
    BadRequest,
    /// サーバーの中の失敗（DB・ストレージ）。
    Internal,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ExternalDetails {
    /// 見つかった件数（`resources` に載せきれなかったものも数える）。
    total: usize,
    /// 該当箇所（行番号の順）。
    resources: Vec<ResourceDetail>,
    /// 資料を作った AI に送る、作り直しの依頼文。
    ai_request: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ResourceDetail {
    line: usize,
    /// 種類（「スクリプト」など、画面に出す言葉）。
    kind: &'static str,
    /// 該当するコード。HTML のソースなので、画面ではテキストとして出す。
    code: String,
}

impl From<&ExternalResources> for ExternalDetails {
    fn from(resources: &ExternalResources) -> Self {
        Self {
            total: resources.total(),
            resources: resources
                .resources()
                .iter()
                .map(|resource| ResourceDetail {
                    line: resource.line,
                    kind: resource.kind.label(),
                    code: resource.code.clone(),
                })
                .collect(),
            ai_request: resources.ai_request(),
        }
    }
}

impl ApiError {
    fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ErrorBody {
                error: ErrorDetail {
                    code,
                    message: message.into(),
                    details: None,
                },
            },
        }
    }
}

impl From<UploadError> for ApiError {
    fn from(error: UploadError) -> Self {
        match error {
            UploadError::Empty => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidHtml,
                "HTML が空です",
            ),
            UploadError::TooLarge { size, max } => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::TooLarge,
                format!(
                    "HTML が大きすぎます（{}。上限は {}）",
                    format_size(size, Rounding::Up),
                    format_size(max, Rounding::Down)
                ),
            ),
            UploadError::NotUtf8 => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidHtml,
                "UTF-8 で保存した HTML を選んでください",
            ),
            UploadError::ExternalResources(resources) => {
                let mut error = Self::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    ErrorCode::ExternalResource,
                    "外部のファイルを読み込んでいるため、登録できません",
                );
                error.body.error.details = Some(ExternalDetails::from(&resources));
                error
            }
        }
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        let message = error.to_string();
        match error {
            AppError::InvalidUpload(error) => error.into(),
            AppError::ProjectNotFound | AppError::DocumentNotFound | AppError::RevisionNotFound => {
                Self::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, message)
            }
            AppError::Project(ProjectError::Archived)
            | AppError::Document(DocumentError::Archived) => {
                Self::new(StatusCode::CONFLICT, ErrorCode::Archived, message)
            }
            AppError::Repository(_) | AppError::Storage(_) => {
                tracing::error!(%error, "資料を登録できませんでした");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorCode::Internal,
                    "登録できませんでした。時間をおいて、もう一度試してください",
                )
            }
            // 資料名・変更メモ・更新者の検査など。core の文言が利用者向けに書いてあるので、そのまま出す
            // （新しいエラーを足したら、利用者に見せてよい文言かを確かめてここに加える）
            AppError::InvalidText(_)
            | AppError::InvalidSlug(_)
            | AppError::InvalidCrestColor(_)
            | AppError::ProjectSlugTaken(_)
            | AppError::Document(
                DocumentError::AlreadyLatest(_) | DocumentError::RevisionMismatch,
            ) => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidInput,
                message,
            ),
        }
    }
}

/// 大きさを小数1桁にするときの丸め方。
///
/// 断った HTML の大きさは切り上げ、上限は切り捨てる。上限をわずかに超えただけでも
/// 「10.1 MB。上限は 10 MB」のように、必ず上限より大きく見せるため（`assets/upload.js` と同じ）。
#[derive(Debug, Clone, Copy)]
enum Rounding {
    Up,
    Down,
}

/// 「512 KB」「10 MB」「12.5 MB」（1MB 未満は KB。1KB = 1024 バイト。`assets/upload.js` の表示と揃える）。
fn format_size(bytes: usize, rounding: Rounding) -> String {
    const KIBIBYTE: usize = 1024;
    const MEBIBYTE: usize = 1024 * 1024;
    let (unit, label) = if bytes < MEBIBYTE {
        (KIBIBYTE, "KB")
    } else {
        (MEBIBYTE, "MB")
    };
    let tenths = match rounding {
        Rounding::Up => (bytes * 10).div_ceil(unit),
        Rounding::Down => bytes * 10 / unit,
    };
    if tenths.is_multiple_of(10) {
        format!("{} {label}", tenths / 10)
    } else {
        format!("{}.{} {label}", tenths / 10, tenths % 10)
    }
}

/// 成功なら `200` と本文、失敗なら [`ApiError`] の状態と本文を JSON で返す。
fn respond<T: Serialize>(cx: &Cx, result: Result<T, ApiError>) -> Result<Response> {
    match result {
        Ok(body) => Json(body).into_response(cx),
        Err(error) => (error.status, Json(error.body)).into_response(cx),
    }
}

/// Topcoat が本文を読む前に返すエラー（ボディの上限を超えた、JSON として読めない）も、
/// ほかのエラーと同じ形の JSON にする。
#[layer("/api/upload")]
pub async fn json_errors(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    match next.run(cx, body).await {
        Err(error) if error.downcast_ref::<ContentTooLargeError>().is_some() => respond::<()>(
            cx,
            Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                ErrorCode::TooLarge,
                format!(
                    "HTML が大きすぎて送れません（上限は {}）",
                    format_size(app_context::<HtmlSizeLimit>(cx).bytes(), Rounding::Down)
                ),
            )),
        ),
        Err(error) if error.downcast_ref::<BadRequestError>().is_some() => respond::<()>(
            cx,
            Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::BadRequest,
                "リクエストの形が正しくありません。画面を読み込み直してください",
            )),
        ),
        result => result,
    }
}

/// HTML を検査する。外部リソースを読み込んでいれば、該当箇所と AI への依頼文を返す。
#[route(POST "/api/upload/check")]
pub async fn check_upload(cx: &Cx, Json(input): Json<CheckRequest>) -> Result<Response> {
    respond(cx, check(input.html, *app_context(cx)))
}

/// 登録先の候補。
#[route(GET "/api/upload/targets")]
pub async fn upload_targets(cx: &Cx) -> Result<Response> {
    let app: &Arc<WebApp> = app_context(cx);
    let result = match query_params::<TargetsQuery>(cx) {
        Ok(TargetsQuery {
            project: Some(project),
            file_name,
        }) if !project.is_empty() => targets(app, project, file_name.as_deref()).await,
        _ => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::BadRequest,
            "プロジェクトを指定してください",
        )),
    };
    respond(cx, result)
}

/// 登録する。
#[route(POST "/api/upload")]
pub async fn register_upload(cx: &Cx, Json(input): Json<RegisterRequest>) -> Result<Response> {
    let app: &Arc<WebApp> = app_context(cx);
    let author = author::from_headers(request::headers(cx));
    respond(cx, register(app, author.as_ref(), input).await)
}

fn check(html: String, limit: HtmlSizeLimit) -> Result<CheckResponse, ApiError> {
    let html = HtmlDocument::parse(html.into_bytes(), limit)?;
    Ok(CheckResponse {
        title: html.title().map(str::to_owned),
    })
}

/// 登録できるプロジェクト（見つからない・アーカイブしたものは断る）。
async fn accepting_project(app: &WebApp, slug: &str) -> Result<Project, ApiError> {
    let project = app.find_project.execute(slug).await?;
    project.ensure_accepts_documents().map_err(AppError::from)?;
    Ok(project)
}

async fn targets(
    app: &WebApp,
    project_slug: &str,
    file_name: Option<&str>,
) -> Result<TargetsResponse, ApiError> {
    let project = accepting_project(app, project_slug).await?;
    let documents = app
        .list_documents
        .execute(&DocumentQuery {
            project_id: Some(project.id),
            archive: ArchiveFilter::Active,
            ..DocumentQuery::default()
        })
        .await?;
    let suggested = match file_name {
        Some(file_name) => app
            .find_document_by_file_name
            .execute(project.id, file_name)
            .await?
            .map(|document| document.slug.to_string()),
        None => None,
    };
    Ok(TargetsResponse {
        documents: documents.iter().map(TargetDocument::from).collect(),
        suggested,
    })
}

async fn register(
    app: &WebApp,
    author: Option<&AuthorName>,
    input: RegisterRequest,
) -> Result<RegisterResponse, ApiError> {
    let author = author.ok_or_else(|| {
        ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::AuthorRequired,
            "登録する前に、あなたの名前を設定してください",
        )
    })?;
    // アーカイブしたプロジェクトには、新しい資料も新しい版も登録しない（一覧に出ないものが更新され続けないように）
    let project = accepting_project(app, &input.project).await?;
    let revision = RevisionInput {
        html: input.html.into_bytes(),
        message: input.message,
        author_name: author.as_str().to_owned(),
        source: RevisionSource::Web,
    };
    let RegisteredRevision { document, revision } = match input.target {
        Target::NewDocument { title } => {
            app.create_document
                .execute(CreateDocumentInput {
                    project_id: project.id,
                    // 空なら HTML の <title> を使う（DocumentTitle::for_new_document）
                    title: Some(title),
                    slug: input
                        .file_name
                        .map_or(SlugSource::None, SlugSource::FileName),
                    revision,
                })
                .await?
        }
        Target::Revision { document } => {
            // アーカイブした資料は AddRevision が断る
            let document = app.find_document.execute(project.id, &document).await?;
            app.add_revision.execute(document.id, revision).await?
        }
    };
    Ok(RegisterResponse {
        project: project.slug.to_string(),
        slug: document.slug.to_string(),
        title: document.title.to_string(),
        revision_number: revision.number.get(),
        url: registered_path(&project.slug, &document.slug, revision.number),
    })
}

#[cfg(test)]
mod tests {
    use galley_core::app::project::ProjectInput;
    use serde_json::{Value, json};

    use super::*;
    use crate::pages::test_app::{self, TestApp};

    type TestResult = std::result::Result<(), String>;

    const HTML: &str = "<!doctype html><title> 事業計画 </title><p>本文</p>";

    async fn project(app: &TestApp, slug: &str) -> Project {
        app.create_project
            .execute(&ProjectInput {
                slug: slug.to_owned(),
                name: "営業企画".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn author() -> AuthorName {
        AuthorName::parse("田中").unwrap_or_else(|e| panic!("{e}"))
    }

    fn request(value: Value) -> RegisterRequest {
        serde_json::from_value(value).unwrap_or_else(|e| panic!("{e}"))
    }

    fn to_json(value: &impl Serialize) -> Value {
        serde_json::to_value(value).unwrap_or_else(|e| panic!("{e}"))
    }

    /// 営業企画（sales）に、第2版まである資料（plan）を作る。
    async fn app_with_document() -> TestApp {
        let app = test_app::new().await;
        let project = project(&app, "sales").await;
        app.document(project.id, "plan", "事業計画", 2).await;
        app
    }

    #[test]
    fn check_returns_title() {
        assert_eq!(
            check(HTML.to_owned(), HtmlSizeLimit::DEFAULT),
            Ok(CheckResponse {
                title: Some("事業計画".to_owned())
            })
        );
        assert_eq!(
            check("<p>題名なし</p>".to_owned(), HtmlSizeLimit::DEFAULT),
            Ok(CheckResponse { title: None })
        );
    }

    #[test]
    fn check_lists_external_resources_with_ai_request() -> TestResult {
        let html = "<!doctype html>\n<title>t</title>\n<script src=\"https://cdn.jsdelivr.net/npm/chart.js@4\"></script>";
        let error = check(html.to_owned(), HtmlSizeLimit::DEFAULT)
            .err()
            .ok_or("断られない")?;
        assert_eq!(error.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            to_json(&error.body),
            json!({ "error": {
                "code": "external_resource",
                "message": "外部のファイルを読み込んでいるため、登録できません",
                "details": {
                    "total": 1,
                    "resources": [{
                        "line": 3,
                        "kind": "スクリプト",
                        "code": "<script src=\"https://cdn.jsdelivr.net/npm/chart.js@4\">",
                    }],
                    "ai_request": "chart.js を外部から読み込まず、すべてを HTML の中に埋め込んだ、1ファイルで完結する HTML に作り直してください。",
                },
            }})
        );
        Ok(())
    }

    #[test]
    fn check_explains_other_problems() -> TestResult {
        let empty = check("  \n".to_owned(), HtmlSizeLimit::DEFAULT)
            .err()
            .ok_or("断られない")?;
        assert_eq!(
            to_json(&empty.body),
            json!({ "error": { "code": "invalid_html", "message": "HTML が空です" } })
        );
        let large = check(
            "a".repeat(HtmlSizeLimit::DEFAULT.bytes() + 1),
            HtmlSizeLimit::DEFAULT,
        )
        .err()
        .ok_or("断られない")?;
        assert_eq!(large.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(large.body.error.code, ErrorCode::TooLarge);
        assert_eq!(
            large.body.error.message,
            "HTML が大きすぎます（10.1 MB。上限は 10 MB）"
        );
        Ok(())
    }

    const KB: usize = 1024;
    const MB: usize = 1024 * 1024;

    #[test]
    fn format_size_rounds_to_one_decimal() {
        for rounding in [Rounding::Up, Rounding::Down] {
            assert_eq!(format_size(10 * MB, rounding), "10 MB");
            assert_eq!(format_size(MB + 512 * KB, rounding), "1.5 MB");
            assert_eq!(format_size(512 * KB, rounding), "512 KB");
            assert_eq!(format_size(1536, rounding), "1.5 KB");
            assert_eq!(format_size(0, rounding), "0 KB");
        }
        // 10.04MB、1025 バイト
        assert_eq!(format_size(10 * MB + 41_943, Rounding::Up), "10.1 MB");
        assert_eq!(format_size(10 * MB + 41_943, Rounding::Down), "10 MB");
        assert_eq!(format_size(KB + 1, Rounding::Up), "1.1 KB");
        assert_eq!(format_size(KB + 1, Rounding::Down), "1 KB");
        // 1MB の手前は KB のまま
        assert_eq!(format_size(MB - 1, Rounding::Up), "1024 KB");
        assert_eq!(format_size(MB - 1, Rounding::Down), "1023.9 KB");
    }

    /// 上限を1バイトでも超えれば、表示も上限より大きくなる（小数1桁で割り切れない上限も含めて）。
    #[test]
    fn size_over_limit_never_looks_equal_to_limit() {
        for limit in [KB, 512 * KB, MB, 1588 * KB, 10 * MB, 100 * MB] {
            let over = format_size(limit + 1, Rounding::Up);
            assert_ne!(over, format_size(limit, Rounding::Down), "{limit}");
        }
        // 1588KB は約 1.551MB。四捨五入では上限も 1.6 MB になり、超えた分と見分けがつかない
        assert_eq!(format_size(1588 * KB, Rounding::Down), "1.5 MB");
        assert_eq!(format_size(1588 * KB + 1, Rounding::Up), "1.6 MB");
    }

    #[tokio::test]
    async fn targets_list_documents_and_file_name_match() -> TestResult {
        let app = app_with_document().await;
        let sales = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        let archived = app.archived_document(sales.id, "古い資料").await;

        let found = targets(&app, "sales", Some("plan.html"))
            .await
            .map_err(|e| e.body.error.message)?;
        assert_eq!(
            found,
            TargetsResponse {
                documents: vec![TargetDocument {
                    slug: "plan".to_owned(),
                    title: "事業計画".to_owned(),
                    revision_number: 2,
                }],
                suggested: Some("plan".to_owned()),
            }
        );
        // ファイル名が一致しない・アーカイブした資料と一致する・貼り付け（ファイル名なし）なら候補なし
        for file_name in [
            Some("other.html"),
            Some(&*format!("{}.html", archived.slug)),
            None,
        ] {
            let found = targets(&app, "sales", file_name)
                .await
                .map_err(|e| e.body.error.message)?;
            assert_eq!(found.suggested, None, "{file_name:?}");
            assert_eq!(found.documents.len(), 1);
        }
        Ok(())
    }

    #[tokio::test]
    async fn targets_refuse_unknown_or_archived_projects() -> TestResult {
        let app = app_with_document().await;
        let unknown = targets(&app, "nothing", None)
            .await
            .err()
            .ok_or("断られない")?;
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);

        let sales = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        app.archive_project
            .execute(sales.id)
            .await
            .map_err(|e| e.to_string())?;
        let archived = targets(&app, "sales", None)
            .await
            .err()
            .ok_or("断られない")?;
        assert_eq!(archived.status, StatusCode::CONFLICT);
        Ok(())
    }

    #[tokio::test]
    async fn registers_new_document_from_file() -> TestResult {
        let app = app_with_document().await;
        let response = register(
            &app,
            Some(&author()),
            request(json!({
                "project": "sales",
                "target": { "kind": "new_document", "title": "" },
                "file_name": "Q3 Report.html",
                "html": HTML,
                "message": "初版",
            })),
        )
        .await
        .map_err(|e| e.body.error.message)?;
        assert_eq!(response.url, "/sales/q3-report?registered=1");

        let sales = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        let document = app
            .find_document
            .execute(sales.id, "q3-report")
            .await
            .map_err(|e| e.to_string())?;
        // 資料名が空なら <title> を使う
        assert_eq!(document.title.as_str(), "事業計画");
        let revisions = app
            .list_revisions
            .execute(&document, None)
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(revisions[0].author_name.as_str(), "田中");
        assert_eq!(revisions[0].message.as_str(), "初版");
        assert_eq!(revisions[0].source, RevisionSource::Web);
        Ok(())
    }

    #[tokio::test]
    async fn registers_pasted_document_with_title_and_generated_slug() -> TestResult {
        let app = app_with_document().await;
        let response = register(
            &app,
            Some(&author()),
            request(json!({
                "project": "sales",
                "target": { "kind": "new_document", "title": "貼り付けた資料" },
                "html": HTML,
            })),
        )
        .await
        .map_err(|e| e.body.error.message)?;
        assert!(
            response.url.starts_with("/sales/doc-") && response.url.ends_with("?registered=1"),
            "{}",
            response.url
        );
        // 同じファイル名の資料があれば、新しい資料は別の slug にする
        let response = register(
            &app,
            Some(&author()),
            request(json!({
                "project": "sales",
                "target": { "kind": "new_document", "title": "別の計画" },
                "file_name": "plan.html",
                "html": HTML,
            })),
        )
        .await
        .map_err(|e| e.body.error.message)?;
        assert_eq!(response.url, "/sales/plan-2?registered=1");
        Ok(())
    }

    #[tokio::test]
    async fn registers_new_revision() -> TestResult {
        let app = app_with_document().await;
        let response = register(
            &app,
            Some(&author()),
            request(json!({
                "project": "sales",
                "target": { "kind": "revision", "document": "plan" },
                "file_name": "plan.html",
                "html": "<!doctype html><title>t</title><p>第3版</p>",
                "message": "数字を更新",
            })),
        )
        .await
        .map_err(|e| e.body.error.message)?;
        assert_eq!(
            to_json(&response),
            json!({
                "project": "sales",
                "slug": "plan",
                "title": "事業計画",
                "revision_number": 3,
                "url": "/sales/plan?registered=3",
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn register_refuses_without_author_or_with_external_resources() -> TestResult {
        let app = app_with_document().await;
        let body = json!({
            "project": "sales",
            "target": { "kind": "revision", "document": "plan" },
            "html": HTML,
        });
        let no_author = register(&app, None, request(body))
            .await
            .err()
            .ok_or("断られない")?;
        assert_eq!(no_author.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            no_author.body.error.message,
            "登録する前に、あなたの名前を設定してください"
        );

        let external = register(
            &app,
            Some(&author()),
            request(json!({
                "project": "sales",
                "target": { "kind": "revision", "document": "plan" },
                "html": "<link rel=\"stylesheet\" href=\"https://example.com/a.css\">",
            })),
        )
        .await
        .err()
        .ok_or("断られない")?;
        assert_eq!(external.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            external.body.error.details.as_ref().map(|d| d.total),
            Some(1)
        );
        Ok(())
    }

    #[tokio::test]
    async fn register_refuses_archived_or_unknown_targets() -> TestResult {
        let app = app_with_document().await;
        let revision_of = |project: &str, document: &str| {
            request(json!({
                "project": project,
                "target": { "kind": "revision", "document": document },
                "html": HTML,
            }))
        };
        let unknown = register(&app, Some(&author()), revision_of("sales", "nothing"))
            .await
            .err()
            .ok_or("断られない")?;
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);
        assert_eq!(unknown.body.error.message, "資料が見つかりません");

        // アーカイブした資料には版を追加しない
        let sales = app
            .find_project
            .execute("sales")
            .await
            .map_err(|e| e.to_string())?;
        let archived = app.archived_document(sales.id, "古い資料").await;
        let refused = register(
            &app,
            Some(&author()),
            revision_of("sales", archived.slug.as_str()),
        )
        .await
        .err()
        .ok_or("断られない")?;
        assert_eq!(refused.status, StatusCode::CONFLICT);

        // アーカイブしたプロジェクトには、新しい資料も新しい版も登録しない
        app.archive_project
            .execute(sales.id)
            .await
            .map_err(|e| e.to_string())?;
        for body in [
            revision_of("sales", "plan"),
            request(json!({
                "project": "sales",
                "target": { "kind": "new_document" },
                "html": HTML,
            })),
        ] {
            let refused = register(&app, Some(&author()), body)
                .await
                .err()
                .ok_or("断られない")?;
            assert_eq!(refused.status, StatusCode::CONFLICT);
            assert!(
                refused.body.error.message.contains("アーカイブ"),
                "{}",
                refused.body.error.message
            );
        }
        Ok(())
    }

    #[test]
    fn register_request_rejects_unknown_shapes() {
        for value in [
            json!({ "project": "sales", "target": { "kind": "delete" }, "html": HTML }),
            json!({ "project": "sales", "target": { "kind": "revision" }, "html": HTML }),
            json!({ "project": "sales", "target": { "kind": "new_document" }, "html": HTML, "author": "x" }),
            json!({ "project": "sales", "html": HTML }),
        ] {
            assert!(
                serde_json::from_value::<RegisterRequest>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}
