//! 一覧の絞り込みと並び順。URL のクエリで持つので、リンクを共有すれば同じ一覧が開く。
//!
//! | クエリ | 意味 |
//! | --- | --- |
//! | `author=me` | 自分が更新（Cookie の更新者名で版を登録したことのある資料） |
//! | `week=1` | 今週更新 |
//! | `project=<slug>` | プロジェクトの選択（すべての資料だけ） |
//! | `sort=title` / `-title` / `updated` | 資料名の昇順・降順、更新の古い順。省略すると更新の新しい順 |

use galley_core::domain::{document::DocumentOrder, project::ProjectSlug};

/// 一覧の並び順（見出しの「資料名」「更新日時」で切り替える）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    TitleAsc,
    TitleDesc,
}

impl Sort {
    /// 知らない値は既定（更新の新しい順）にする。
    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("title") => Self::TitleAsc,
            Some("-title") => Self::TitleDesc,
            Some("updated") => Self::UpdatedAsc,
            _ => Self::UpdatedDesc,
        }
    }

    fn as_param(self) -> Option<&'static str> {
        match self {
            Self::UpdatedDesc => None,
            Self::UpdatedAsc => Some("updated"),
            Self::TitleAsc => Some("title"),
            Self::TitleDesc => Some("-title"),
        }
    }

    pub fn order(self) -> DocumentOrder {
        match self {
            Self::UpdatedDesc => DocumentOrder::UpdatedDesc,
            Self::UpdatedAsc => DocumentOrder::UpdatedAsc,
            Self::TitleAsc => DocumentOrder::TitleAsc,
            Self::TitleDesc => DocumentOrder::TitleDesc,
        }
    }

    /// 「資料名」を押したときの並び順。昇順のときだけ降順にする。
    pub fn by_title(self) -> Self {
        if self == Self::TitleAsc {
            Self::TitleDesc
        } else {
            Self::TitleAsc
        }
    }

    /// 「更新日時」を押したときの並び順。新しい順のときだけ古い順にする。
    pub fn by_updated(self) -> Self {
        if self == Self::UpdatedDesc {
            Self::UpdatedAsc
        } else {
            Self::UpdatedDesc
        }
    }

    /// 「資料名」の見出しの `aria-sort`。
    pub fn title_aria(self) -> Option<&'static str> {
        match self {
            Self::TitleAsc => Some("ascending"),
            Self::TitleDesc => Some("descending"),
            _ => None,
        }
    }

    /// 「更新日時」の見出しの `aria-sort`。
    pub fn updated_aria(self) -> Option<&'static str> {
        match self {
            Self::UpdatedDesc => Some("descending"),
            Self::UpdatedAsc => Some("ascending"),
            _ => None,
        }
    }
}

/// 一覧の条件。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListParams {
    pub is_mine: bool,
    pub is_this_week: bool,
    /// すべての資料で選んだプロジェクト。slug として正しいものだけ持つ。
    pub project: Option<ProjectSlug>,
    pub sort: Sort,
}

impl ListParams {
    /// クエリの値から読む。知らない値や正しくない slug は無視する。
    pub fn parse(
        author: Option<&str>,
        week: Option<&str>,
        project: Option<&str>,
        sort: Option<&str>,
    ) -> Self {
        Self {
            is_mine: author == Some("me"),
            is_this_week: week == Some("1"),
            project: project.and_then(|slug| ProjectSlug::parse(slug).ok()),
            sort: Sort::parse(sort),
        }
    }

    /// 絞り込みをしているか（並び順は含めない）。
    pub fn has_filters(&self) -> bool {
        self.is_mine || self.is_this_week || self.project.is_some()
    }

    /// `path` にこの条件のクエリを付けた URL。
    pub fn href(&self, path: &str) -> String {
        let query = self
            .pairs()
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        if query.is_empty() {
            path.to_owned()
        } else {
            format!("{path}?{query}")
        }
    }

    /// クエリの組（`project=<slug>` の slug は英小文字・数字・ハイフンだけなので、そのまま使える）。
    pub fn pairs(&self) -> Vec<(&'static str, &str)> {
        let mut pairs = Vec::new();
        if self.is_mine {
            pairs.push(("author", "me"));
        }
        if self.is_this_week {
            pairs.push(("week", "1"));
        }
        if let Some(project) = &self.project {
            pairs.push(("project", project.as_str()));
        }
        if let Some(sort) = self.sort.as_param() {
            pairs.push(("sort", sort));
        }
        pairs
    }

    pub fn toggle_mine(&self) -> Self {
        Self {
            is_mine: !self.is_mine,
            ..self.clone()
        }
    }

    pub fn toggle_this_week(&self) -> Self {
        Self {
            is_this_week: !self.is_this_week,
            ..self.clone()
        }
    }

    pub fn with_sort(&self, sort: Sort) -> Self {
        Self {
            sort,
            ..self.clone()
        }
    }

    pub fn without_project(&self) -> Self {
        Self {
            project: None,
            ..self.clone()
        }
    }

    /// 絞り込みを外す（並び順は残す）。
    pub fn without_filters(&self) -> Self {
        Self {
            sort: self.sort,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_values_and_ignores_others() {
        let params = ListParams::parse(Some("me"), Some("1"), Some("sales"), Some("-title"));
        assert!(params.is_mine);
        assert!(params.is_this_week);
        assert_eq!(
            params.project.as_ref().map(ProjectSlug::as_str),
            Some("sales")
        );
        assert_eq!(params.sort, Sort::TitleDesc);
        assert!(params.has_filters());

        let params = ListParams::parse(Some("佐藤"), Some("yes"), Some("Sales&x=1"), Some("size"));
        assert_eq!(params, ListParams::default());
        assert!(!params.has_filters());
        // 予約語はプロジェクトの slug にならない
        assert_eq!(
            ListParams::parse(None, None, Some("archive"), None).project,
            None
        );
    }

    #[test]
    fn href_keeps_only_non_default_values() {
        assert_eq!(ListParams::default().href("/"), "/");
        let params = ListParams::parse(Some("me"), None, None, Some("title"));
        assert_eq!(params.href("/"), "/?author=me&sort=title");
        assert_eq!(params.toggle_mine().href("/sales"), "/sales?sort=title");
        assert_eq!(
            params.toggle_this_week().href("/"),
            "/?author=me&week=1&sort=title"
        );
        assert_eq!(params.without_filters().href("/"), "/?sort=title");
        let params = ListParams::parse(None, None, Some("sales"), None);
        assert_eq!(params.href("/"), "/?project=sales");
        assert_eq!(params.without_project().href("/"), "/");
    }

    #[test]
    fn sort_headers_toggle() {
        assert_eq!(Sort::UpdatedDesc.by_title(), Sort::TitleAsc);
        assert_eq!(Sort::TitleAsc.by_title(), Sort::TitleDesc);
        assert_eq!(Sort::TitleDesc.by_title(), Sort::TitleAsc);
        assert_eq!(Sort::UpdatedDesc.by_updated(), Sort::UpdatedAsc);
        assert_eq!(Sort::UpdatedAsc.by_updated(), Sort::UpdatedDesc);
        assert_eq!(Sort::TitleAsc.by_updated(), Sort::UpdatedDesc);
        assert_eq!(Sort::UpdatedDesc.updated_aria(), Some("descending"));
        assert_eq!(Sort::UpdatedDesc.title_aria(), None);
        assert_eq!(Sort::TitleDesc.title_aria(), Some("descending"));
        // 既定の並び順はクエリに出さない
        let params = ListParams::default().with_sort(Sort::UpdatedAsc);
        assert_eq!(params.href("/"), "/?sort=updated");
        assert_eq!(params.with_sort(Sort::UpdatedDesc).href("/"), "/");
    }
}
