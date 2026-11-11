//! アーカイブ（`/archive`）。アーカイブしたプロジェクトと資料の一覧と、元に戻す操作。
//!
//! キャンバスに無い画面なので、資料一覧と同じ表の形で作る。

use std::{cmp::Reverse, collections::HashSet, sync::Arc};

use galley_core::{
    app::error::AppError,
    domain::{
        document::{DocumentListItem, DocumentQuery},
        project::ProjectSummary,
        shared::{ArchiveFilter, DocumentId, ProjectId},
    },
};
use jiff::{Timestamp, Zoned};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::page,
    runtime::{Event, procedure, signal},
    view::{Unescaped, View, component, view},
};

use super::project_form::{self, form_error};
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
}
