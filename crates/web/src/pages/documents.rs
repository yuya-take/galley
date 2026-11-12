//! 資料の一覧（すべての資料 `/`、自分が更新した資料 `/?author=me`、プロジェクトの資料一覧 `/{project}`）。
//!
//! 絞り込みと並び順は URL のクエリで持ち、サーバーで絞る（[`params`]）。行を選ぶと、右のプレビュー欄だけを
//! サーバーで描き直す（[`preview`] の shard）。「この中を絞り込む」は表示中の行をブラウザで絞る（`assets/app.js`）。

mod params;
mod preview;

pub use preview::document_preview;

use std::sync::Arc;

use galley_core::{
    app::error::AppError,
    domain::{
        document::{DocumentListItem, DocumentQuery},
        project::ProjectSummary,
        revision::AuthorName,
        shared::{ArchiveFilter, ProjectId},
    },
};
use jiff::Zoned;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, page, path_param, query_params, request},
    runtime::{Signal, signal},
    view::{Unescaped, View, component, view},
};

use self::params::ListParams;
use crate::{WebApp, author, date, icon, mcp_command, search};

mod param {
    topcoat::router::path_param!(pub project);
}

/// 一覧のクエリ（[`params`] のモジュールの説明）。読めなければ既定の一覧にする。
#[query_params]
struct ListQuery {
    author: Option<String>,
    week: Option<String>,
    project: Option<String>,
    sort: Option<String>,
}

fn list_params(cx: &Cx) -> ListParams {
    match query_params::<ListQuery>(cx) {
        Ok(query) => ListParams::parse(
            query.author.as_deref(),
            query.week.as_deref(),
            query.project.as_deref(),
            query.sort.as_deref(),
        ),
        Err(_) => ListParams::default(),
    }
}

/// 一覧に出す行。「自分が更新」で名前が未設定なら、一覧の代わりに名前の設定を促す。
enum Rows {
    NeedsAuthor,
    Items(Vec<DocumentListItem>),
}

impl Rows {
    async fn load(
        app: &WebApp,
        params: &ListParams,
        project_id: Option<ProjectId>,
        author: Option<&AuthorName>,
        now: &Zoned,
    ) -> Result<Self, AppError> {
        let author_name = match (params.is_mine, author) {
            (false, _) => None,
            (true, Some(author)) => Some(author.clone()),
            (true, None) => return Ok(Self::NeedsAuthor),
        };
        let query = DocumentQuery {
            project_id,
            archive: ArchiveFilter::Active,
            title_contains: None,
            updated_since: params.is_this_week.then(|| date::start_of_week(now)),
            author_name,
            order: params.sort.order(),
            limit: None,
        };
        Ok(Self::Items(app.list_documents.execute(&query).await?))
    }

    fn count(&self) -> Option<usize> {
        match self {
            Self::NeedsAuthor => None,
            Self::Items(items) => Some(items.len()),
        }
    }
}

/// すべての資料・自分が更新した資料。
#[page("/")]
pub async fn all_documents(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let author = author::from_headers(request::headers(cx));
    let projects = app.sidebar_projects().await?;
    let mut params = list_params(cx);
    // 選べるのはアーカイブしていないプロジェクトだけ。一覧に無いものは選んでいないことにする
    let selected_project = params.project.as_ref().and_then(|slug| {
        projects
            .iter()
            .find(|summary| &summary.project.slug == slug)
    });
    let project_id = selected_project.map(|summary| summary.project.id);
    if selected_project.is_none() {
        params.project = None;
    }
    let now = date::now();
    let rows = Rows::load(app, &params, project_id, author.as_ref(), &now).await?;

    let (title, description) = if params.is_mine {
        (
            "自分が更新した資料",
            match &author {
                Some(name) => format!("{name}さんが版を登録した資料を、新しい順に並べています"),
                None => "名前を設定すると、自分が版を登録した資料を絞り込めます".to_owned(),
            },
        )
    } else {
        (
            "すべての資料",
            "すべてのプロジェクトの資料を、更新が新しい順に並べています".to_owned(),
        )
    };
    let count = rows.count();
    let empty = if projects.is_empty() {
        Empty::NoProjects
    } else {
        Empty::NoDocuments
    };

    Ok(view! {
        <header class="page-header">
            <div class="page-title">
                <div class="page-title-line">
                    <h1>(title)</h1>
                    page_count(count: count)
                </div>
                <p class="page-description">(description)</p>
            </div>
            register_button()
        </header>
        document_list(
            path: "/",
            params: &params,
            rows: &rows,
            projects: Some(projects.as_slice()),
            empty: empty,
            now: &now,
        )
    })
}

/// プロジェクトの資料一覧。アーカイブしたプロジェクトは見つからないことにする。
#[page("/{project}")]
pub async fn project_documents(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let slug: &str = path_param::<param::Project>(cx);
    let project = found(app.find_project.execute(slug).await)?
        .filter(|project| !project.is_archived())
        .ok_or_not_found()?;
    let author = author::from_headers(request::headers(cx));
    let params = ListParams {
        project: None,
        ..list_params(cx)
    };
    let now = date::now();
    let rows = Rows::load(app, &params, Some(project.id), author.as_ref(), &now).await?;
    let path = format!("/{}", project.slug);
    let mcp_url = mcp_command::url(&app.app_url, host(cx));
    let command = mcp_command::claude_code(&mcp_url, author.as_ref().map(AuthorName::as_str));
    let empty = Empty::Project {
        name: project.name.to_string(),
        command,
        has_author: author.is_some(),
    };

    Ok(view! {
        <header class="page-header">
            <div class="page-title">
                <div class="page-title-line">
                    <span class=(format!("page-crest crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                    <h1>(project.name.as_str())</h1>
                    page_count(count: rows.count())
                </div>
                if !project.description.as_str().is_empty() {
                    <p class="page-description">(project.description.as_str())</p>
                }
            </div>
            <a class="button button--outline" href=(format!("{path}/settings"))>
                (Unescaped::new_unchecked(SETTINGS_SVG))
                "設定"
            </a>
            register_button()
        </header>
        document_list(
            path: &path,
            params: &params,
            rows: &rows,
            projects: None,
            empty: empty,
            now: &now,
        )
    })
}

/// 見つからないエラーを `None` にする（ほかのエラーはそのまま返す）。
pub(super) fn found<T>(result: Result<T, AppError>) -> Result<Option<T>, AppError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(AppError::ProjectNotFound | AppError::DocumentNotFound) => Ok(None),
        Err(err) => Err(err),
    }
}

/// リクエストのホスト（Host ヘッダー。HTTP/2 なら `:authority`）。
pub(super) fn host(cx: &Cx) -> Option<&str> {
    request::headers(cx)
        .get(http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            request::uri(cx)
                .authority()
                .map(|authority| authority.as_str())
        })
}

/// 一覧が空のときの表示。
enum Empty {
    /// すべての資料で、プロジェクトがまだ無い。
    NoProjects,
    /// すべての資料で、資料がまだ無い。
    NoDocuments,
    /// プロジェクトに資料がまだ無い（最初の登録方法を案内する）。
    Project {
        name: String,
        command: String,
        has_author: bool,
    },
}

#[component]
async fn page_count(count: Option<usize>) -> Result<impl View> {
    Ok(view! {
        if let Some(count) = count {
            <span class="page-count" data-row-count="">(format!("{count}件"))</span>
        }
    })
}

/// 「HTMLを登録」。登録の流れは #17 で作る。
#[component]
async fn register_button() -> Result<impl View> {
    Ok(view! {
        <button type="button" class="button button--primary" aria-disabled="true" title="登録は準備中です">"＋ HTMLを登録"</button>
    })
}

/// 絞り込み、表、プレビュー欄。
#[component]
async fn document_list<'a>(
    cx: &'a Cx,
    path: &'a str,
    params: &'a ListParams,
    rows: &'a Rows,
    projects: Option<&'a [ProjectSummary]>,
    empty: Empty,
    now: &'a Zoned,
) -> Result<impl View> {
    let items = match rows {
        Rows::NeedsAuthor => None,
        Rows::Items(items) => Some(items.as_slice()),
    };
    let is_empty = items.is_some_and(|items| items.is_empty() && !params.has_filters());
    let first = items
        .and_then(|items| items.first())
        .map(row_key)
        .unwrap_or_default();
    let selected = signal(cx, move || first);

    Ok(view! {
        match items {
            None => {
                filters(path: path, params: params, projects: projects)
                author_notice()
            },
            Some(_) if is_empty => empty_state(empty: empty),
            Some(items) => <div class="documents">
            <section class="documents-main" aria-label="資料の一覧">
                filters(path: path, params: params, projects: projects)
                if items.is_empty() {
                    <div class="panel notice">
                        <p>"条件に合う資料はありません。"</p>
                        <p><a href=(params.without_filters().href(path))>"絞り込みを外す"</a></p>
                    </div>
                } else {
                    document_table(path: path, params: params, items: items, selected: &selected, now: now)
                }
                <p class="drop-hint">
                    (Unescaped::new_unchecked(UPLOAD_SVG))
                    "HTMLファイルを画面のどこにでもドロップ、または " <kbd>"⌘V"</kbd> " で貼り付けて登録"
                </p>
            </section>
            if !items.is_empty() {
                <aside class="preview" aria-label="プレビュー">
                    document_preview(key: $(selected.get()))
                </aside>
            }
        </div>,
        }
    })
}

/// 「自分が更新」で名前が未設定のとき。
#[component]
async fn author_notice() -> Result<impl View> {
    Ok(view! {
        <section class="panel notice">
            <h2>"名前が未設定です"</h2>
            <p>"「自分が更新」は、このブラウザに保存した名前で版を登録した資料を絞り込みます。"</p>
            <div>
                <button type="button" class="button button--primary" data-action="change-author">"名前を設定する"</button>
            </div>
        </section>
    })
}

/// 資料の表。行を押すと `selected` が変わり、プレビュー欄が描き直される。
#[component]
async fn document_table<'a>(
    path: &'a str,
    params: &'a ListParams,
    items: &'a [DocumentListItem],
    selected: &'a Signal<String>,
    now: &'a Zoned,
) -> Result<impl View> {
    let time_zone = now.time_zone().clone();
    Ok(view! {
        <div class="table-wrap">
            <table class="documents-table">
                <thead>
                    <tr>
                        <th scope="col" aria-sort=(params.sort.title_aria())>
                            <a class="sort-link" href=(params.with_sort(params.sort.by_title()).href(path))>"資料名" (sort_mark(params.sort.title_aria()))</a>
                        </th>
                        <th scope="col" class="col-version">"版"</th>
                        <th scope="col" class="col-author">"更新者"</th>
                        <th scope="col" class="col-updated" aria-sort=(params.sort.updated_aria())>
                            <a class="sort-link" href=(params.with_sort(params.sort.by_updated()).href(path))>"更新日時" (sort_mark(params.sort.updated_aria()))</a>
                        </th>
                    </tr>
                </thead>
                <tbody>
                    #[key(item.id.to_string())]
                    for item in items {
                        let key = row_key(item);
                        <tr
                            class="document-row"
                            data-search-text=(search::search_text(item))
                            @click=$(|_e| selected.set(key.to_owned()))
                        >
                            <td class="col-title">
                                <button
                                    type="button"
                                    class="document-select"
                                    data-href=(format!("/{key}"))
                                    :aria-current=$(if selected.get() == key { "true" } else { "false" })
                                >(item.title.as_str())</button>
                                <div class="document-meta">
                                    <span class="document-project">
                                        <span class=(format!("crest--{}", item.project_color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                                        (item.project_name.as_str())
                                    </span>
                                    if !item.revision_message.as_str().is_empty() {
                                        <span class="document-message">(item.revision_message.as_str())</span>
                                    }
                                </div>
                            </td>
                            <td class="col-version"><span class="version">(format!("第{}版", item.revision_number))</span></td>
                            <td class="col-author">
                                <span class="author">
                                    <span class="avatar" aria-hidden="true">(author::initial(&item.author_name))</span>
                                    (item.author_name.as_str())
                                </span>
                            </td>
                            <td class="col-updated">
                                <time datetime=(date::machine(item.updated_at)) title=(date::long(item.updated_at, &time_zone))>(date::short(item.updated_at, now))</time>
                            </td>
                        </tr>
                    }
                </tbody>
            </table>
            <p class="table-empty" data-filter-empty="" hidden="">"「この中を絞り込む」に合う資料はありません。"</p>
        </div>
    })
}

/// 行を選んだときにプレビュー欄へ渡す値（`<プロジェクト>/<資料>`。資料ビューアの URL のパスと同じ）。
fn row_key(item: &DocumentListItem) -> String {
    format!("{}/{}", item.project_slug.as_str(), item.slug.as_str())
}

/// 並び替えている列の印。
fn sort_mark(aria: Option<&str>) -> &'static str {
    match aria {
        Some("ascending") => "↑",
        Some("descending") => "↓",
        _ => "",
    }
}

/// 絞り込み（「この中を絞り込む」「今週更新」「自分が更新」と、すべての資料ではプロジェクトの選択）。
#[component]
async fn filters<'a>(
    path: &'a str,
    params: &'a ListParams,
    projects: Option<&'a [ProjectSummary]>,
) -> Result<impl View> {
    Ok(view! {
        <div class="filters">
            <label class="filter-search">
                (Unescaped::new_unchecked(FILTER_SVG))
                <input type="search" placeholder="この中を絞り込む" aria-label="資料名・プロジェクト名で絞り込む" data-row-filter="">
            </label>
            <a class="pill" href=(params.toggle_this_week().href(path)) aria-current=(params.is_this_week.then_some("true"))>"今週更新"</a>
            <a class="pill" href=(params.toggle_mine().href(path)) aria-current=(params.is_mine.then_some("true"))>"自分が更新"</a>
            if let Some(projects) = projects {
                <form class="filter-project" method="get" action=(path)>
                    for (key, value) in params.without_project().pairs() {
                        <input type="hidden" name=(key) value=(value)>
                    }
                    <label>
                        <span class="visually-hidden">"プロジェクト"</span>
                        <select class="select" name="project" data-autosubmit="">
                            <option value="">"すべてのプロジェクト"</option>
                            #[key(summary.project.id.to_string())]
                            for summary in projects {
                                <option
                                    value=(summary.project.slug.as_str())
                                    selected=(params.project.as_ref() == Some(&summary.project.slug))
                                >(summary.project.name.as_str())</option>
                            }
                        </select>
                    </label>
                    <noscript><button type="submit" class="button button--outline">"絞り込む"</button></noscript>
                </form>
            }
        </div>
    })
}

/// 一覧が空のとき。プロジェクトなら最初の登録方法を案内する。
#[component]
async fn empty_state(empty: Empty) -> Result<impl View> {
    Ok(view! {
        match empty {
            Empty::NoProjects => {
                <section class="panel empty">
                    <h2>"まだプロジェクトがありません"</h2>
                    <p>"資料はプロジェクトにまとめて登録します。まずプロジェクトを作りましょう。"</p>
                    <div><a class="button button--primary" href="/new">"＋ プロジェクトを作る"</a></div>
                </section>
            },
            Empty::NoDocuments => {
                <section class="panel empty">
                    <h2>"まだ資料がありません"</h2>
                    <p>"サイドバーからプロジェクトを開いて、最初の資料を登録しましょう。"</p>
                </section>
            },
            Empty::Project { name, command, has_author } => {
                <section class="panel empty">
                    <h2>"最初の資料を登録しましょう"</h2>
                    <p>(format!("「{name}」にはまだ資料がありません。次のどれかの方法で登録できます。"))</p>
                    <ol class="register-methods">
                        <li class="register-method">
                            <span class="register-icon">(Unescaped::new_unchecked(UPLOAD_SVG))</span>
                            <div>
                                <h3>"ファイルをドロップ"</h3>
                                <p>"HTML ファイルを画面のどこにでもドラッグして離します。"</p>
                            </div>
                        </li>
                        <li class="register-method">
                            <span class="register-icon">(Unescaped::new_unchecked(PASTE_SVG))</span>
                            <div>
                                <h3>"HTML を貼り付け"</h3>
                                <p>"HTML をコピーして、この画面で " <kbd>"⌘V"</kbd> " を押します。"</p>
                            </div>
                        </li>
                        <li class="register-method">
                            <span class="register-icon">(Unescaped::new_unchecked(AI_SVG))</span>
                            <div class="register-method-body">
                                <h3>"AI から直接登録する"</h3>
                                <p>"Claude Code などに Galley をつなぐと、作った資料を AI がそのまま登録できます。次のコマンドで登録します。"</p>
                                <div class="command">
                                    <code>(command.clone())</code>
                                    <button type="button" class="button button--outline button--small" data-copy-text=(command)>
                                        <span data-copy-label="">"コピー"</span>
                                    </button>
                                </div>
                                <p class="command-note">
                                    if has_author {
                                        "最後の名前が、AI が登録した資料の更新者として残ります。"
                                    } else {
                                        "最後の「あなたの名前」を自分の名前に変えてください。AI が登録した資料の更新者として残ります。"
                                    }
                                    " " <a href="/connect">"接続の手引き"</a>
                                </p>
                            </div>
                        </li>
                    </ol>
                </section>
            },
        }
    })
}

const FILTER_SVG: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>"#;
const SETTINGS_SVG: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>"#;
const UPLOAD_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 16V4"/><path d="M7 9l5-5 5 5"/><path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3"/></svg>"#;
const PASTE_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="6" y="4" width="12" height="17" rx="2"/><path d="M9 4V3h6v1"/><path d="M9 10h6"/><path d="M9 14h4"/></svg>"#;
const AI_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v4"/><path d="M12 17v4"/><path d="M5 12H3"/><path d="M21 12h-2"/><rect x="7" y="7" width="10" height="10" rx="2"/></svg>"#;
