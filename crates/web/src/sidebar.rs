//! 共通サイドバー（幅 248px）。資料を読む画面（#15）以外で出す。

use galley_core::domain::{project::ProjectSummary, revision::AuthorName};
use topcoat::{
    Result,
    context::Cx,
    router::request,
    view::{Unescaped, View, component, view},
};

use crate::{author, icon};

/// サイドバーのどの項目が現在地か。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Current<'a> {
    All,
    Mine,
    Archive,
    Project(&'a str),
    Other,
}

impl<'a> Current<'a> {
    fn of(path: &'a str, query: Option<&str>) -> Self {
        let is_mine = query
            .unwrap_or_default()
            .split('&')
            .any(|pair| pair == "author=me");
        match path.trim_end_matches('/') {
            "" if is_mine => Self::Mine,
            "" => Self::All,
            "/archive" => Self::Archive,
            "/connect" | "/new" => Self::Other,
            other => other
                .trim_start_matches('/')
                .split('/')
                .next()
                .filter(|slug| !slug.is_empty())
                .map_or(Self::Other, Self::Project),
        }
    }
}

fn aria_current(on: bool) -> Option<&'static str> {
    on.then_some("page")
}

#[component]
pub async fn sidebar(
    cx: &Cx,
    projects: &[ProjectSummary],
    author: Option<&AuthorName>,
) -> Result<impl View> {
    let uri = request::uri(cx);
    let current = Current::of(uri.path(), uri.query());
    Ok(view! {
        <nav class="sidebar" aria-label="メイン">
            <a class="logo" href="/" aria-label="Galley（すべての資料）">
                <span class="logo-mark">(Unescaped::new_unchecked(icon::LOGO_SVG))</span>
                <span class="logo-text">"Galley"</span>
            </a>
            // 検索そのものは #30
            <button type="button" class="search-button" data-action="search">
                (Unescaped::new_unchecked(SEARCH_SVG))
                <span>"資料を探す"</span>
                <kbd>"⌘K"</kbd>
            </button>
            <div class="nav">
                <a class="nav-item" href="/" aria-current=(aria_current(current == Current::All))>"すべての資料"</a>
                <a class="nav-item" href="/?author=me" aria-current=(aria_current(current == Current::Mine))>"自分が更新した資料"</a>
                <a class="nav-item" href="/archive" aria-current=(aria_current(current == Current::Archive))>"アーカイブ"</a>
            </div>
            <div class="projects">
                <div class="projects-header">
                    <span class="section-label" id="projects-label">"プロジェクト"</span>
                    <a class="icon-button" href="/new" aria-label="プロジェクトを作る">(Unescaped::new_unchecked(PLUS_SVG))</a>
                </div>
                <label class="project-filter">
                    (Unescaped::new_unchecked(FILTER_SVG))
                    <input type="text" placeholder="絞り込む" aria-label="プロジェクトを絞り込む" data-project-filter="">
                </label>
                project_list(projects: projects, current: current)
            </div>
            <a class="connect-link" href="/connect">
                (Unescaped::new_unchecked(CONNECT_SVG))
                <span>
                    <strong>"AIから登録する"</strong>
                    <small>"Claude Code などとつなぐ"</small>
                </span>
            </a>
            author_button(author: author)
        </nav>
    })
}

/// プロジェクトの一覧（紋章の色、名前、資料の件数）。
#[component]
async fn project_list<'a>(
    projects: &'a [ProjectSummary],
    current: Current<'a>,
) -> Result<impl View> {
    Ok(view! {
        if projects.is_empty() {
            <p class="projects-empty">"まだプロジェクトがありません"</p>
        } else {
            <ul class="project-list" aria-labelledby="projects-label">
                #[key(summary.project.id.to_string())]
                for summary in projects {
                    <li data-project-name=(summary.project.name.as_str())>
                        <a
                            class="project-item"
                            href=(format!("/{}", summary.project.slug))
                            aria-current=(aria_current(current == Current::Project(summary.project.slug.as_str())))
                        >
                            <span class=(format!("crest--{}", summary.project.color.as_str()))>(Unescaped::new_unchecked(icon::CREST_SVG))</span>
                            <span class="project-name">(summary.project.name.as_str())</span>
                            <span class="project-count" aria-label=(format!("{}件", summary.document_count))>(summary.document_count)</span>
                        </a>
                    </li>
                }
            </ul>
        }
    })
}

/// 更新者名と「名前を変える」。名前が無ければ設定を促す。
#[component]
async fn author_button(author: Option<&AuthorName>) -> Result<impl View> {
    Ok(view! {
        match author {
            Some(name) => {
                <button type="button" class="author-button" data-action="change-author">
                    <span class="avatar avatar--large" aria-hidden="true">(author::initial(name))</span>
                    <span class="author-name">(name.as_str())</span>
                    <span class="author-action">"名前を変える"</span>
                </button>
            },
            None => {
                <button type="button" class="author-button" data-action="change-author">
                    <span class="avatar avatar--large avatar--empty" aria-hidden="true">"?"</span>
                    <span class="author-name">"名前が未設定です"</span>
                    <span class="author-action">"設定する"</span>
                </button>
            },
        }
    })
}

const SEARCH_SVG: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/></svg>"#;
const PLUS_SVG: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14"/><path d="M5 12h14"/></svg>"#;
const FILTER_SVG: &str = r#"<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M4 6h16"/><path d="M7 12h10"/><path d="M10 18h4"/></svg>"#;
const CONNECT_SVG: &str = r#"<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v4"/><path d="M12 17v4"/><path d="M5 12H3"/><path d="M21 12h-2"/><rect x="7" y="7" width="10" height="10" rx="2"/></svg>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_follows_path_and_query() {
        assert_eq!(Current::of("/", None), Current::All);
        assert_eq!(Current::of("/", Some("author=me")), Current::Mine);
        assert_eq!(
            Current::of("/", Some("sort=title&author=me")),
            Current::Mine
        );
        assert_eq!(Current::of("/archive", None), Current::Archive);
        assert_eq!(Current::of("/sales", None), Current::Project("sales"));
        assert_eq!(
            Current::of("/sales/plan/v/2", None),
            Current::Project("sales")
        );
        assert_eq!(Current::of("/connect", None), Current::Other);
    }
}
