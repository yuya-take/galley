//! HTML を lol_html で読み、外部リソースの読み込みと `<title>` を取り出す。
//!
//! lol_html は属性値と一部のテキストを文字参照のまま返すので、判定の前に htmlize で戻す
//! （`h&#116;tps:` のような書き方ですり抜けないようにする）。

use std::{cell::RefCell, ops::Range};

use lol_html::{
    HtmlRewriter, Settings, element,
    html_content::{Element, TextType},
    text,
};

use super::{
    css::css_urls,
    resource::ResourceKind,
    script::{import_map_urls, module_urls},
    url::{DataContent, data_content, is_external, refresh_url, srcset_urls},
};

/// `srcdoc` や `data:` URL の中の HTML を、何段まで入れ子で調べるか。
const MAX_DEPTH: u8 = 3;

/// 見つかった読み込み。位置は調べた HTML の先頭からのバイト位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Finding {
    pub offset: usize,
    /// `offset` の行から、さらに何行下か（属性値の中の CSS など）。
    pub extra_lines: usize,
    pub kind: ResourceKind,
    pub code: String,
    pub url: String,
}

#[derive(Debug, Default)]
pub(super) struct Scan {
    pub findings: Vec<Finding>,
    pub title: Option<String>,
}

/// 読み込みを調べる。HTML として壊れていてもブラウザーと同じく読める範囲で調べる。
pub(super) fn scan(html: &str) -> Scan {
    scan_nested(html, 0)
}

fn scan_nested(html: &str, depth: u8) -> Scan {
    let state = RefCell::new(State {
        html,
        depth,
        findings: Vec::new(),
        title: TitleState::Waiting,
        text: None,
    });
    let mut rewriter = HtmlRewriter::new(
        Settings::new()
            .with_strict(false)
            .append_element_content_handler(element!("*", |el| {
                let mut state = state.borrow_mut();
                state.flush_text();
                state.inspect_element(&*el);
                Ok(())
            }))
            .append_element_content_handler(text!("style, script", |chunk| {
                let range = chunk.source_location().bytes();
                state
                    .borrow_mut()
                    .push_text(chunk.as_str(), range.start, chunk.text_type());
                Ok(())
            }))
            .append_element_content_handler(text!("title", |chunk| {
                state.borrow_mut().push_title(
                    chunk.as_str(),
                    chunk.text_type(),
                    chunk.last_in_text_node(),
                );
                Ok(())
            })),
        |_: &[u8]| {},
    );
    // 出力は捨て、ハンドラーもエラーを返さないので、失敗するのはメモリの上限に達したときだけ。
    // そのときもそこまでに見つけたものは報告する（最終的な防御は表示側の CSP）
    let _ = rewriter
        .write(html.as_bytes())
        .and_then(|()| rewriter.end());
    let mut state = state.into_inner();
    state.flush_text();
    let title = match state.title {
        TitleState::Done(title) => normalize_title(&title),
        TitleState::Reading(title) => normalize_title(&title),
        TitleState::Waiting => None,
    };
    Scan {
        findings: state.findings,
        title,
    }
}

enum TitleState {
    Waiting,
    Reading(String),
    Done(String),
}

/// `<style>` / `<script>` の中身。コメントや CDATA で分かれて届くので、次の要素まで貯めてから調べる。
///
/// SVG の中では文字参照を含みうるので、断片ごとに戻してから貯める。戻すと長さが変わるので、
/// 位置は「断片の開始位置」と「そこから何行下か」で表す。
struct TextBuffer {
    kind: TextKind,
    text: String,
    /// 断片の、貯めた文字列での開始位置と、元の HTML での開始位置。
    segments: Vec<(usize, usize)>,
}

impl TextBuffer {
    /// 貯めた文字列の位置を、元の HTML の位置（断片の開始位置と、そこから何行下か）にする。
    fn source_position(&self, offset: usize) -> (usize, usize) {
        let (text_start, source_start) = self
            .segments
            .iter()
            .rev()
            .find(|(start, _)| *start <= offset)
            .copied()
            .unwrap_or((0, 0));
        let extra_lines = self
            .text
            .get(text_start..offset)
            .map_or(0, |between| between.matches('\n').count());
        (source_start, extra_lines)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextKind {
    Style,
    Script,
    ImportMap,
}

struct State<'a> {
    html: &'a str,
    depth: u8,
    findings: Vec<Finding>,
    title: TitleState,
    text: Option<TextBuffer>,
}

impl State<'_> {
    fn inspect_element(&mut self, el: &Element<'_, '_>) {
        let tag = el.tag_name();
        let tag_range = el.source_location().bytes();
        match tag.as_str() {
            "style" => self.start_text(TextKind::Style),
            "script" => {
                let is_import_map = el.get_attribute("type").is_some_and(|t| {
                    htmlize::unescape_attribute(&t)
                        .trim()
                        .eq_ignore_ascii_case("importmap")
                });
                self.start_text(if is_import_map {
                    TextKind::ImportMap
                } else {
                    TextKind::Script
                });
            }
            _ => {}
        }

        let link = (tag == "link").then(|| LinkRel::of(el));
        for attr in el.attributes() {
            let value = htmlize::unescape_attribute(attr.value());
            let offset = attr
                .value_source_location()
                .map_or(tag_range.start, |l| l.bytes().start);
            let at = Location {
                offset,
                extra_lines: 0,
                code: Code::Source(tag_range.clone()),
            };
            self.inspect_attribute(el, &tag, link.as_ref(), &attr.name(), &value, &at);
        }
    }

    /// 1つの属性。読み込みに使う属性だけ調べる。
    fn inspect_attribute(
        &mut self,
        el: &Element<'_, '_>,
        tag: &str,
        link: Option<&LinkRel>,
        name: &str,
        value: &str,
        at: &Location,
    ) {
        match name {
            "src" => {
                if let Some(kind) = src_kind(tag) {
                    self.check(kind, value, at);
                }
            }
            "srcset" | "imagesrcset" => {
                for url in srcset_urls(value) {
                    self.check(ResourceKind::Image, url, at);
                }
            }
            "poster" => self.check(ResourceKind::Image, value, at),
            "data" if tag == "object" => self.check(ResourceKind::Embed, value, at),
            "background" if BACKGROUND_TAGS.contains(&tag) => {
                self.check(ResourceKind::Image, value, at);
            }
            "href" | "xlink:href" => match (link, tag) {
                (_, "a" | "area") => {}
                (Some(rel), _) => {
                    if let Some(kind) = rel.kind(value) {
                        self.check(kind, value, at);
                    }
                }
                (None, "script") => self.check(ResourceKind::Script, value, at),
                (None, "image" | "use" | "feimage") => self.check(ResourceKind::Image, value, at),
                (None, _) => self.check(ResourceKind::Other, value, at),
            },
            "style" => self.check_css(value, at),
            "srcdoc" if tag == "iframe" => self.check_nested_html(value, at),
            "content" if tag == "meta" && is_refresh(el) => {
                if let Some(url) = refresh_url(value) {
                    self.check(ResourceKind::Navigation, url, at);
                }
            }
            name if PRESENTATION_ATTRIBUTES.contains(&name) => self.check_css(value, at),
            _ => {}
        }
    }

    /// URL が外部なら記録する。`data:` URL なら中身の HTML・CSS・スクリプトを調べる。
    fn check(&mut self, kind: ResourceKind, url: &str, at: &Location) {
        if is_external(url) {
            self.push(kind, url.trim().to_owned(), at);
            return;
        }
        match data_content(url) {
            Some(DataContent::Html(html)) => self.check_nested_html(&html, at),
            Some(DataContent::Css(css)) => self.check_css_nested(&css, at),
            Some(DataContent::Script(script)) => {
                for (_, url) in module_urls(&script) {
                    self.push(ResourceKind::Script, url, at);
                }
            }
            None => {}
        }
    }

    /// CSS（`style` 属性、SVG の `fill="url(...)"`、`<style>` の中身）。
    fn check_css(&mut self, css: &str, at: &Location) {
        for found in css_urls(css) {
            let found_at = Location {
                offset: at.offset,
                extra_lines: at.extra_lines + css[..found.offset].matches('\n').count(),
                code: Code::Text(found.code),
            };
            self.check(found.kind, &found.url, &found_at);
        }
    }

    /// `data:` URL の中の CSS。行は属性の行にまとめる。
    fn check_css_nested(&mut self, css: &str, at: &Location) {
        if self.depth >= MAX_DEPTH {
            return;
        }
        for found in css_urls(css) {
            if is_external(&found.url) {
                self.push(found.kind, found.url, at);
            }
        }
    }

    /// `srcdoc` や `data:` URL の中の HTML。行は属性の行にまとめる。
    fn check_nested_html(&mut self, html: &str, at: &Location) {
        if self.depth >= MAX_DEPTH {
            return;
        }
        for found in scan_nested(html, self.depth + 1).findings {
            self.push(found.kind, found.url, at);
        }
    }

    fn push(&mut self, kind: ResourceKind, url: String, at: &Location) {
        let code = match &at.code {
            Code::Source(range) => snippet(self.html.get(range.clone()).unwrap_or_default()),
            Code::Text(text) => snippet(text),
        };
        self.findings.push(Finding {
            offset: at.offset,
            extra_lines: at.extra_lines,
            kind,
            code,
            url,
        });
    }

    fn start_text(&mut self, kind: TextKind) {
        self.text = Some(TextBuffer {
            kind,
            text: String::new(),
            segments: Vec::new(),
        });
    }

    fn push_text(&mut self, chunk: &str, source_offset: usize, text_type: TextType) {
        let Some(buffer) = self.text.as_mut() else {
            return;
        };
        if chunk.is_empty() {
            return;
        }
        buffer.segments.push((buffer.text.len(), source_offset));
        if matches!(text_type, TextType::Data | TextType::RCData) {
            buffer.text.push_str(&htmlize::unescape(chunk));
        } else {
            buffer.text.push_str(chunk);
        }
    }

    /// 貯めた `<style>` / `<script>` の中身を調べる。
    fn flush_text(&mut self) {
        let Some(buffer) = self.text.take() else {
            return;
        };
        let found: Vec<(usize, ResourceKind, String, String)> = match buffer.kind {
            TextKind::Style => css_urls(&buffer.text)
                .into_iter()
                .map(|u| (u.offset, u.kind, u.url, u.code))
                .collect(),
            TextKind::Script | TextKind::ImportMap => {
                let urls = if buffer.kind == TextKind::ImportMap {
                    import_map_urls(&buffer.text)
                } else {
                    module_urls(&buffer.text)
                };
                urls.into_iter()
                    .map(|(offset, url)| {
                        let code = line_at(&buffer.text, offset).to_owned();
                        (offset, ResourceKind::Script, url, code)
                    })
                    .collect()
            }
        };
        for (offset, kind, url, code) in found {
            let (source_offset, extra_lines) = buffer.source_position(offset);
            let at = Location {
                offset: source_offset,
                extra_lines,
                code: Code::Text(code),
            };
            self.check(kind, &url, &at);
        }
    }

    /// 最初の HTML の `<title>` の中身。SVG の `<title>` は文字の扱い（RCDATA ではない）で区別する。
    fn push_title(&mut self, chunk: &str, text_type: TextType, is_last: bool) {
        if text_type != TextType::RCData {
            return;
        }
        let title = match std::mem::replace(&mut self.title, TitleState::Waiting) {
            TitleState::Waiting => chunk.to_owned(),
            TitleState::Reading(mut title) => {
                title.push_str(chunk);
                title
            }
            done @ TitleState::Done(_) => {
                self.title = done;
                return;
            }
        };
        self.title = if is_last {
            TitleState::Done(title)
        } else {
            TitleState::Reading(title)
        };
    }
}

/// 見つけた場所。行番号は `offset` の行から `extra_lines` 行下。
struct Location {
    offset: usize,
    extra_lines: usize,
    code: Code,
}

/// 該当するコードとして見せるもの。
enum Code {
    /// 元の HTML の範囲（属性を持つタグ）。
    Source(Range<usize>),
    /// CSS やスクリプトの一部。
    Text(String),
}

/// `src` 属性で読み込む要素と、その種類。
fn src_kind(tag: &str) -> Option<ResourceKind> {
    Some(match tag {
        "script" => ResourceKind::Script,
        "img" | "image" | "input" => ResourceKind::Image,
        "iframe" | "frame" | "embed" | "portal" => ResourceKind::Embed,
        "video" | "audio" | "source" | "track" => ResourceKind::Media,
        _ => return None,
    })
}

/// 古い `background` 属性で背景画像を読み込む要素。
const BACKGROUND_TAGS: [&str; 8] = ["body", "table", "thead", "tbody", "tfoot", "tr", "td", "th"];

/// SVG の、値に `url()` を書ける属性。
const PRESENTATION_ATTRIBUTES: [&str; 9] = [
    "fill",
    "stroke",
    "filter",
    "mask",
    "clip-path",
    "marker-start",
    "marker-mid",
    "marker-end",
    "cursor",
];

/// 読み込みをしない `rel`（ページどうしの関係を示すだけのもの）。
const NAVIGATION_RELS: [&str; 17] = [
    "alternate",
    "author",
    "bookmark",
    "canonical",
    "external",
    "help",
    "license",
    "me",
    "next",
    "nofollow",
    "noopener",
    "noreferrer",
    "opener",
    "prev",
    "privacy-policy",
    "search",
    "terms-of-service",
];

/// `<link>` の `rel` と `as`。
struct LinkRel {
    rels: Vec<String>,
    as_: String,
}

impl LinkRel {
    fn of(el: &Element<'_, '_>) -> Self {
        let rel = el
            .get_attribute("rel")
            .map(|r| htmlize::unescape_attribute(&r).to_ascii_lowercase())
            .unwrap_or_default();
        Self {
            rels: rel.split_ascii_whitespace().map(str::to_owned).collect(),
            as_: el
                .get_attribute("as")
                .map(|a| htmlize::unescape_attribute(&a).trim().to_ascii_lowercase())
                .unwrap_or_default(),
        }
    }

    /// `href` の読み込みの種類。読み込みをしない `rel` だけなら `None`。
    fn kind(&self, href: &str) -> Option<ResourceKind> {
        let has = |rel: &str| self.rels.iter().any(|r| r == rel);
        if self
            .rels
            .iter()
            .all(|r| NAVIGATION_RELS.contains(&r.as_str()))
        {
            return None;
        }
        let kind = if has("stylesheet") {
            if is_font_service(href) {
                ResourceKind::Font
            } else {
                ResourceKind::Stylesheet
            }
        } else if has("icon") || has("apple-touch-icon") || has("mask-icon") {
            ResourceKind::Image
        } else if has("modulepreload") {
            ResourceKind::Script
        } else if has("preload") || has("prefetch") {
            match self.as_.as_str() {
                "script" => ResourceKind::Script,
                "style" => ResourceKind::Stylesheet,
                "font" => ResourceKind::Font,
                "image" => ResourceKind::Image,
                "audio" | "video" | "track" => ResourceKind::Media,
                _ => ResourceKind::Other,
            }
        } else if (has("preconnect") || has("dns-prefetch")) && is_font_service(href) {
            ResourceKind::Font
        } else {
            ResourceKind::Other
        };
        Some(kind)
    }
}

/// Web フォントの配信サービス（Google Fonts など）か。
fn is_font_service(href: &str) -> bool {
    let href = href.to_ascii_lowercase();
    [
        "fonts.googleapis.com",
        "fonts.gstatic.com",
        "use.typekit.net",
        "fonts.bunny.net",
    ]
    .iter()
    .any(|host| href.contains(host))
}

fn is_refresh(el: &Element<'_, '_>) -> bool {
    el.get_attribute("http-equiv").is_some_and(|v| {
        htmlize::unescape_attribute(&v)
            .trim()
            .eq_ignore_ascii_case("refresh")
    })
}

/// `offset` を含む1行。
fn line_at(html: &str, offset: usize) -> &str {
    // 文字参照を戻した分だけ位置がずれることがあるので、文字の境界に合わせる
    let offset = html.floor_char_boundary(offset);
    let start = html[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = html[offset..].find('\n').map_or(html.len(), |i| offset + i);
    &html[start..end]
}

/// 該当するコードとして見せる文字列。空白を詰め、長ければ切り詰める。
fn snippet(code: &str) -> String {
    const MAX_CHARS: usize = 120;
    let collapsed = code.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_CHARS {
        return collapsed;
    }
    let mut truncated: String = collapsed.chars().take(MAX_CHARS).collect();
    truncated.push('…');
    truncated
}

/// ブラウザーの `document.title` と同じく、前後の空白を除いて途中の空白を1つに詰める。
fn normalize_title(raw: &str) -> Option<String> {
    let decoded = htmlize::unescape(raw);
    let title = decoded
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!title.is_empty()).then_some(title)
}
