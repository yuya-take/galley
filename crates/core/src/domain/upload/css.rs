//! CSS の中の外部リソース（`url()`、`@import`、`image-set()`）。
//!
//! 文字列の置換ではなく CSS の字句解析（cssparser）で読むので、エスケープ（`\75 rl(`）や
//! コメントを挟んだ書き方でもブラウザーと同じ解釈になる。

use cssparser::{ParseError, Parser, SourcePosition, Token};

use super::resource::ResourceKind;

/// CSS の中で見つかった読み込み。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CssUrl {
    /// CSS の先頭からのバイト位置。
    pub offset: usize,
    pub kind: ResourceKind,
    pub code: String,
    pub url: String,
}

/// フォントの拡張子。`@font-face` の外でも、これらはフォントとして扱う。
const FONT_EXTENSIONS: [&str; 5] = [".woff2", ".woff", ".ttf", ".otf", ".eot"];

/// CSS（`<style>` の中身、または `style` 属性の値）から、読み込む URL をすべて取り出す。
/// 外部かどうかはここでは判定しない。
pub(super) fn css_urls(css: &str) -> Vec<CssUrl> {
    let mut parser = Parser::new(css);
    let mut urls = Vec::new();
    walk(&mut parser, Context::default(), &mut urls);
    urls
}

/// 入れ子をたどる深さの上限。深すぎる入れ子でスタックを使い切らないよう、これより深い所は調べない
/// （最終的な防御は表示側の CSP）。
const MAX_DEPTH: u8 = 32;

#[derive(Debug, Clone, Copy, Default)]
struct Context {
    in_font_face: bool,
    /// `image-set()` の中では、文字列もそのまま URL になる。
    in_image_set: bool,
    depth: u8,
}

impl Context {
    fn nested(self, in_font_face: bool, in_image_set: bool) -> Self {
        Self {
            in_font_face,
            in_image_set,
            depth: self.depth + 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtRule {
    Import,
    FontFace,
}

type Nested = Result<(), ParseError<()>>;

fn walk(parser: &mut Parser<'_>, context: Context, urls: &mut Vec<CssUrl>) {
    if context.depth >= MAX_DEPTH {
        return;
    }
    let mut at_rule = None;
    loop {
        let start = parser.position();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break;
        };
        match token.clone() {
            Token::UnquotedUrl(url) => {
                push(parser, start, kind(context, at_rule, &url), &url, urls);
            }
            Token::QuotedString(url) if at_rule == Some(AtRule::Import) || context.in_image_set => {
                push(parser, start, kind(context, at_rule, &url), &url, urls);
            }
            Token::Function(name) => {
                let name = name.to_ascii_lowercase();
                if name == "url" || name == "src" {
                    let mut found = None;
                    let _ = parser.parse_nested_block(|inner| -> Nested {
                        found = first_string(inner);
                        Ok(())
                    });
                    if let Some(url) = found {
                        push(parser, start, kind(context, at_rule, &url), &url, urls);
                    }
                } else {
                    let in_image_set = name == "image-set" || name == "-webkit-image-set";
                    let inner_context = context.nested(context.in_font_face, in_image_set);
                    let _ = parser.parse_nested_block(|inner| -> Nested {
                        walk(inner, inner_context, urls);
                        Ok(())
                    });
                }
            }
            Token::AtKeyword(name) => {
                at_rule = if name.eq_ignore_ascii_case("import") {
                    Some(AtRule::Import)
                } else if name.eq_ignore_ascii_case("font-face") {
                    Some(AtRule::FontFace)
                } else {
                    None
                };
            }
            Token::CurlyBracketBlock => {
                let in_font_face = context.in_font_face || at_rule == Some(AtRule::FontFace);
                let inner_context = context.nested(in_font_face, false);
                let _ = parser.parse_nested_block(|inner| -> Nested {
                    walk(inner, inner_context, urls);
                    Ok(())
                });
                at_rule = None;
            }
            Token::ParenthesisBlock | Token::SquareBracketBlock => {
                let inner_context = context.nested(context.in_font_face, false);
                let _ = parser.parse_nested_block(|inner| -> Nested {
                    walk(inner, inner_context, urls);
                    Ok(())
                });
            }
            Token::Semicolon => at_rule = None,
            _ => {}
        }
    }
}

/// `url( "..." )` の中の最初の文字列。
fn first_string(parser: &mut Parser<'_>) -> Option<String> {
    loop {
        match parser.next_including_whitespace_and_comments().ok()? {
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::QuotedString(url) => return Some(url.to_string()),
            _ => return None,
        }
    }
}

fn kind(context: Context, at_rule: Option<AtRule>, url: &str) -> ResourceKind {
    if at_rule == Some(AtRule::Import) {
        return ResourceKind::Stylesheet;
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .to_ascii_lowercase();
    if context.in_font_face || FONT_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        ResourceKind::Font
    } else {
        ResourceKind::Image
    }
}

fn push(
    parser: &Parser<'_>,
    start: SourcePosition,
    kind: ResourceKind,
    url: &str,
    urls: &mut Vec<CssUrl>,
) {
    urls.push(CssUrl {
        offset: start.byte_index(),
        kind,
        code: parser.slice_from(start).to_owned(),
        url: url.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(css: &str) -> Vec<(ResourceKind, String)> {
        css_urls(css).into_iter().map(|u| (u.kind, u.url)).collect()
    }

    #[test]
    fn finds_url_tokens_and_functions() {
        assert_eq!(
            found("a{background:url(a.png)} b{background:URL( 'b.png' )}"),
            vec![
                (ResourceKind::Image, "a.png".to_owned()),
                (ResourceKind::Image, "b.png".to_owned()),
            ]
        );
    }

    #[test]
    fn finds_imports_in_both_forms() {
        assert_eq!(
            found("@import 'a.css'; @IMPORT url(b.css) screen; a{}"),
            vec![
                (ResourceKind::Stylesheet, "a.css".to_owned()),
                (ResourceKind::Stylesheet, "b.css".to_owned()),
            ]
        );
    }

    #[test]
    fn font_face_sources_are_fonts() {
        assert_eq!(
            found("@font-face{font-family:x;src:url(x.bin) format('woff2')} a{b:url(y.woff2)}"),
            vec![
                (ResourceKind::Font, "x.bin".to_owned()),
                (ResourceKind::Font, "y.woff2".to_owned()),
            ]
        );
    }

    #[test]
    fn escapes_and_comments_do_not_hide_urls() {
        assert_eq!(found(r"a{b:\75 rl(x.png)}")[0].1, "x.png");
        assert_eq!(found(r"a{b:u\72l(x.png)}")[0].1, "x.png");
        assert_eq!(found(r"@\69mport 'x.css';")[0].1, "x.css");
        assert_eq!(found("a{b:url(/**/x.png)}")[0].1, "/**/x.png");
        assert_eq!(
            found(r"a{b:url(h\74tps://e.com/x.png)}")[0].1,
            "https://e.com/x.png"
        );
    }

    #[test]
    fn image_set_strings_are_urls() {
        assert_eq!(
            found("a{b:image-set('a.png' 1x, url(b.png) 2x)} c{d:-webkit-image-set(\"c.png\" 1x)}"),
            vec![
                (ResourceKind::Image, "a.png".to_owned()),
                (ResourceKind::Image, "b.png".to_owned()),
                (ResourceKind::Image, "c.png".to_owned()),
            ]
        );
    }

    #[test]
    fn nested_rules_are_walked() {
        assert_eq!(
            found("@media screen{@supports (x:y){a{b:url(n.png)}}}")[0].1,
            "n.png"
        );
    }

    #[test]
    fn deep_nesting_stops_without_overflowing() {
        let depth = 200_000;
        let css = format!(
            "a{{b:{}url(x.png){}}}",
            "f(".repeat(depth),
            ")".repeat(depth)
        );
        assert!(found(&css).is_empty());
        let css = format!("a{{b:{}}}", "(".repeat(depth));
        assert!(found(&css).is_empty());
        let shallow = format!("a{{b:{}url(x.png){}}}", "f(".repeat(10), ")".repeat(10));
        assert_eq!(found(&shallow).len(), 1);
    }

    #[test]
    fn plain_strings_are_not_urls() {
        assert!(found("a::before{content:'https://e.com/x.png'}").is_empty());
    }

    #[test]
    fn reports_offset_and_code() {
        let urls = css_urls("a{}\nb{c:url( \"x.png\" )}");
        assert_eq!(urls[0].offset, 8);
        assert_eq!(urls[0].code, "url( \"x.png\" )");
    }
}
