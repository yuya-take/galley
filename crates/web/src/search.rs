//! 「資料を探す」（⌘K）のダイアログ。資料名とプロジェクト名で探して、資料を開く。全画面のレイアウトに置く。
//!
//! 候補（アーカイブしていない資料）は、ダイアログを初めて開いたときに shard で読み込む。絞り込みと
//! キーボードでの選択はブラウザで行う（`assets/app.js`）。照合はサイドバーの「絞り込む」、一覧の
//! 「この中を絞り込む」と同じ関数を使う。資料の本文の検索（全文検索）は将来に回す。

use std::sync::Arc;

use galley_core::{
    app::error::AppError,
    domain::{
        document::{DocumentListItem, DocumentOrder, DocumentQuery},
        shared::ArchiveFilter,
    },
};
use jiff::Zoned;
use topcoat::{
    Result,
    context::{Cx, app_context},
    runtime::{shard, signal},
    view::{Unescaped, View, component, view},
};

use crate::{WebApp, date, icon};

/// 候補にする資料の上限（更新が新しい順）。これより古い資料は一覧から探してもらう。
const CANDIDATE_LIMIT: usize = 2000;

/// 検索のダイアログ。開くのは `assets/app.js`（⌘K / Ctrl+K、サイドバーの「資料を探す」）。
#[component]
pub async fn search_dialog(cx: &Cx) -> Result<impl View> {
    // 開いて入力欄にフォーカスが入ったら true にし、shard に候補を読み込ませる
    let has_opened = signal(cx, || false);
    Ok(view! {
        <dialog
            id="search-dialog"
            class="dialog search-dialog"
            aria-label="資料を探す"
            @focusin=$(|_e| has_opened.set(true))
        >
            <div class="search-field">
                (Unescaped::new_unchecked(SEARCH_SVG))
                <input
                    type="text"
                    class="search-input"
                    role="combobox"
                    aria-expanded="true"
                    aria-controls="search-results"
                    aria-autocomplete="list"
                    aria-label="資料名・プロジェクト名で探す"
                    placeholder="資料名・プロジェクト名で探す"
                    autocomplete="off"
                    spellcheck="false"
                    data-search-input=""
                >
                <kbd>"esc"</kbd>
            </div>
            <p class="search-label" data-search-label="">"最近更新した資料"</p>
            // 候補が届いたかを app.js がこの中だけ見張る（ラベルや案内の書き換えには反応させない）
            <div class="search-results-box" data-search-region="">
                document_search(has_opened: $(has_opened.get()))
            </div>
            <p class="search-status" data-search-status="" aria-live="polite">"読み込んでいます…"</p>
            <div class="search-footer" aria-hidden="true">
                <span><kbd>"↑"</kbd><kbd>"↓"</kbd>" 選ぶ"</span>
                <span><kbd>"↵"</kbd>" 開く"</span>
                <span><kbd>"esc"</kbd>" 閉じる"</span>
            </div>
        </dialog>
    })
}

/// 検索の候補。開く前（`has_opened` が false）は空の一覧を返し、資料を読まない。
///
/// shard は画面を通らずに直接呼べる（`POST /api/document-search`）が、受け取るのは真偽値だけで、
/// 返すのは「すべての資料」と同じく、アーカイブしていない資料だけ。
#[shard("/api/document-search")]
pub async fn document_search(cx: &Cx, has_opened: bool) -> Result<impl View> {
    let candidates = if has_opened {
        let app: &Arc<WebApp> = app_context(cx);
        Some(Candidates::load(app, CANDIDATE_LIMIT).await?)
    } else {
        None
    };
    let now = date::now();
    Ok(view! {
        match candidates {
            None => {
                <div class="search-results" id="search-results" role="listbox" aria-label="資料" data-search-results=""></div>
            },
            Some(candidates) => {
                <div
                    class="search-results"
                    id="search-results"
                    role="listbox"
                    aria-label="資料"
                    data-search-results=""
                    data-loaded=""
                    data-truncated=(candidates.is_truncated.then(|| CANDIDATE_LIMIT.to_string()))
                >
                    #[key(item.id.to_string())]
                    for item in &candidates.items {
                        search_option(item: item, now: &now)
                    }
                </div>
            },
        }
    })
}

/// 候補の1件。押すと資料を読む画面（`/<プロジェクト>/<資料>`）を開く。
#[component]
async fn search_option<'a>(item: &'a DocumentListItem, now: &'a Zoned) -> Result<impl View> {
    Ok(view! {
        <a
            class="search-option"
            role="option"
            id=(format!("search-option-{}", item.id))
            href=(format!("/{}/{}", item.project_slug.as_str(), item.slug.as_str()))
            tabindex="-1"
            aria-selected="false"
            data-search-text=(search_text(item))
            hidden=""
        >
            <span class="search-option-title">(item.title.as_str())</span>
            <span class="search-option-project">
                <span class=(format!("crest--{}", item.project_color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                (item.project_name.as_str())
            </span>
            <time class="search-option-time" datetime=(date::machine(item.updated_at))>(date::short(item.updated_at, now))</time>
        </a>
    })
}

/// 照合する文字列（資料名とプロジェクト名）。語は空白で区切るので、改行でつないでも2つにまたがって当たらない。
pub(crate) fn search_text(item: &DocumentListItem) -> String {
    format!("{}\n{}", item.title.as_str(), item.project_name.as_str())
}

/// 検索の候補（アーカイブしていない資料を、更新が新しい順に）。
struct Candidates {
    items: Vec<DocumentListItem>,
    /// 上限で切ったか（それより古い資料がある）。
    is_truncated: bool,
}

impl Candidates {
    async fn load(app: &WebApp, limit: usize) -> Result<Self, AppError> {
        let query = DocumentQuery {
            archive: ArchiveFilter::Active,
            order: DocumentOrder::UpdatedDesc,
            // 1件多く読んで、上限を超えるかを知る
            limit: Some(u32::try_from(limit.saturating_add(1)).unwrap_or(u32::MAX)),
            ..DocumentQuery::default()
        };
        let mut items = app.list_documents.execute(&query).await?;
        let is_truncated = items.len() > limit;
        items.truncate(limit);
        Ok(Self {
            items,
            is_truncated,
        })
    }
}

const SEARCH_SVG: &str = r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>"#;

#[cfg(test)]
mod tests {
    use galley_core::app::project::ProjectInput;

    use super::*;
    use crate::pages::test_app;

    type TestResult = std::result::Result<(), String>;

    async fn project(
        app: &test_app::TestApp,
        slug: &str,
        name: &str,
    ) -> galley_core::domain::project::Project {
        app.create_project
            .execute(&ProjectInput {
                slug: slug.to_owned(),
                name: name.to_owned(),
                description: String::new(),
                color: "red".to_owned(),
            })
            .await
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn titles(candidates: &Candidates) -> Vec<&str> {
        candidates
            .items
            .iter()
            .map(|item| item.title.as_str())
            .collect()
    }

    #[tokio::test]
    async fn candidates_leave_out_archived_documents_and_projects() -> TestResult {
        let app = test_app::new().await;
        let sales = project(&app, "sales", "営業企画").await;
        let old = project(&app, "old", "旧プロジェクト").await;
        app.document(sales.id, "提案書").await;
        app.archived_document(sales.id, "古い見積").await;
        app.document(old.id, "議事録").await;
        app.archive_project
            .execute(old.id)
            .await
            .map_err(|e| e.to_string())?;

        let candidates = Candidates::load(&app, CANDIDATE_LIMIT)
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(titles(&candidates), ["提案書"]);
        assert!(!candidates.is_truncated);
        assert_eq!(search_text(&candidates.items[0]), "提案書\n営業企画");
        Ok(())
    }

    #[tokio::test]
    async fn candidates_keep_the_newest_up_to_the_limit() -> TestResult {
        let app = test_app::new().await;
        let sales = project(&app, "sales", "営業企画").await;
        for title in ["一", "二", "三"] {
            app.document(sales.id, title).await;
        }

        let candidates = Candidates::load(&app, 2).await.map_err(|e| e.to_string())?;
        assert_eq!(titles(&candidates), ["三", "二"]);
        assert!(candidates.is_truncated);

        let candidates = Candidates::load(&app, 3).await.map_err(|e| e.to_string())?;
        assert_eq!(candidates.items.len(), 3);
        assert!(!candidates.is_truncated);
        Ok(())
    }
}
