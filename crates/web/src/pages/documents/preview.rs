//! 一覧の右のプレビュー欄（幅 380px）。行を選ぶと、この shard だけをサーバーで描き直す。
//!
//! shard は一覧の画面を通らずに直接呼べる（`POST /api/document-preview`）ので、受け取った値を
//! ここで検査し、アーカイブしたプロジェクト・資料は出さない。

use std::sync::Arc;

use galley_core::domain::{
    document::Document,
    project::Project,
    revision::{Revision, RevisionNumber},
};
use topcoat::{
    Result,
    context::{Cx, app_context},
    runtime::{shard, signal},
    view::{Unescaped, View, ViewExt, view},
};

use super::{found, host};
use crate::{
    WebApp, author, date, icon,
    pages::archive::{archive_confirm, archive_menu_item},
};

/// プレビュー欄に出す直近の版の数。
const RECENT_REVISIONS: u32 = 3;

/// プレビュー欄。`key` は選んだ行の `<プロジェクト>/<資料>`。
#[shard("/api/document-preview")]
pub async fn document_preview(cx: &Cx, key: String) -> Result<impl View> {
    let app: &Arc<WebApp> = app_context(cx);
    let Some(preview) = Preview::load(app, &key).await? else {
        return Ok(view! {
            <p class="preview-empty">"資料を選ぶと、ここに表示します。"</p>
        }
        .boxed());
    };
    let Preview {
        project,
        document,
        revisions,
    } = preview;
    let path = format!("/{}/{}", project.slug.as_str(), document.slug.as_str());
    let title = document.title.as_str().to_owned();
    // 現在版は常に一番新しい版
    let number = revisions
        .first()
        .map_or(RevisionNumber::FIRST, |revision| revision.number);
    let frame_url = host(cx).and_then(|host| {
        app.viewer_url
            .revision_url(host, document.current_revision_id)
    });
    let now = date::now();
    let time_zone = now.time_zone().clone();
    // 「アーカイブ」の確認を出している資料（`<プロジェクト>/<資料>`。別の行を選んだら隠れる）
    let confirming = signal(cx, String::new);
    let (project_slug, document_slug) = (project.slug.to_string(), document.slug.to_string());

    Ok(view! {
        <div class="preview-frame-box">
            if let Some(frame_url) = frame_url {
                // 資料は資料配信のオリジンから、sandbox を付けて表示する（操作はできない縮小表示）
                <iframe
                    class="preview-frame"
                    src=(frame_url)
                    sandbox="allow-scripts"
                    title=(format!("{title}のプレビュー"))
                    tabindex="-1"
                    referrerpolicy="no-referrer"
                ></iframe>
            } else {
                <p class="preview-empty">"プレビューを表示できません"</p>
            }
        </div>
        <div class="preview-body">
            <h2 class="preview-title">(title.clone())</h2>
            <p class="preview-project">
                <span class=(format!("crest--{}", project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                <a href=(format!("/{}", project.slug.as_str()))>(project.name.as_str())</a>
            </p>
            <p class="preview-badges">
                <span class="version">(format!("第{number}版"))</span>
                <span class="latest">"最新"</span>
            </p>
            <div class="preview-actions">
                <a class="button button--primary" href=(path.clone())>"開く" <kbd>"↵"</kbd></a>
                <button type="button" class="button button--outline" data-copy-link=(path.clone())>
                    <span data-copy-label="">"リンクをコピー"</span>
                </button>
                <details class="menu">
                    <summary class="button button--outline button--icon" aria-label="そのほかの操作" title="そのほかの操作">
                        (Unescaped::new_unchecked(icon::MORE_SVG))
                    </summary>
                    <div class="menu-list">
                        <a class="menu-item" href=(path.clone()) target="_blank" rel="noopener">"新しいタブで開く"</a>
                        <a class="menu-item" href=(format!("{path}/history"))>"版の履歴"</a>
                        archive_menu_item(key: format!("{project_slug}/{document_slug}"), confirming: &confirming)
                    </div>
                </details>
            </div>
            archive_confirm(
                class: "preview-confirm",
                project: project_slug,
                document: document_slug,
                title: &title,
                confirming: &confirming,
            )
            <section class="preview-history" aria-labelledby="preview-history-title">
                <div class="preview-section-head">
                    <h3 id="preview-history-title">"版の履歴"</h3>
                    <a href=(format!("{path}/history"))>"すべて見る"</a>
                </div>
                <ol class="revision-list">
                    #[key(revision.id.to_string())]
                    for revision in revisions {
                        revision_item(revision: revision, time_zone: &time_zone, now: &now)
                    }
                </ol>
            </section>
        </div>
    }
    .boxed())
}

#[topcoat::view::component]
async fn revision_item<'a>(
    revision: Revision,
    time_zone: &'a jiff::tz::TimeZone,
    now: &'a jiff::Zoned,
) -> Result<impl View> {
    let message = match revision.message.as_str() {
        "" => "変更メモなし".to_owned(),
        message => message.to_owned(),
    };
    Ok(view! {
        <li class="revision-item">
            <span class="version">(format!("第{}版", revision.number))</span>
            <div class="revision-body">
                <span class="revision-message">(message)</span>
                <span class="revision-meta">
                    <span class="avatar avatar--small" aria-hidden="true">(author::initial(&revision.author_name))</span>
                    (revision.author_name.as_str())
                    " · "
                    <time datetime=(date::machine(revision.created_at)) title=(date::long(revision.created_at, time_zone))>(date::short(revision.created_at, now))</time>
                </span>
            </div>
        </li>
    })
}

/// プレビュー欄に出す資料。
struct Preview {
    project: Project,
    document: Document,
    /// 新しい順に [`RECENT_REVISIONS`] 件まで。
    revisions: Vec<Revision>,
}

impl Preview {
    /// `<プロジェクト>/<資料>` から読む。形が正しくない、見つからない、アーカイブしたものは `None`。
    async fn load(
        app: &WebApp,
        key: &str,
    ) -> Result<Option<Self>, galley_core::app::error::AppError> {
        let Some((project_slug, document_slug)) = key.split_once('/') else {
            return Ok(None);
        };
        let Some(project) = found(app.find_project.execute(project_slug).await)?
            .filter(|project| !project.is_archived())
        else {
            return Ok(None);
        };
        let Some(document) = found(app.find_document.execute(project.id, document_slug).await)?
            .filter(|document| !document.is_archived())
        else {
            return Ok(None);
        };
        let revisions = app
            .list_revisions
            .execute(&document, Some(RECENT_REVISIONS))
            .await?;
        Ok(Some(Self {
            project,
            document,
            revisions,
        }))
    }
}
