//! 資料を読む（`/{project}/{document}`、`/{project}/{document}/v/{number}`）。
//!
//! サイドバーを隠して（`assets/app.css` の `.app:has(.viewer)`）、資料を全面に表示する。資料は資料配信の
//! オリジンの URL を sandbox 付きの iframe で埋め込む。版を省略すると常に最新版、`/v/<番号>` でその版。
//!
//! 登録した直後は、URL に `?registered=<版の番号>` を付けて開くと「第6版として登録しました」の通知を出す
//! （[`registered_path`]。登録の画面と「この版に戻す」が付ける）。
//! 版の一覧の `[` `]` キーと、通知を閉じる操作は `assets/app.js`。

use std::sync::Arc;

use galley_core::{
    app::error::AppError,
    domain::{
        document::{Document, DocumentSlug},
        project::{Project, ProjectSlug},
        revision::{AuthorName, Revision, RevisionNumber, RevisionSource},
    },
};
use jiff::Zoned;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::RouterErrorExt, page, path_param, query_params, request},
    runtime::{Event, procedure, signal},
    view::{Unescaped, View, component, view},
};

use super::{
    documents::{found, host},
    project_form::{self, form_error},
};
use crate::{WebApp, author, date, icon};

mod param {
    topcoat::router::path_param!(pub project);
    topcoat::router::path_param!(pub document);
    topcoat::router::path_param!(pub number);
}

/// 登録した直後の通知を出す合図のクエリ（`?registered=6`）。
#[query_params]
struct ViewerQuery {
    registered: Option<String>,
}

/// 登録した直後に開く資料ビューアの URL（`/<プロジェクト>/<資料>?registered=<版の番号>`）。
/// 開くと「第N版として登録しました」の通知を出す。
pub fn registered_path(
    project: &ProjectSlug,
    document: &DocumentSlug,
    number: RevisionNumber,
) -> String {
    format!("/{project}/{document}?registered={number}")
}

/// 資料の最新版。
#[page("/{project}/{document}")]
pub async fn document_viewer(cx: &Cx) -> Result<impl View> {
    let viewer = load(cx, None).await?;
    let notice = registered_notice(cx, &viewer);
    Ok(view! {
        viewer_screen(viewer: &viewer, notice: notice)
    })
}

/// 資料の特定の版（第N版）。
#[page("/{project}/{document}/v/{number}")]
pub async fn revision_viewer(cx: &Cx) -> Result<impl View> {
    let number: &str = path_param::<param::Number>(cx);
    let viewer = load(cx, Some(number)).await?;
    let notice = registered_notice(cx, &viewer);
    Ok(view! {
        viewer_screen(viewer: &viewer, notice: notice)
    })
}

/// URL の資料と版を読む。見つからない・アーカイブしたものは 404 にする。
async fn load(cx: &Cx, number: Option<&str>) -> Result<Viewer> {
    let app: &Arc<WebApp> = app_context(cx);
    let project: &str = path_param::<param::Project>(cx);
    let document: &str = path_param::<param::Document>(cx);
    Ok(Viewer::load(app, project, document, number)
        .await?
        .ok_or_not_found()?)
}

/// `?registered=<番号>` があれば、登録直後の通知の文言。
fn registered_notice(cx: &Cx, viewer: &Viewer) -> Option<String> {
    let query = query_params::<ViewerQuery>(cx).ok()?;
    viewer
        .registered(query.registered.as_deref())
        .map(registered_message)
}

/// 「第6版として登録しました」。「この版に戻す」で作った版なら、戻した元の版も書く。
fn registered_message(revision: &Revision) -> String {
    match revision.restored_from {
        Some(from) => format!(
            "第{from}版の内容を、第{}版として登録しました",
            revision.number
        ),
        None => format!("第{}版として登録しました", revision.number),
    }
}

/// 表示する資料と版。
struct Viewer {
    project: Project,
    document: Document,
    /// 資料の版（新しい順）。先頭が最新版で、空にはならない。
    revisions: Vec<Revision>,
    /// 表示する版の、`revisions` での位置。
    shown: usize,
    /// URL で版を指定して開いたか（`/v/<番号>`）。
    has_number: bool,
}

impl Viewer {
    /// `number` を省略すると最新版。形が正しくない・見つからない・アーカイブしたものは `None`。
    async fn load(
        app: &WebApp,
        project_slug: &str,
        document_slug: &str,
        number: Option<&str>,
    ) -> Result<Option<Self>, AppError> {
        let number = match number.map(parse_number) {
            None => None,
            Some(Some(number)) => Some(number),
            Some(None) => return Ok(None),
        };
        let Some((project, document)) = find_active(app, project_slug, document_slug).await? else {
            return Ok(None);
        };
        let revisions = app.list_revisions.execute(&document, None).await?;
        let shown = match number {
            None => (!revisions.is_empty()).then_some(0),
            Some(number) => revisions
                .iter()
                .position(|revision| revision.number.get() == number),
        };
        Ok(shown.map(|shown| Self {
            project,
            document,
            revisions,
            shown,
            has_number: number.is_some(),
        }))
    }

    fn shown(&self) -> &Revision {
        &self.revisions[self.shown]
    }

    fn latest(&self) -> &Revision {
        &self.revisions[0]
    }

    /// 表示中の版の URL（「リンクをコピー」）。版を指定して開いたときは、最新版でもその版の URL にする
    /// （あとで版が増えても、共有した人が見た版を開くように）。
    fn shown_path(&self) -> String {
        if self.has_number {
            format!("{}/v/{}", self.document_path(), self.shown().number)
        } else {
            self.document_path()
        }
    }

    fn is_latest(&self) -> bool {
        self.shown == 0
    }

    /// ひとつ前（古い）の版。`[` で移る。
    fn older(&self) -> Option<&Revision> {
        self.revisions.get(self.shown + 1)
    }

    /// ひとつ後（新しい）の版。`]` で移る。
    fn newer(&self) -> Option<&Revision> {
        self.shown
            .checked_sub(1)
            .and_then(|index| self.revisions.get(index))
    }

    fn project_path(&self) -> String {
        format!("/{}", self.project.slug)
    }

    /// 最新版を表示する URL（版を省略した URL）。
    fn document_path(&self) -> String {
        format!("/{}/{}", self.project.slug, self.document.slug)
    }

    /// 版を表示する URL。最新版は版を省略した URL にする（あとで版が増えても最新を開くように）。
    fn revision_path(&self, revision: &Revision) -> String {
        if revision.id == self.latest().id {
            self.document_path()
        } else {
            format!("{}/v/{}", self.document_path(), revision.number)
        }
    }

    /// `?registered=<番号>` の版。最新版を表示していて、番号が最新版と同じときだけ（古い通知は出さない）。
    fn registered(&self, number: Option<&str>) -> Option<&Revision> {
        let number = parse_number(number?)?;
        (self.is_latest() && self.latest().number.get() == number).then(|| self.latest())
    }
}

/// URL の版の番号（`/v/3` の `3`）。同じ版に URL が何通りもできないよう、数字だけで 0 から始まらないものに限る
/// （`03` や `+3` は受け付けない）。
fn parse_number(text: &str) -> Option<u32> {
    if text.starts_with('0') || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// アーカイブしていないプロジェクトと資料を、URL の slug から探す。
async fn find_active(
    app: &WebApp,
    project_slug: &str,
    document_slug: &str,
) -> Result<Option<(Project, Document)>, AppError> {
    let Some(project) = found(app.find_project.execute(project_slug).await)?
        .filter(|project| !project.is_archived())
    else {
        return Ok(None);
    };
    let document = found(app.find_document.execute(project.id, document_slug).await)?
        .filter(|document| !document.is_archived());
    Ok(document.map(|document| (project, document)))
}

/// 上部バー、通知・過去の版の帯、資料。
#[component]
async fn viewer_screen<'a>(
    cx: &'a Cx,
    viewer: &'a Viewer,
    notice: Option<String>,
) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let has_author = author::from_headers(request::headers(cx)).is_some();
    let shown = viewer.shown();
    let host = host(cx);
    let frame_url = host.and_then(|host| app.viewer_url.revision_url(host, shown.id));
    let download_url = host.and_then(|host| app.viewer_url.download_url(host, shown.id));
    let title = viewer.document.title.as_str();
    let project = &viewer.project;
    let project_path = viewer.project_path();
    let now = date::now();

    Ok(view! {
        <div
            class="viewer"
            data-older-href=(viewer.older().map(|revision| viewer.revision_path(revision)))
            data-newer-href=(viewer.newer().map(|revision| viewer.revision_path(revision)))
        >
            <header class="viewer-bar">
                <a class="button button--outline button--icon" href=(project_path.clone()) aria-label="資料一覧へ戻る" title="資料一覧へ戻る">
                    (Unescaped::new_unchecked(BACK_SVG))
                </a>
                <div class="viewer-heading">
                    <nav class="breadcrumb" aria-label="現在地">
                        <a class="breadcrumb-item" href=(project_path)>
                            <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                            (project.name.as_str())
                        </a>
                        <span class="breadcrumb-separator" aria-hidden="true">"/"</span>
                    </nav>
                    <h1 class="viewer-title" title=(title)>(title)</h1>
                </div>
                revision_menu(viewer: viewer, now: &now)
                <div class="viewer-actions">
                    <button type="button" class="button button--outline" data-copy-link=(viewer.shown_path())>
                        <span data-copy-label="">"リンクをコピー"</span>
                    </button>
                    if let Some(frame_url) = frame_url.clone() {
                        <a
                            class="button button--outline button--icon"
                            href=(frame_url)
                            target="_blank"
                            rel="noopener noreferrer"
                            aria-label="別タブで開く"
                            title="別タブで開く"
                        >(Unescaped::new_unchecked(EXTERNAL_SVG))</a>
                    }
                    if let Some(download_url) = download_url {
                        <a
                            class="button button--outline button--icon"
                            href=(download_url)
                            aria-label="HTML をダウンロード"
                            title="HTML をダウンロード"
                        >(Unescaped::new_unchecked(DOWNLOAD_SVG))</a>
                    }
                    // 登録の流れは #17 で作る
                    <button type="button" class="button button--primary" aria-disabled="true" title="登録は準備中です">"新しい版を登録"</button>
                </div>
            </header>
            if !viewer.is_latest() {
                past_banner(viewer: viewer, has_author: has_author, now: &now)
            }
            <div class="viewer-body">
                if let Some(frame_url) = frame_url {
                    // 資料は資料配信のオリジンから、sandbox を付けて表示する
                    <iframe
                        class="viewer-frame"
                        src=(frame_url)
                        sandbox="allow-scripts"
                        title=(format!("{title}（第{}版）", shown.number))
                        referrerpolicy="no-referrer"
                    ></iframe>
                } else {
                    <p class="viewer-empty">"資料を表示できません"</p>
                }
                if let Some(notice) = notice {
                    registered_toast(message: notice, path: viewer.document_path())
                }
            </div>
        </div>
    })
}

/// 版のボタン（「第5版 最新」「第3版 過去の版」）と、押すと開く版の一覧。
#[component]
async fn revision_menu<'a>(viewer: &'a Viewer, now: &'a Zoned) -> Result<impl View> {
    let shown = viewer.shown();
    let time_zone = now.time_zone().clone();
    Ok(view! {
        <details class="menu revision-menu">
            <summary class="button button--outline revision-button" title="版の一覧（[ ] で前後の版）">
                <span class="version">(format!("第{}版", shown.number))</span>
                if viewer.is_latest() {
                    <span class="latest">"最新"</span>
                } else {
                    <span class="past">"過去の版"</span>
                }
                (Unescaped::new_unchecked(CHEVRON_SVG))
            </summary>
            <div class="menu-list revision-menu-list">
                <p class="revision-menu-head">
                    <span>"版"</span>
                    <span><kbd>"["</kbd> " " <kbd>"]"</kbd> " で前後の版"</span>
                </p>
                <ol class="revision-options">
                    #[key(revision.id.to_string())]
                    for revision in &viewer.revisions {
                        let is_shown = revision.id == shown.id;
                        <li>
                            <a
                                class="revision-option"
                                href=(viewer.revision_path(revision))
                                aria-current=(is_shown.then_some("page"))
                            >
                                <span class="version">(format!("第{}版", revision.number))</span>
                                <span class="revision-body">
                                    <span class="revision-message">(revision_message(revision))</span>
                                    <span class="revision-meta">
                                        <span class="avatar avatar--small" aria-hidden="true">(author::initial(&revision.author_name))</span>
                                        (revision.author_name.as_str())
                                        " · "
                                        <time datetime=(date::machine(revision.created_at)) title=(date::long(revision.created_at, &time_zone))>(date::short(revision.created_at, now))</time>
                                    </span>
                                </span>
                                if is_shown {
                                    <span class="revision-check">
                                        (Unescaped::new_unchecked(CHECK_SVG))
                                        <span class="visually-hidden">"表示中"</span>
                                    </span>
                                }
                            </a>
                        </li>
                    }
                </ol>
                <a class="revision-menu-foot" href=(format!("{}/history", viewer.document_path()))>"版の履歴を開く"</a>
            </div>
        </details>
    })
}

fn revision_message(revision: &Revision) -> String {
    match revision.message.as_str() {
        "" => "変更メモなし".to_owned(),
        message => message.to_owned(),
    }
}

/// 過去の版を表示しているときの帯（「第3版（9月20日 18:40、田中）を表示しています。最新は第5版です。」）と、
/// 「最新を表示」「この版に戻す」。戻す前に帯の中で確かめる。名前が未設定なら、先に名前を聞く。
#[component]
async fn past_banner<'a>(
    cx: &'a Cx,
    viewer: &'a Viewer,
    has_author: bool,
    now: &'a Zoned,
) -> Result<impl View> {
    let shown = viewer.shown();
    let latest = viewer.latest();
    let is_confirming = signal(cx, || false);
    let is_busy = signal(cx, || false);
    let error = signal(cx, String::new);
    let project = viewer.project.slug.to_string();
    let document = viewer.document.slug.to_string();
    let number = shown.number.get();
    let text = format!(
        "第{}版（{}、{}）を表示しています。最新は第{}版です。",
        shown.number,
        date::medium(shown.created_at, now),
        shown.author_name,
        latest.number
    );
    let confirm = format!(
        "この版に戻すと、同じ内容の「第{}版」が追加されます。第{}版以前もそのまま残ります。",
        latest.number.next(),
        latest.number
    );

    Ok(view! {
        <section class="viewer-banner" aria-label="過去の版">
            <div class="viewer-banner-line">
                <p class="viewer-banner-text">(text)</p>
                <div class="viewer-banner-actions" :hidden=$(is_confirming.get())>
                    <a class="button button--outline button--small" href=(viewer.document_path())>"最新を表示"</a>
                    if has_author {
                        <button
                            type="button"
                            class="button button--primary button--small"
                            @click=$(|_e: Event| is_confirming.set(true))
                        >"この版に戻す"</button>
                    } else {
                        <button
                            type="button"
                            class="button button--primary button--small"
                            data-action="change-author"
                            title="戻す前に、更新者として残る名前を設定します"
                        >"この版に戻す"</button>
                    }
                </div>
            </div>
            <div class="viewer-confirm" :hidden=$(!is_confirming.get())>
                <p>(confirm)</p>
                <div class="viewer-banner-actions">
                    <button
                        type="button"
                        class="button button--outline button--small"
                        :disabled=$(is_busy.get())
                        @click=$(|_e: Event| {
                            is_confirming.set(false);
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
                            let result = revert_to_revision(project, document, number).await;
                            if result.is_ok() {
                                let url = result.unwrap();
                                raw!("location.assign(${url})", drop(url));
                            } else {
                                is_busy.set(false);
                                error.set(result.unwrap_err());
                            }
                        })
                    >"この版に戻す"</button>
                </div>
            </div>
            form_error(error: &error)
        </section>
    })
}

/// 登録した直後の通知。「閉じる」と、URL から `?registered=` を消す処理は `assets/app.js`。
#[component]
async fn registered_toast(message: String, path: String) -> Result<impl View> {
    Ok(view! {
        <div class="viewer-toast" role="status" data-registered-notice="">
            <span class="viewer-toast-icon">(Unescaped::new_unchecked(CHECK_SVG))</span>
            <p>(message)</p>
            <button type="button" class="button button--outline button--small" data-copy-link=(path)>
                <span data-copy-label="">"リンクをコピー"</span>
            </button>
            <button type="button" class="button button--outline button--small" data-action="close-notice">"閉じる"</button>
        </div>
    })
}

/// 過去の版に戻す（「この版に戻す」）。過去の版と同じ内容の新しい版を追加する。
///
/// 成功したら最新版の資料ビューアの URL（登録直後の通知付き）、失敗したら画面に出す文言を返す。
/// 版の履歴（#16）からも使う。更新者名は Cookie から読む。
#[procedure]
pub async fn revert_to_revision(
    cx: &Cx,
    project: String,
    document: String,
    number: u32,
) -> Result<Result<String, String>> {
    let app: &Arc<WebApp> = app_context(cx);
    let author = author::from_headers(request::headers(cx));
    Ok(revert(app, author.as_ref(), &project, &document, number).await)
}

async fn revert(
    app: &WebApp,
    author: Option<&AuthorName>,
    project_slug: &str,
    document_slug: &str,
    number: u32,
) -> Result<String, String> {
    let author = author.ok_or_else(|| "戻す前に、あなたの名前を設定してください".to_owned())?;
    let (project, document) = find_active(app, project_slug, document_slug)
        .await
        .map_err(error_message)?
        .ok_or_else(|| error_message(AppError::DocumentNotFound))?;
    let registered = app
        .revert_to_revision
        .execute(document.id, number, author.as_str(), RevisionSource::Web)
        .await
        .map_err(error_message)?;
    Ok(registered_path(
        &project.slug,
        &registered.document.slug,
        registered.revision.number,
    ))
}

/// ユースケースのエラーを、画面に出す文言にする。
fn error_message(error: AppError) -> String {
    match error {
        AppError::Repository(_) | AppError::Storage(_) => {
            tracing::error!(%error, "過去の版に戻せませんでした");
            "戻せませんでした。時間をおいて、もう一度試してください".to_owned()
        }
        // ほかは core の文言（「第5版は最新の版なので、戻せません」など）をそのまま出す
        error => project_form::error_message(error),
    }
}

const BACK_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M19 12H5"/><path d="M11 18l-6-6 6-6"/></svg>"#;
const EXTERNAL_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 4h6v6"/><path d="M20 4l-9 9"/><path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/></svg>"#;
const DOWNLOAD_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 4v12"/><path d="M7 11l5 5 5-5"/><path d="M4 20h16"/></svg>"#;
const CHEVRON_SVG: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>"#;
const CHECK_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M5 12l5 5 9-10"/></svg>"#;

#[cfg(test)]
mod tests {
    use galley_core::{app::project::ProjectInput, domain::project::Project};

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

    async fn viewer(app: &TestApp, number: Option<&str>) -> Option<Viewer> {
        Viewer::load(app, "sales", "plan", number)
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn author() -> AuthorName {
        AuthorName::parse("田中").unwrap_or_else(|e| panic!("{e}"))
    }

    /// 営業部（sales）に、第3版まである資料（plan）を作る。
    async fn app_with_document() -> TestApp {
        let app = test_app::new().await;
        let project = project(&app, "sales").await;
        app.document(project.id, "plan", "事業計画", 3).await;
        app
    }

    #[tokio::test]
    async fn shows_latest_without_number() -> TestResult {
        let app = app_with_document().await;
        let viewer = viewer(&app, None).await.ok_or("見つからない")?;
        assert_eq!(viewer.shown().number.get(), 3);
        assert!(viewer.is_latest());
        assert_eq!(
            viewer.older().map(|r| viewer.revision_path(r)).as_deref(),
            Some("/sales/plan/v/2")
        );
        assert!(viewer.newer().is_none());
        assert_eq!(viewer.revision_path(viewer.shown()), "/sales/plan");
        assert_eq!(viewer.shown_path(), "/sales/plan");
        assert_eq!(viewer.project_path(), "/sales");
        Ok(())
    }

    #[tokio::test]
    async fn shows_past_revision_by_number() -> TestResult {
        let app = app_with_document().await;
        let viewer = viewer(&app, Some("2")).await.ok_or("見つからない")?;
        assert_eq!(viewer.shown().number.get(), 2);
        assert_eq!(viewer.shown().message.as_str(), "第2版");
        assert_eq!(viewer.shown_path(), "/sales/plan/v/2");
        assert!(!viewer.is_latest());
        assert_eq!(
            viewer.older().map(|r| viewer.revision_path(r)).as_deref(),
            Some("/sales/plan/v/1")
        );
        // 最新版へは版を省略した URL で移る
        assert_eq!(
            viewer.newer().map(|r| viewer.revision_path(r)).as_deref(),
            Some("/sales/plan")
        );

        let first = Viewer::load(&app, "sales", "plan", Some("1"))
            .await
            .map_err(|e| e.to_string())?
            .ok_or("見つからない")?;
        assert!(first.older().is_none());
        // 最新版の番号を指定しても開ける
        let latest = Viewer::load(&app, "sales", "plan", Some("3"))
            .await
            .map_err(|e| e.to_string())?
            .ok_or("見つからない")?;
        assert!(latest.is_latest());
        // 版を指定して開いたら、リンクはその版の URL のまま
        assert_eq!(latest.shown_path(), "/sales/plan/v/3");
        Ok(())
    }

    #[tokio::test]
    async fn unknown_or_odd_numbers_are_not_found() {
        let app = app_with_document().await;
        for number in ["4", "0", "03", "+2", "-1", "2a", "", "99999999999"] {
            assert!(viewer(&app, Some(number)).await.is_none(), "{number:?}");
        }
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
        ] {
            let found = Viewer::load(&app, project, document, None)
                .await
                .unwrap_or_else(|e| panic!("{e}"));
            assert!(found.is_none(), "{project}/{document}");
        }

        let archived_project = project(&app, "legal").await;
        app.document(archived_project.id, "plan", "規程", 1).await;
        app.archive_project
            .execute(archived_project.id)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        let found = Viewer::load(&app, "legal", "plan", None)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn registered_notice_only_for_latest_number() -> TestResult {
        let app = app_with_document().await;
        let latest = viewer(&app, None).await.ok_or("見つからない")?;
        assert_eq!(
            latest
                .registered(Some("3"))
                .map(registered_message)
                .as_deref(),
            Some("第3版として登録しました")
        );
        for query in [None, Some("2"), Some("03"), Some("x")] {
            assert!(latest.registered(query).is_none(), "{query:?}");
        }
        let past = viewer(&app, Some("2")).await.ok_or("見つからない")?;
        assert!(past.registered(Some("3")).is_none());
        Ok(())
    }

    #[tokio::test]
    async fn revert_adds_revision_and_returns_registered_url() -> TestResult {
        let app = app_with_document().await;
        let url = revert(&app, Some(&author()), "sales", "plan", 1).await?;
        assert_eq!(url, "/sales/plan?registered=4");

        let viewer = viewer(&app, None).await.ok_or("見つからない")?;
        let latest = viewer.latest();
        assert_eq!(latest.number.get(), 4);
        assert_eq!(latest.author_name.as_str(), "田中");
        assert_eq!(
            viewer
                .registered(Some("4"))
                .map(registered_message)
                .as_deref(),
            Some("第1版の内容を、第4版として登録しました")
        );
        Ok(())
    }

    #[tokio::test]
    async fn revert_explains_why_it_cannot() {
        let app = app_with_document().await;
        assert_eq!(
            revert(&app, None, "sales", "plan", 1).await,
            Err("戻す前に、あなたの名前を設定してください".to_owned())
        );
        assert_eq!(
            revert(&app, Some(&author()), "sales", "plan", 3).await,
            Err("第3版は最新の版なので、戻せません".to_owned())
        );
        assert_eq!(
            revert(&app, Some(&author()), "sales", "plan", 9).await,
            Err("版が見つかりません".to_owned())
        );
        assert_eq!(
            revert(&app, Some(&author()), "sales", "nothing", 1).await,
            Err("資料が見つかりません".to_owned())
        );

        let viewer = viewer(&app, None).await;
        app.archive(
            &viewer
                .as_ref()
                .map(|v| v.document.clone())
                .unwrap_or_else(|| panic!("見つからない")),
        )
        .await;
        assert_eq!(
            revert(&app, Some(&author()), "sales", "plan", 1).await,
            Err("資料が見つかりません".to_owned())
        );
    }

    #[test]
    fn parse_number_accepts_only_canonical_numbers() {
        assert_eq!(parse_number("1"), Some(1));
        assert_eq!(parse_number("12"), Some(12));
        for text in ["", "0", "01", "+1", "-1", "1.0", " 1", "4294967296"] {
            assert_eq!(parse_number(text), None, "{text:?}");
        }
    }
}
