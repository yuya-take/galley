//! 見つかった外部リソースと、利用者・AI に返す文言。

use std::fmt;

use super::url::library_name;

/// 拒否したときの理由。MCP では AI がそのまま作り直せるよう、この文言を返す。
pub const REJECTION_REASON: &str = "外部リソースを含まない単一 HTML にしてください";

/// 外部リソースの種類。画面には [`ResourceKind::label`] を出す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResourceKind {
    Script,
    Stylesheet,
    Font,
    Image,
    Media,
    Embed,
    Navigation,
    Other,
}

impl ResourceKind {
    /// 利用者が分かる言葉。
    pub fn label(self) -> &'static str {
        match self {
            Self::Script => "スクリプト",
            Self::Stylesheet => "スタイルシート",
            Self::Font => "Web フォント",
            Self::Image => "画像",
            Self::Media => "動画・音声",
            Self::Embed => "埋め込み（iframe など）",
            Self::Navigation => "ページの移動",
            Self::Other => "その他のファイル",
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// 外部から読み込んでいる1か所。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalResource {
    /// 1 から数えた行番号。
    pub line: usize,
    pub kind: ResourceKind,
    /// 該当するコード（空白を詰め、長ければ切り詰めたもの）。
    pub code: String,
    /// 読み込もうとしている URL。
    pub url: String,
}

/// 見つかった外部リソースの一覧。多すぎるときは先頭の [`ExternalResources::MAX_LISTED`] 件だけ持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalResources {
    resources: Vec<ExternalResource>,
    total: usize,
}

impl ExternalResources {
    /// 一覧に持つ件数の上限。
    pub const MAX_LISTED: usize = 100;

    pub(super) fn new(resources: Vec<ExternalResource>, total: usize) -> Self {
        Self { resources, total }
    }

    /// 該当箇所（行番号の順）。
    pub fn resources(&self) -> &[ExternalResource] {
        &self.resources
    }

    /// 見つかった件数。一覧に載せきれなかったものも数える。
    pub fn total(&self) -> usize {
        self.total
    }

    /// 見つかった種類（重複なし、[`ResourceKind`] の順）。
    pub fn kinds(&self) -> Vec<ResourceKind> {
        let mut kinds: Vec<ResourceKind> = self.resources.iter().map(|r| r.kind).collect();
        kinds.sort();
        kinds.dedup();
        kinds
    }

    /// 見つかった種類に合わせた、AI への作り直しの依頼文。
    ///
    /// スクリプトは URL からライブラリ名が分かればその名前を使う（例：`…/npm/chart.js@4` なら `chart.js`）。
    pub fn ai_request(&self) -> String {
        let mut items: Vec<String> = Vec::new();
        let mut has_unnamed_script = false;
        for resource in &self.resources {
            if resource.kind != ResourceKind::Script {
                continue;
            }
            match library_name(&resource.url) {
                Some(name) if !items.contains(&name) => items.push(name),
                Some(_) => {}
                None => has_unnamed_script = true,
            }
        }
        let kinds = self.kinds();
        for &kind in &kinds {
            let listed = match kind {
                // ライブラリ名が分からなかったものだけ「スクリプト」と書く
                ResourceKind::Script => has_unnamed_script,
                // 読み込みではないので、後ろに別の文で書く
                ResourceKind::Navigation => false,
                ResourceKind::Stylesheet
                | ResourceKind::Font
                | ResourceKind::Image
                | ResourceKind::Media
                | ResourceKind::Embed
                | ResourceKind::Other => true,
            };
            if listed {
                items.push(kind.label().to_owned());
            }
        }

        let mut request = String::new();
        if !items.is_empty() {
            let joined = items.join("、");
            // 英字で終わるとき（ライブラリ名）は、続く「を」との間に空白を入れる
            let space = if joined.ends_with(|c: char| c.is_ascii_alphanumeric()) {
                " "
            } else {
                ""
            };
            request.push_str(&joined);
            request.push_str(space);
            request.push_str(
                "を外部から読み込まず、すべてを HTML の中に埋め込んだ、1ファイルで完結する HTML に作り直してください。",
            );
        }
        if kinds.contains(&ResourceKind::Navigation) {
            request.push_str("外部のページへ自動で移動する meta refresh も取り除いてください。");
        }
        request
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(kind: ResourceKind, url: &str) -> ExternalResource {
        ExternalResource {
            line: 1,
            kind,
            code: String::new(),
            url: url.to_owned(),
        }
    }

    #[test]
    fn ai_request_names_libraries_and_kinds() {
        let resources = ExternalResources::new(
            vec![
                resource(
                    ResourceKind::Script,
                    "https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js",
                ),
                resource(
                    ResourceKind::Font,
                    "https://fonts.googleapis.com/css2?family=Noto+Sans+JP",
                ),
                resource(ResourceKind::Image, "logo.png"),
                resource(ResourceKind::Image, "photo.png"),
            ],
            4,
        );
        assert_eq!(
            resources.ai_request(),
            "chart.js、Web フォント、画像を外部から読み込まず、すべてを HTML の中に埋め込んだ、1ファイルで完結する HTML に作り直してください。"
        );
    }

    #[test]
    fn ai_request_spaces_after_library_name() {
        let resources = ExternalResources::new(
            vec![resource(
                ResourceKind::Script,
                "https://cdn.jsdelivr.net/npm/chart.js@4",
            )],
            1,
        );
        assert!(resources.ai_request().starts_with("chart.js を外部から"));
    }

    #[test]
    fn ai_request_falls_back_to_kind_for_unknown_scripts() {
        let resources = ExternalResources::new(
            vec![
                resource(ResourceKind::Script, "https://cdn.jsdelivr.net/npm/d3@7"),
                resource(ResourceKind::Script, "app"),
            ],
            2,
        );
        assert_eq!(
            resources.ai_request(),
            "d3、スクリプトを外部から読み込まず、すべてを HTML の中に埋め込んだ、1ファイルで完結する HTML に作り直してください。"
        );
    }

    #[test]
    fn ai_request_mentions_meta_refresh() {
        let resources = ExternalResources::new(
            vec![resource(ResourceKind::Navigation, "https://example.com")],
            1,
        );
        assert_eq!(
            resources.ai_request(),
            "外部のページへ自動で移動する meta refresh も取り除いてください。"
        );
    }
}
