//! 属性値の URL の判定と取り出し。

use base64::Engine as _;

/// 外部から読み込まない（HTML の中で完結する）スキーム。
const INLINE_SCHEMES: [&str; 4] = ["data", "blob", "about", "javascript"];

/// ブラウザーと同じく、前後の空白・制御文字と、途中のタブ・改行を取り除く。
fn normalize(url: &str) -> String {
    url.trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

/// 小文字にしたスキーム。スキームが無ければ（相対 URL なら）`None`。
fn scheme(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let is_valid = starts_with_letter
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    is_valid.then(|| scheme.to_ascii_lowercase())
}

/// 外部から読み込む URL か。`data:`・`blob:` と、文書内の参照（`#id`）、空の値は外部ではない。
///
/// 相対 URL（`logo.png`、`/app.js`、`//cdn.example.com/x.js`）は資料配信のオリジンから
/// 読み込もうとするので、外部として扱う。
pub(super) fn is_external(url: &str) -> bool {
    let url = normalize(url);
    if url.is_empty() || url.starts_with('#') {
        return false;
    }
    match scheme(&url) {
        Some(scheme) => !INLINE_SCHEMES.contains(&scheme.as_str()),
        None => true,
    }
}

/// スクリプトから読み込むモジュールの指定のうち、URL として解決されるもの
/// （`https:`、`//`、`/`、`./`、`../` で始まる）か。`three` のような名前だけの指定は URL ではない。
pub(super) fn is_module_url(specifier: &str) -> bool {
    let specifier = normalize(specifier).to_ascii_lowercase();
    ["http:", "https:", "/", "./", "../"]
        .iter()
        .any(|prefix| specifier.starts_with(prefix))
}

/// `srcset` / `imagesrcset` の各候補の URL。HTML の仕様の手順で分ける（`data:` URL のカンマで切らない）。
pub(super) fn srcset_urls(value: &str) -> Vec<&str> {
    let mut urls = Vec::new();
    let mut rest = value;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let url = &rest[..end];
        rest = &rest[end..];
        if url.ends_with(',') {
            urls.push(url.trim_end_matches(','));
            continue;
        }
        urls.push(url);
        // 次のカンマ（括弧の外）までは幅や密度の指定なので読み飛ばす
        let mut depth = 0usize;
        let mut next = rest.len();
        for (i, c) in rest.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {
                    next = i + 1;
                    break;
                }
                _ => {}
            }
        }
        rest = &rest[next..];
    }
    urls
}

/// `<meta http-equiv="refresh" content="5; url=...">` の移動先。移動先が無ければ（再読み込みなら）`None`。
pub(super) fn refresh_url(content: &str) -> Option<&str> {
    let rest = content.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let rest = rest.strip_prefix([';', ',']).unwrap_or(rest);
    let mut rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
    if rest.get(..3).is_some_and(|s| s.eq_ignore_ascii_case("url")) {
        let after = rest[3..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(after) = after.strip_prefix('=') {
            rest = after.trim_start_matches(|c: char| c.is_ascii_whitespace());
        }
    }
    let url = match rest.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let inner = &rest[1..];
            inner.split(quote).next().unwrap_or(inner)
        }
        _ => rest,
    };
    let url = url.trim_end_matches(|c: char| c.is_ascii_whitespace());
    (!url.is_empty()).then_some(url)
}

/// `data:` URL の中身。中を検査する種類（HTML、SVG、CSS、JavaScript）だけ返す。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DataContent {
    Html(String),
    Css(String),
    Script(String),
}

pub(super) fn data_content(url: &str) -> Option<DataContent> {
    let url = normalize(url);
    if scheme(&url).as_deref() != Some("data") {
        return None;
    }
    let (meta, body) = url["data:".len()..].split_once(',')?;
    let mut params = meta.split(';').map(str::trim);
    let mime = params.next().unwrap_or_default().to_ascii_lowercase();
    let is_base64 = params.any(|p| p.eq_ignore_ascii_case("base64"));
    let bytes = if is_base64 {
        let compact: String = percent_decode(body)
            .into_iter()
            .map(char::from)
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        base64::engine::general_purpose::STANDARD
            .decode(compact.trim_end_matches('='))
            .or_else(|_| {
                base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(compact.trim_end_matches('='))
            })
            .ok()?
    } else {
        percent_decode(body)
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    match mime.as_str() {
        "text/html" | "application/xhtml+xml" | "image/svg+xml" => Some(DataContent::Html(text)),
        "text/css" => Some(DataContent::Css(text)),
        "text/javascript"
        | "application/javascript"
        | "application/ecmascript"
        | "text/ecmascript" => Some(DataContent::Script(text)),
        _ => None,
    }
}

fn percent_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                i += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    decoded
}

/// スクリプトの URL からライブラリ名を推測する（AI への依頼文に使う）。分からなければ `None`。
pub(super) fn library_name(url: &str) -> Option<String> {
    let url = normalize(url);
    let lower = url.to_ascii_lowercase();
    let rest = ["https://", "http://", "//"]
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix).map(|_| &url[prefix.len()..]))?;
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let name = match (host.to_ascii_lowercase().as_str(), segments.as_slice()) {
        ("cdn.jsdelivr.net", ["npm", rest @ ..]) => npm_name(rest),
        ("cdn.jsdelivr.net", ["gh", _, repo, ..]) => Some(strip_version(repo).to_owned()),
        ("cdnjs.cloudflare.com", ["ajax", "libs", name, ..]) => Some((*name).to_owned()),
        ("unpkg.com" | "esm.sh" | "esm.run" | "cdn.skypack.dev", rest) => npm_name(rest),
        (_, [.., file]) => file_name(file),
        _ => None,
    }?;
    (!name.is_empty()).then_some(name)
}

fn npm_name(segments: &[&str]) -> Option<String> {
    match segments {
        [scope, name, ..] if scope.starts_with('@') => {
            Some(format!("{scope}/{}", strip_version(name)))
        }
        [name, ..] => Some(strip_version(name).to_owned()),
        [] => None,
    }
}

/// `chart.js@4.4.0` → `chart.js`、`jquery-3.7.1` → `jquery`。
fn strip_version(name: &str) -> &str {
    let name = name.split('@').next().unwrap_or(name);
    match name.rfind('-') {
        Some(i) if name[i + 1..].starts_with(|c: char| c.is_ascii_digit()) => &name[..i],
        _ => name,
    }
}

/// `chart.umd.min.js` → `chart`、`jquery-3.7.1.min.js` → `jquery`。
fn file_name(file: &str) -> Option<String> {
    let mut stem = file
        .strip_suffix(".js")
        .or_else(|| file.strip_suffix(".mjs"))?;
    const SUFFIXES: [&str; 8] = [
        ".min",
        ".umd",
        ".esm",
        ".prod",
        ".production",
        ".global",
        ".bundle",
        ".browser",
    ];
    while let Some(stripped) = SUFFIXES.iter().find_map(|s| stem.strip_suffix(s)) {
        stem = stripped;
    }
    // 末尾の `.3.7.1` のような版番号を除く
    while let Some((head, tail)) = stem.rsplit_once('.') {
        if tail.chars().all(|c| c.is_ascii_digit()) && !tail.is_empty() {
            stem = head;
        } else {
            break;
        }
    }
    let stem = strip_version(stem);
    stem.chars()
        .any(|c| c.is_ascii_alphabetic())
        .then(|| stem.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_urls_are_not_external() {
        for url in [
            "",
            "   ",
            "#chart",
            "data:image/png;base64,AAAA",
            " DATA:image/png,x",
            "blob:https://example.com/1",
            "about:blank",
            "javascript:void(0)",
            "d\na\tta:image/png,x",
        ] {
            assert!(!is_external(url), "{url:?}");
        }
    }

    #[test]
    fn remote_and_relative_urls_are_external() {
        for url in [
            "https://cdn.example.com/a.js",
            "HTTP://example.com",
            "//cdn.example.com/a.js",
            "logo.png",
            "/assets/app.css",
            "./a.js",
            "?page=2",
            "file:///etc/passwd",
            "ftp://example.com/a",
            "\u{0}\u{1} https://example.com",
            "ht\ttps://example.com",
            "data",
        ] {
            assert!(is_external(url), "{url:?}");
        }
    }

    #[test]
    fn module_urls_exclude_bare_specifiers() {
        assert!(is_module_url("https://esm.sh/three"));
        assert!(is_module_url("//esm.sh/three"));
        assert!(is_module_url("./chart.js"));
        assert!(is_module_url("../lib.js"));
        assert!(!is_module_url("three"));
        assert!(!is_module_url("data:text/javascript,export default 1"));
    }

    #[test]
    fn srcset_keeps_commas_inside_data_urls() {
        assert_eq!(srcset_urls("a.png 1x, b.png 2x"), vec!["a.png", "b.png"]);
        assert_eq!(
            srcset_urls("data:image/png;base64,AAA= 1x,https://e.com/b.png 2x"),
            vec!["data:image/png;base64,AAA=", "https://e.com/b.png"]
        );
        assert_eq!(srcset_urls("a.png,b.png"), vec!["a.png,b.png"]);
        assert_eq!(srcset_urls("a.png, b.png"), vec!["a.png", "b.png"]);
        assert_eq!(
            srcset_urls("a.png (max-width: 1px, 2x), b.png"),
            vec!["a.png", "b.png"]
        );
        assert!(srcset_urls("  ,  ").is_empty());
    }

    #[test]
    fn refresh_url_follows_declarative_refresh() {
        assert_eq!(refresh_url("0; url=https://e.com"), Some("https://e.com"));
        assert_eq!(
            refresh_url("5;URL = 'https://e.com'"),
            Some("https://e.com")
        );
        assert_eq!(refresh_url("0,https://e.com"), Some("https://e.com"));
        assert_eq!(refresh_url("0 https://e.com"), Some("https://e.com"));
        assert_eq!(refresh_url("0;url=\"a.html\" "), Some("a.html"));
        assert_eq!(refresh_url("30"), None);
        assert_eq!(refresh_url("30; "), None);
        assert_eq!(refresh_url("0;あいう"), Some("あいう"));
    }

    #[test]
    fn data_content_decodes_percent_and_base64() {
        assert_eq!(
            data_content("data:text/html,%3Cimg%20src=x%3E"),
            Some(DataContent::Html("<img src=x>".to_owned()))
        );
        assert_eq!(
            data_content("data:text/css;base64,QGltcG9ydCAneCc7"),
            Some(DataContent::Css("@import 'x';".to_owned()))
        );
        assert_eq!(
            data_content("data:image/svg+xml;charset=utf-8;base64,PHN2Zz4="),
            Some(DataContent::Html("<svg>".to_owned()))
        );
        assert_eq!(data_content("data:image/png;base64,AAAA"), None);
        assert_eq!(data_content("https://e.com"), None);
    }

    #[test]
    fn library_name_reads_cdn_paths() {
        let cases = [
            (
                "https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js",
                "chart.js",
            ),
            (
                "https://cdn.jsdelivr.net/npm/@mermaid-js/mermaid@10",
                "@mermaid-js/mermaid",
            ),
            (
                "https://cdnjs.cloudflare.com/ajax/libs/Chart.js/4.4.0/chart.umd.js",
                "Chart.js",
            ),
            (
                "https://unpkg.com/react@18/umd/react.production.min.js",
                "react",
            ),
            ("https://esm.sh/three@0.160.0", "three"),
            ("https://code.jquery.com/jquery-3.7.1.min.js", "jquery"),
            ("//cdn.example.com/libs/lodash.min.js", "lodash"),
        ];
        for (url, name) in cases {
            assert_eq!(library_name(url).as_deref(), Some(name), "{url}");
        }
        assert_eq!(library_name("app.js"), None);
        assert_eq!(library_name("https://cdn.tailwindcss.com"), None);
        assert_eq!(library_name("https://example.com/1.2.3.js"), None);
    }
}
