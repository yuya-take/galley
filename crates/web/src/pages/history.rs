//! 版の履歴（`/{project}/{document}/history`）。差分は出さない。
//!
//! 資料の版を新しい順に表で並べ、行を選ぶと右のプレビュー欄（[`revision_preview`] の shard）だけを
//! サーバーで描き直す。最初は最新の版を選んでおく。行の `↑` `↓`、`↵` とダブルクリック（その版を資料ビューアで
//! 開く）は、資料の一覧と同じ `assets/app.js` の処理（`.document-row`、`.document-select`）。
//!
//! 「この版に戻す」は資料ビューアと同じ procedure（[`revert_to_revision`]）を使い、戻したら登録直後の
//! 通知付きの資料ビューアに移る。

use std::sync::Arc;

use galley_core::{
    app::error::AppError,
    domain::{document::Document, project::Project, revision::Revision},
};
use jiff::Zoned;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, page, path_param, request},
    runtime::{Event, Signal, shard, signal},
    view::{Unescaped, View, ViewExt, component, view},
};

use super::{
    documents::host,
    project_form::form_error,
    viewer::{find_active, revert_note, revert_to_revision, revision_message},
};
use crate::{WebApp, author, date, icon};

mod param {
    topcoat::router::path_param!(pub project);
    topcoat::router::path_param!(pub document);
}

/// 版の履歴。見つからない・アーカイブした資料は 404 にする。
#[page("/{project}/{document}/history")]
pub async fn revision_history(cx: &Cx) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let project: &str = path_param::<param::Project>(cx);
    let document: &str = path_param::<param::Document>(cx);
    let history = History::load(app, project, document)
        .await?
        .ok_or_not_found()?;
    let latest = history.latest().number.get();
    let selected = signal(cx, move || latest);
    // 「この版に戻す」の失敗の文言。別の版を選んだら消すので、プレビュー欄の外で持つ
    let error = signal(cx, String::new);
    let now = date::now();

    Ok(view! {
        breadcrumb(history: &history)
        <header class="page-header">
            <div class="page-title">
                <h1>"版の履歴"</h1>
                <p class="page-description">"過去の版はすべて残ります。戻すと、その内容が新しい版として追加され、履歴は書き換わりません。"</p>
            </div>
        </header>
        <div class="documents">
            <section class="documents-main" aria-label="版の一覧">
                revision_table(history: &history, selected: &selected, error: &error, now: &now)
            </section>
            <aside class="preview" aria-label="選んだ版">
                revision_preview(
                    project: history.project.slug.to_string(),
                    document: history.document.slug.to_string(),
                    number: $(selected.get()),
                    error: error.clone(),
                )
            </aside>
        </div>
    })
}

/// パンくず（プロジェクト / 資料名）。資料名は資料ビューア（最新の版）へのリンク。
#[component]
async fn breadcrumb<'a>(history: &'a History) -> Result<impl View> {
    let project = &history.project;
    Ok(view! {
        <nav class="breadcrumb" aria-label="現在地">
            <a class="breadcrumb-item" href=(history.project_path())>
                <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                (project.name.as_str())
            </a>
            <span class="breadcrumb-separator" aria-hidden="true">"/"</span>
            <a class="breadcrumb-item" href=(history.document_path())>(history.document.title.as_str())</a>
        </nav>
    })
}

/// 版の表。行を押すと `selected` が変わり、プレビュー欄が描き直される。
#[component]
async fn revision_table<'a>(
    history: &'a History,
    selected: &'a Signal<u32>,
    error: &'a Signal<String>,
    now: &'a Zoned,
) -> Result<impl View> {
    let time_zone = now.time_zone().clone();
    Ok(view! {
        <div class="table-wrap">
            <table class="documents-table history-table">
                <thead>
                    <tr>
                        <th scope="col" class="col-version">"版"</th>
                        <th scope="col">"何を変えたか"</th>
                        <th scope="col" class="col-author">"更新者"</th>
                        <th scope="col" class="col-updated">"日時"</th>
                    </tr>
                </thead>
                <tbody>
                    #[key(revision.id.to_string())]
                    for revision in &history.revisions {
                        let number = revision.number.get();
                        <tr
                            class="document-row"
                            @click=$(|_e| {
                                selected.set(number);
                                error.set("".to_owned());
                            })
                        >
                            <td class="col-version">
                                <button
                                    type="button"
                                    class="document-select revision-select"
                                    data-href=(history.revision_path(revision))
                                    :aria-current=$(if selected.get() == number { "true" } else { "false" })
                                >
                                    <span class="version">(format!("第{number}版"))</span>
                                </button>
                            </td>
                            <td>
                                <span class="revision-change">
                                    <span class=(memo_class("revision-change-message", revision))>(revision_message(revision))</span>
                                    revision_marks(history: history, revision: revision)
                                </span>
                            </td>
                            <td class="col-author">
                                <span class="author">
                                    <span class="avatar" aria-hidden="true">(author::initial(&revision.author_name))</span>
                                    (revision.author_name.as_str())
                                </span>
                            </td>
                            <td class="col-updated">
                                <time datetime=(date::machine(revision.created_at)) title=(date::long(revision.created_at, &time_zone))>(date::short(revision.created_at, now))</time>
                            </td>
                        </tr>
                    }
                </tbody>
            </table>
        </div>
    })
}

/// 変更メモを出す要素の class。変更メモが空（「変更メモなし」と出す）なら `--empty` を足して薄くする。
fn memo_class(base: &str, revision: &Revision) -> String {
    if revision.message.as_str().is_empty() {
        format!("{base} {base}--empty")
    } else {
        base.to_owned()
    }
}

/// 「最新」「戻した版」の印。
#[component]
async fn revision_marks<'a>(history: &'a History, revision: &'a Revision) -> Result<impl View> {
    Ok(view! {
        if history.is_latest(revision) {
            <span class="latest">"最新"</span>
        }
        if let Some(from) = revision.restored_from {
            <span class="restored" title=(format!("第{from}版の内容に戻した版"))>"戻した版"</span>
        }
    })
}

/// 右のプレビュー欄。選んだ版のプレビュー、変更メモ、版・更新者・日時、「この版を開く」「この版に戻す」。
///
/// shard は画面を通らずに直接呼べる（`POST /api/revision-preview`）ので、受け取った値をここで検査し、
/// アーカイブしたプロジェクト・資料と、無い版は出さない。
#[shard("/api/revision-preview")]
pub async fn revision_preview(
    cx: &Cx,
    project: String,
    document: String,
    number: u32,
    error: Signal<String>,
) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let loaded = History::load(app, &project, &document).await?;
    let Some((history, revision)) = loaded.and_then(|history| {
        let revision = history.find(number)?.clone();
        Some((history, revision))
    }) else {
        return Ok(view! {
            <p class="preview-empty">"版を選ぶと、ここに表示します。"</p>
        }
        .boxed());
    };
    let is_latest = history.is_latest(&revision);
    let frame_url = host(cx).and_then(|host| app.viewer_url.revision_url(host, revision.id));
    let frame_title = format!(
        "{}（第{number}版）のプレビュー",
        history.document.title.as_str()
    );
    let note = if is_latest {
        "いま表示されている最新の版です。".to_owned()
    } else {
        revert_note(history.latest().number)
    };
    let now = date::now();

    Ok(view! {
        <div class="preview-frame-box">
            if let Some(frame_url) = frame_url {
                // 資料は資料配信のオリジンから、sandbox を付けて表示する（操作はできない縮小表示）
                <iframe
                    class="preview-frame"
                    src=(frame_url)
                    sandbox="allow-scripts"
                    title=(frame_title)
                    tabindex="-1"
                    referrerpolicy="no-referrer"
                ></iframe>
            } else {
                <p class="preview-empty">"プレビューを表示できません"</p>
            }
        </div>
        <div class="preview-body">
            <h2 class=(memo_class("history-memo", &revision))>(revision_message(&revision))</h2>
            <dl class="history-facts">
                <div>
                    <dt>"版"</dt>
                    <dd>
                        <span class="version">(format!("第{number}版"))</span>
                        revision_marks(history: &history, revision: &revision)
                    </dd>
                </div>
                <div>
                    <dt>"更新者"</dt>
                    <dd>
                        <span class="author">
                            <span class="avatar avatar--small" aria-hidden="true">(author::initial(&revision.author_name))</span>
                            (revision.author_name.as_str())
                        </span>
                    </dd>
                </div>
                <div>
                    <dt>"日時"</dt>
                    <dd><time datetime=(date::machine(revision.created_at))>(date::long(revision.created_at, now.time_zone()))</time></dd>
                </div>
            </dl>
            <div class="history-actions">
                <a class="button button--outline" href=(history.revision_path(&revision))>"この版を開く" <kbd>"↵"</kbd></a>
                if is_latest {
                    <button type="button" class="button button--primary" disabled="">"この版に戻す"</button>
                } else {
                    revert_button(project: project, document: document, number: number, error: &error)
                }
            </div>
            <p class="history-note">(note)</p>
            form_error(error: &error)
        </div>
    }
    .boxed())
}

/// 過去の版の「この版に戻す」。押したら戻して、登録直後の通知付きの資料ビューアに移る。
/// 名前が未設定なら、先に名前のダイアログを出す（`assets/app.js` の `data-action=change-author`）。
#[component]
async fn revert_button<'a>(
    cx: &'a Cx,
    project: String,
    document: String,
    number: u32,
    error: &'a Signal<String>,
) -> Result<impl View> {
    let has_author = author::from_headers(request::headers(cx)).is_some();
    let is_busy = signal(cx, || false);
    Ok(view! {
        if has_author {
            <button
                type="button"
                class="button button--primary"
                :disabled=$(is_busy.get())
                @click=$(async |_e: Event| {
                    is_busy.set(true);
                    error.set("".to_owned());
                    let result = revert_to_revision(project, document, number).await;
                    if result.is_ok() {
                        // 移るまでボタンは押せないままにする（二度押しで版が2つ増えないように）
                        let url = result.unwrap();
                        raw!("location.assign(${url})", drop(url));
                    } else {
                        is_busy.set(false);
                        error.set(result.unwrap_err());
                    }
                })
            >"この版に戻す"</button>
        } else {
            <button
                type="button"
                class="button button--primary"
                data-action="change-author"
                title="戻す前に、更新者として残る名前を設定します"
            >"この版に戻す"</button>
        }
    })
}

/// 履歴を出す資料と版。
struct History {
    project: Project,
    document: Document,
    /// 資料の版（新しい順）。先頭が最新の版で、空にはならない。
    revisions: Vec<Revision>,
}

impl History {
    /// 見つからない・アーカイブしたものは `None`。
    async fn load(
        app: &WebApp,
        project_slug: &str,
        document_slug: &str,
    ) -> Result<Option<Self>, AppError> {
        let Some((project, document)) = find_active(app, project_slug, document_slug).await? else {
            return Ok(None);
        };
        let revisions = app.list_revisions.execute(&document, None).await?;
        Ok((!revisions.is_empty()).then_some(Self {
            project,
            document,
            revisions,
        }))
    }

    fn latest(&self) -> &Revision {
        &self.revisions[0]
    }

    fn is_latest(&self, revision: &Revision) -> bool {
        revision.id == self.latest().id
    }

    /// 第`number`版。無ければ `None`。
    fn find(&self, number: u32) -> Option<&Revision> {
        self.revisions
            .iter()
            .find(|revision| revision.number.get() == number)
    }

    fn project_path(&self) -> String {
        format!("/{}", self.project.slug)
    }

    /// 最新の版を表示する資料ビューアの URL。
    fn document_path(&self) -> String {
        format!("/{}/{}", self.project.slug, self.document.slug)
    }

    /// 版を資料ビューアで開く URL（「この版を開く」）。最新の版は版を省略した URL にする
    /// （あとで版が増えても最新を開くように。資料ビューアの版の一覧と同じ）。
    fn revision_path(&self, revision: &Revision) -> String {
        if self.is_latest(revision) {
            self.document_path()
        } else {
            format!("{}/v/{}", self.document_path(), revision.number)
        }
    }
}

#[cfg(test)]
mod tests {
    use galley_core::{
        app::project::ProjectInput,
        domain::{
            project::Project,
            revision::{RevisionNumber, RevisionSource},
        },
    };

    use super::*;
    use crate::pages::test_app::{self, TestApp};

    type TestResult = std::result::Result<(), String>;

    async fn project(app: &TestApp, slug: &str) -> Project {
        app.create_project
            .execute(&ProjectInput {
                slug: slug.to_owned(),
                name: "営業部".to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    async fn history(app: &TestApp, project: &str, document: &str) -> Option<History> {
        History::load(app, project, document)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// 営業部（sales）に、第3版まである資料（plan）を作る。
    async fn app_with_document() -> TestApp {
        let app = test_app::new().await;
        let project = project(&app, "sales").await;
        app.document(project.id, "plan", "事業計画", 3).await;
        app
    }

    #[tokio::test]
    async fn lists_revisions_newest_first() -> TestResult {
        let app = app_with_document().await;
        let history = history(&app, "sales", "plan").await.ok_or("見つからない")?;
        let numbers: Vec<u32> = history.revisions.iter().map(|r| r.number.get()).collect();
        assert_eq!(numbers, [3, 2, 1]);
        assert_eq!(history.latest().number.get(), 3);
        assert_eq!(history.project_path(), "/sales");
        assert_eq!(history.document_path(), "/sales/plan");
        Ok(())
    }

    #[tokio::test]
    async fn opens_latest_without_number_and_past_with_number() -> TestResult {
        let app = app_with_document().await;
        let history = history(&app, "sales", "plan").await.ok_or("見つからない")?;
        let latest = history.find(3).ok_or("第3版が無い")?;
        let past = history.find(1).ok_or("第1版が無い")?;
        assert!(history.is_latest(latest));
        assert!(!history.is_latest(past));
        assert_eq!(history.revision_path(latest), "/sales/plan");
        assert_eq!(history.revision_path(past), "/sales/plan/v/1");
        for number in [0, 4, u32::MAX] {
            assert!(history.find(number).is_none(), "{number}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn archived_or_unknown_documents_are_not_found() {
        let app = app_with_document().await;
        let other = project(&app, "hr").await;
        let archived = app.document(other.id, "old", "古い資料", 1).await;
        app.archive(&archived).await;
        for (project, document) in [
            ("sales", "nothing"),
            ("nothing", "plan"),
            ("hr", "old"),
            ("hr", "plan"),
            ("sales", ""),
        ] {
            assert!(
                history(&app, project, document).await.is_none(),
                "{project}/{document}"
            );
        }

        let archived_project = project(&app, "legal").await;
        app.document(archived_project.id, "plan", "規程", 1).await;
        app.archive_project
            .execute(archived_project.id)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(history(&app, "legal", "plan").await.is_none());
    }

    #[tokio::test]
    async fn marks_restored_revision() -> TestResult {
        let app = app_with_document().await;
        let document = history(&app, "sales", "plan")
            .await
            .ok_or("見つからない")?
            .document;
        app.revert_to_revision
            .execute(document.id, 1, "田中", RevisionSource::Web)
            .await
            .map_err(|e| e.to_string())?;
        let history = history(&app, "sales", "plan").await.ok_or("見つからない")?;
        let latest = history.latest();
        assert_eq!(latest.number.get(), 4);
        assert_eq!(latest.restored_from.map(RevisionNumber::get), Some(1));
        assert_eq!(memo_class("history-memo", latest), "history-memo");
        // 戻す前の最新（第3版）はもう最新ではないので、開くときは版を付ける
        let previous = history.find(3).ok_or("第3版が無い")?;
        assert!(!history.is_latest(previous));
        assert_eq!(history.revision_path(previous), "/sales/plan/v/3");
        Ok(())
    }

    #[test]
    fn revert_note_counts_the_next_number() -> TestResult {
        let latest = RevisionNumber::new(5).map_err(|e| e.to_string())?;
        assert_eq!(
            revert_note(latest),
            "この版に戻すと、同じ内容の「第6版」が追加されます。第5版以前もそのまま残ります。"
        );
        Ok(())
    }
}
