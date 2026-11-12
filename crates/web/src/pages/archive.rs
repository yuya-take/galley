//! アーカイブ（`/archive`）。アーカイブしたプロジェクトと資料の一覧と、元に戻す操作。
//!
//! キャンバスに無い画面なので、資料一覧と同じ表の形で作る。
//!
//! 資料を1件ずつアーカイブする操作（一覧のプレビュー欄と資料ビューアの「アーカイブ」、確かめる段階、
//! アーカイブした直後の通知）もここに置く。

use std::{cmp::Reverse, collections::HashSet, sync::Arc};

use galley_core::{
    app::error::AppError,
    domain::{
        document::{Document, DocumentListItem, DocumentQuery, DocumentSlug},
        project::{Project, ProjectSlug, ProjectSummary},
        shared::{ArchiveFilter, DocumentId, ProjectId},
    },
};
use jiff::{Timestamp, Zoned};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    runtime::{Event, Signal, procedure, signal},
    view::{Unescaped, View, component, view},
};

use super::{
    documents::found,
    project_form::{self, form_error},
};
use crate::{WebApp, author, date, icon};

#[page("/archive")]
pub async fn archive(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let mut projects = app.list_projects.execute(ArchiveFilter::Archived).await?;
    let mut documents = app
        .list_documents
        .execute(&DocumentQuery {
            archive: ArchiveFilter::Archived,
            ..DocumentQuery::default()
        })
        .await?;
    // アーカイブした日時が新しい順（直前にアーカイブしたものを探しやすくする）
    projects.sort_by_key(|summary| Reverse(summary.project.archived_at));
    documents.sort_by_key(|document| Reverse(document.archived_at));
    let archived_project_ids: HashSet<ProjectId> =
        projects.iter().map(|summary| summary.project.id).collect();

    let now = date::now();
    Ok(view! {
        <header class="page-header">
            <div class="page-title">
                <h1>"アーカイブ"</h1>
                <p class="page-description">"アーカイブしたプロジェクトと資料です。資料と版の履歴はそのまま残っていて、元に戻すと一覧とサイドバーにまた出ます。"</p>
            </div>
        </header>
        archived_projects(projects: &projects, now: &now)
        archived_documents(
            documents: &documents,
            archived_project_ids: &archived_project_ids,
            now: &now,
        )
    })
}

/// アーカイブしたプロジェクトの表。
#[component]
async fn archived_projects(projects: &[ProjectSummary], now: &Zoned) -> Result<impl View> {
    Ok(view! {
        <section class="archive-section" aria-labelledby="archived-projects">
            <h2 id="archived-projects">"プロジェクト" <span class="archive-count">(projects.len())</span></h2>
            if projects.is_empty() {
                <p class="panel archive-empty">"アーカイブしたプロジェクトはありません"</p>
            } else {
                <table class="archive-table">
                    <thead>
                        <tr>
                            <th scope="col">"プロジェクト"</th>
                            <th scope="col">"資料"</th>
                            <th scope="col">"アーカイブした日時"</th>
                            <th scope="col"><span class="visually-hidden">"操作"</span></th>
                        </tr>
                    </thead>
                    <tbody>
                        #[key(summary.project.id.to_string())]
                        for summary in projects {
                            project_row(summary: summary, now: now)
                        }
                    </tbody>
                </table>
            }
        </section>
    })
}

/// アーカイブした資料の表（資料一覧と同じ列に、アーカイブした日時と「元に戻す」）。
#[component]
async fn archived_documents(
    documents: &[DocumentListItem],
    archived_project_ids: &HashSet<ProjectId>,
    now: &Zoned,
) -> Result<impl View> {
    Ok(view! {
        <section class="archive-section" aria-labelledby="archived-documents">
            <h2 id="archived-documents">"資料" <span class="archive-count">(documents.len())</span></h2>
            if documents.is_empty() {
                <p class="panel archive-empty">"アーカイブした資料はありません"</p>
            } else {
                <table class="archive-table">
                    <thead>
                        <tr>
                            <th scope="col">"資料名"</th>
                            <th scope="col">"版"</th>
                            <th scope="col">"更新者"</th>
                            <th scope="col">"アーカイブした日時"</th>
                            <th scope="col"><span class="visually-hidden">"操作"</span></th>
                        </tr>
                    </thead>
                    <tbody>
                        #[key(document.id.to_string())]
                        for document in documents {
                            document_row(
                                document: document,
                                is_project_archived: archived_project_ids.contains(&document.project_id),
                                now: now,
                            )
                        }
                    </tbody>
                </table>
            }
        </section>
    })
}

#[component]
async fn project_row(summary: &ProjectSummary, now: &Zoned) -> Result<impl View> {
    let project = &summary.project;
    let id = project.id.to_string();
    Ok(view! {
        <tr>
            <td>
                <a class="archive-title" href=(format!("/{}/settings", project.slug))>
                    <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                    (project.name.as_str())
                </a>
                <div class="archive-meta">(format!("/{}", project.slug))</div>
            </td>
            <td class="archive-number">(format!("{}件", summary.document_count))</td>
            <td>archived_time(at: project.archived_at, now: now)</td>
            <td class="archive-action">
                restore_button(target: Restore::Project, id: &id, label: project.name.as_str())
            </td>
        </tr>
    })
}

#[component]
async fn document_row(
    document: &DocumentListItem,
    is_project_archived: bool,
    now: &Zoned,
) -> Result<impl View> {
    let id = document.id.to_string();
    Ok(view! {
        <tr>
            <td>
                <span class="archive-title archive-title--document">(document.title.as_str())</span>
                <div class="archive-meta">
                    <span class=(format!("crest--{}", document.project_color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                    <span>(document.project_name.as_str())</span>
                    if !document.revision_message.as_str().is_empty() {
                        <span aria-hidden="true">"·"</span>
                        <span class="archive-message">(document.revision_message.as_str())</span>
                    }
                </div>
                if is_project_archived {
                    <span class="tag">"プロジェクトもアーカイブ中"</span>
                }
            </td>
            <td><span class="version">(format!("第{}版", document.revision_number))</span></td>
            <td>
                <span class="author">
                    <span class="avatar" aria-hidden="true">(author::initial(&document.author_name))</span>
                    (document.author_name.as_str())
                </span>
            </td>
            <td>archived_time(at: document.archived_at, now: now)</td>
            <td class="archive-action">
                restore_button(target: Restore::Document, id: &id, label: document.title.as_str())
            </td>
        </tr>
    })
}

#[component]
async fn archived_time(at: Option<Timestamp>, now: &Zoned) -> Result<impl View> {
    Ok(view! {
        if let Some(at) = at {
            <time datetime=(date::machine(at)) title=(date::long(at, now.time_zone()))>(date::short(at, now))</time>
        }
    })
}

/// 元に戻す対象。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    Project,
    Document,
}

/// 「元に戻す」。戻したら読み込み直して一覧から消す。
#[component]
async fn restore_button(cx: &Cx, target: Restore, id: &str, label: &str) -> Result<impl View> {
    let error = signal(cx, String::new);
    let is_busy = signal(cx, || false);
    let id = id.to_owned();
    let aria_label = format!("「{label}」を元に戻す");
    Ok(view! {
        match target {
            Restore::Project => {
                <button
                    type="button"
                    class="button button--outline button--small"
                    aria-label=(aria_label)
                    :disabled=$(is_busy.get())
                    @click=$(async |_e: Event| {
                        is_busy.set(true);
                        let failure = restore_project(id).await;
                        if failure.is_none() {
                            raw!("location.reload()");
                        } else {
                            is_busy.set(false);
                            error.set(failure.unwrap());
                        }
                    })
                >"元に戻す"</button>
            },
            Restore::Document => {
                <button
                    type="button"
                    class="button button--outline button--small"
                    aria-label=(aria_label)
                    :disabled=$(is_busy.get())
                    @click=$(async |_e: Event| {
                        is_busy.set(true);
                        let failure = restore_document(id).await;
                        if failure.is_none() {
                            raw!("location.reload()");
                        } else {
                            is_busy.set(false);
                            error.set(failure.unwrap());
                        }
                    })
                >"元に戻す"</button>
            },
        }
        form_error(error: &error)
    })
}

/// アーカイブしたプロジェクトを元に戻す。失敗したら画面に出す文言を返す（成功なら `None`）。
#[procedure]
pub async fn restore_project(cx: &Cx, id: String) -> Result<Option<String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(restore_project_by_id(app, &id).await.err())
}

async fn restore_project_by_id(app: &WebApp, id: &str) -> Result<(), String> {
    let id = project_form::parse_project_id(id)?;
    app.restore_project
        .execute(id)
        .await
        .map(|_| ())
        .map_err(project_form::error_message)
}

/// アーカイブした資料を元に戻す。失敗したら画面に出す文言を返す（成功なら `None`）。
#[procedure]
pub async fn restore_document(cx: &Cx, id: String) -> Result<Option<String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(restore_document_by_id(app, &id).await.err())
}

async fn restore_document_by_id(app: &WebApp, id: &str) -> Result<(), String> {
    let id = DocumentId::parse(id)
        .map_err(|_| project_form::error_message(AppError::DocumentNotFound))?;
    app.restore_document
        .execute(id)
        .await
        .map(|_| ())
        .map_err(project_form::error_message)
}

/// 資料をアーカイブする前に出す説明。
const ARCHIVE_NOTE: &str =
    "アーカイブした資料は一覧に出なくなります。アーカイブの画面から戻せます。";

/// そのほかの操作のメニューの「アーカイブ」。押すと確認（[`archive_confirm`]）を出し、フォーカスを確認の
/// 「やめる」に移す（メニューを閉じるのは `assets/app.js`）。`key` は `<プロジェクト>/<資料>`。
#[component]
pub(super) async fn archive_menu_item<'a>(
    key: String,
    confirming: &'a Signal<String>,
) -> Result<impl View> {
    Ok(view! {
        <button
            type="button"
            class="menu-item"
            @click=$(|_e: Event| {
                confirming.set(key.to_owned());
                raw!("requestAnimationFrame(() => document.querySelector('[data-archive-confirm]:not([hidden]) button')?.focus())");
            })
        >"アーカイブ"</button>
    })
}

/// 資料をアーカイブする前の確認（「『提案書』をアーカイブしますか？」と、戻せることの説明）。
/// `confirming` が `<プロジェクト>/<資料>` のときだけ出す（プレビュー欄で別の行を選んだら隠れる）。
/// アーカイブしたら、通知付きのプロジェクトの資料一覧に移る。`class` は置く場所の見た目。
#[component]
pub(super) async fn archive_confirm<'a>(
    cx: &'a Cx,
    class: &'a str,
    project: String,
    document: String,
    title: &'a str,
    confirming: &'a Signal<String>,
) -> Result<impl View> {
    let key = format!("{project}/{document}");
    let is_busy = signal(cx, || false);
    let error = signal(cx, String::new);
    Ok(view! {
        <section
            class=(format!("archive-confirm {class}"))
            aria-label="アーカイブの確認"
            data-archive-confirm=""
            :hidden=$(confirming.get() != key)
        >
            <p class="archive-confirm-text">
                <strong>(format!("「{title}」をアーカイブしますか？"))</strong>
                " "
                (ARCHIVE_NOTE)
            </p>
            <div class="archive-confirm-actions">
                <button
                    type="button"
                    class="button button--outline button--small"
                    :disabled=$(is_busy.get())
                    @click=$(|_e: Event| {
                        confirming.set("".to_owned());
                        error.set("".to_owned());
                    })
                >"やめる"</button>
                <button
                    type="button"
                    class="button button--primary button--small"
                    :disabled=$(is_busy.get())
                    @click=$(async |_e: Event| {
                        is_busy.set(true);
                        error.set("".to_owned());
                        let result = archive_document(project, document).await;
                        if result.is_ok() {
                            // 移るまでボタンは押せないままにする
                            let url = result.unwrap();
                            raw!("location.assign(${url})", drop(url));
                        } else {
                            is_busy.set(false);
                            error.set(result.unwrap_err());
                        }
                    })
                >"アーカイブする"</button>
            </div>
            form_error(error: &error)
        </section>
    })
}

/// 資料をアーカイブする。成功したらプロジェクトの資料一覧の URL（アーカイブした直後の通知付き）、
/// 失敗したら画面に出す文言を返す。
#[procedure]
pub async fn archive_document(
    cx: &Cx,
    project: String,
    document: String,
) -> Result<Result<String, String>> {
    let app: &Arc<WebApp> = app_context(cx);
    Ok(archive_document_by_slug(app, &project, &document).await)
}

async fn archive_document_by_slug(
    app: &WebApp,
    project_slug: &str,
    document_slug: &str,
) -> Result<String, String> {
    // アーカイブしたプロジェクト・資料も探し、断る理由（core の文言）をそのまま出す
    let project = app
        .find_project
        .execute(project_slug)
        .await
        .map_err(project_form::error_message)?;
    let document = app
        .find_document
        .execute(project.id, document_slug)
        .await
        .map_err(project_form::error_message)?;
    let archived = app
        .archive_document
        .execute(document.id)
        .await
        .map_err(project_form::error_message)?;
    Ok(archived_path(&project.slug, &archived.slug))
}

/// アーカイブした直後に開く資料一覧の URL（`/<プロジェクト>?archived=<資料>`）。開くと通知を出す。
fn archived_path(project: &ProjectSlug, document: &DocumentSlug) -> String {
    format!("/{project}?archived={document}")
}

/// `?archived=<資料の slug>` の資料。このプロジェクトの資料で、いまアーカイブしてあるときだけ返す
/// （戻したあとの URL や、ほかのプロジェクトの資料では通知を出さない）。
pub(super) async fn archived_notice(
    app: &WebApp,
    project: &Project,
    slug: Option<&str>,
) -> Result<Option<Document>, AppError> {
    let Some(slug) = slug else {
        return Ok(None);
    };
    Ok(found(app.find_document.execute(project.id, slug).await)?.filter(Document::is_archived))
}

/// アーカイブした直後の通知（プロジェクトの資料一覧）。「元に戻す」で戻して読み込み直す。
/// 「閉じる」と、URL から `?archived=` を消す処理は `assets/app.js`（`data-notice`）。
#[component]
pub(super) async fn archived_toast<'a>(cx: &'a Cx, document: &'a Document) -> Result<impl View> {
    let is_busy = signal(cx, || false);
    let error = signal(cx, String::new);
    let id = document.id.to_string();
    Ok(view! {
        <div class="toast toast--page" role="status" data-notice="archived">
            <span class="toast-icon">(Unescaped::new_unchecked(icon::CHECK_SVG))</span>
            <p>(format!("「{}」をアーカイブしました", document.title))</p>
            <button
                type="button"
                class="button button--outline button--small"
                :disabled=$(is_busy.get())
                @click=$(async |_e: Event| {
                    is_busy.set(true);
                    let failure = restore_document(id).await;
                    if failure.is_none() {
                        raw!("location.reload()");
                    } else {
                        is_busy.set(false);
                        error.set(failure.unwrap());
                    }
                })
            >"元に戻す"</button>
            <button type="button" class="button button--outline button--small" data-action="close-notice">"閉じる"</button>
            form_error(error: &error)
        </div>
    })
}

#[cfg(test)]
mod tests {
    use galley_core::app::project::ProjectInput;

    use super::*;
    use crate::pages::test_app;

    type TestResult = std::result::Result<(), String>;

    #[tokio::test]
    async fn restore_project_brings_it_back() -> TestResult {
        let app = test_app::new().await;
        let project = app
            .create_project
            .execute(&ProjectInput {
                slug: "sales".to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .map_err(|e| e.to_string())?;
        app.archive_project
            .execute(project.id)
            .await
            .map_err(|e| e.to_string())?;

        restore_project_by_id(&app, &project.id.to_string()).await?;
        let active = app.sidebar_projects().await.map_err(|e| e.to_string())?;
        assert_eq!(active.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn restore_document_brings_it_back_to_the_list() -> TestResult {
        let app = test_app::new().await;
        let project = app
            .create_project
            .execute(&ProjectInput {
                slug: "sales".to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .map_err(|e| e.to_string())?;
        let document = app.archived_document(project.id, "提案書").await;
        let archived = DocumentQuery {
            archive: ArchiveFilter::Archived,
            ..DocumentQuery::default()
        };
        let listed = app
            .list_documents
            .execute(&archived)
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(listed.len(), 1);

        restore_document_by_id(&app, &document.id.to_string()).await?;
        let listed = app
            .list_documents
            .execute(&archived)
            .await
            .map_err(|e| e.to_string())?;
        assert!(listed.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn restore_rejects_unknown_ids() -> TestResult {
        let app = test_app::new().await;
        assert_eq!(
            restore_project_by_id(&app, "x").await,
            Err("プロジェクトが見つかりません".to_owned())
        );
        assert_eq!(
            restore_document_by_id(&app, &DocumentId::generate().to_string()).await,
            Err("資料が見つかりません".to_owned())
        );
        Ok(())
    }

    async fn create_project(app: &WebApp, slug: &str) -> std::result::Result<Project, String> {
        app.create_project
            .execute(&ProjectInput {
                slug: slug.to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .map_err(|e| e.to_string())
    }

    async fn notice(
        app: &WebApp,
        project: &Project,
        slug: Option<&str>,
    ) -> std::result::Result<Option<DocumentId>, String> {
        archived_notice(app, project, slug)
            .await
            .map(|document| document.map(|document| document.id))
            .map_err(|e| e.to_string())
    }

    #[tokio::test]
    async fn archive_document_opens_list_with_notice() -> TestResult {
        let app = test_app::new().await;
        let sales = create_project(&app, "sales").await?;
        let hr = create_project(&app, "hr").await?;
        let plan = app.document(sales.id, "plan", "事業計画", 2).await;
        app.document(sales.id, "memo", "メモ", 1).await;
        let other = app.document(hr.id, "hiring", "採用計画", 1).await;
        app.archive(&other).await;

        // アーカイブする前は通知を出さない
        assert_eq!(notice(&app, &sales, Some("plan")).await?, None);

        assert_eq!(
            archive_document_by_slug(&app, "sales", "plan").await?,
            "/sales?archived=plan"
        );
        let listed = app
            .list_documents
            .execute(&DocumentQuery {
                project_id: Some(sales.id),
                ..DocumentQuery::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let titles: Vec<&str> = listed.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, ["メモ"]);
        assert_eq!(notice(&app, &sales, Some("plan")).await?, Some(plan.id));

        // 合図が無い、アーカイブしていない、無い、ほかのプロジェクトの資料には出さない
        assert_eq!(notice(&app, &sales, None).await?, None);
        assert_eq!(notice(&app, &sales, Some("memo")).await?, None);
        assert_eq!(notice(&app, &sales, Some("nope")).await?, None);
        assert_eq!(notice(&app, &sales, Some("Not A Slug")).await?, None);
        assert_eq!(notice(&app, &sales, Some("hiring")).await?, None);

        // 戻したあと（通知の「元に戻す」）は出さない
        restore_document_by_id(&app, &plan.id.to_string()).await?;
        assert_eq!(notice(&app, &sales, Some("plan")).await?, None);
        Ok(())
    }

    #[tokio::test]
    async fn archive_document_rejects_archived_missing_and_archived_project() -> TestResult {
        let app = test_app::new().await;
        let sales = create_project(&app, "sales").await?;
        app.document(sales.id, "plan", "事業計画", 1).await;
        app.document(sales.id, "memo", "メモ", 1).await;

        archive_document_by_slug(&app, "sales", "plan").await?;
        assert_eq!(
            archive_document_by_slug(&app, "sales", "plan").await,
            Err("この資料はすでにアーカイブしています".to_owned())
        );
        assert_eq!(
            archive_document_by_slug(&app, "sales", "nope").await,
            Err("資料が見つかりません".to_owned())
        );
        assert_eq!(
            archive_document_by_slug(&app, "nope", "memo").await,
            Err("プロジェクトが見つかりません".to_owned())
        );

        app.archive_project
            .execute(sales.id)
            .await
            .map_err(|e| e.to_string())?;
        let rejected = archive_document_by_slug(&app, "sales", "memo").await;
        assert!(
            rejected.as_ref().is_err_and(|message| message
                .starts_with("アーカイブしたプロジェクトの資料は、1件ずつはアーカイブできません")),
            "{rejected:?}"
        );
        Ok(())
    }
}
