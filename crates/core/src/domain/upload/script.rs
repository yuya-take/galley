//! インラインのスクリプトの中の、モジュールの読み込み（`import ... from "https://..."`、`import("...")`）。
//!
//! 字句解析はせず、`import` / `from` の直後の文字列だけを見る。スクリプトが実行時に組み立てる
//! URL は見つけられないが、そこは表示側の CSP が止める。

use super::url::is_module_url;

/// スクリプトの中で見つかったモジュールの URL（スクリプトの先頭からのバイト位置と URL）。
pub(super) fn module_urls(script: &str) -> Vec<(usize, String)> {
    let mut urls = Vec::new();
    for keyword in ["import", "from"] {
        let mut search_from = 0;
        while let Some(found) = script[search_from..].find(keyword) {
            let start = search_from + found;
            search_from = start + keyword.len();
            let is_word_start = script[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_identifier(c));
            if !is_word_start {
                continue;
            }
            if let Some(specifier) = string_after(&script[search_from..])
                && is_module_url(specifier)
            {
                urls.push((start, specifier.to_owned()));
            }
        }
    }
    urls.sort_by_key(|(offset, _)| *offset);
    urls
}

/// インポートマップ（`<script type="importmap">`）の値のうち、URL として解決されるもの。
pub(super) fn import_map_urls(json: &str) -> Vec<(usize, String)> {
    let mut urls = Vec::new();
    let mut rest = json;
    let mut offset = 0;
    while let Some(open) = rest.find('"') {
        let body = &rest[open + 1..];
        let Some(close) = body.find('"') else {
            break;
        };
        let value = &body[..close];
        if is_module_url(value) {
            urls.push((offset + open, value.to_owned()));
        }
        let consumed = open + 1 + close + 1;
        offset += consumed;
        rest = &rest[consumed..];
    }
    urls
}

fn is_identifier(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '.'
}

/// 空白と `(` を読み飛ばした先の、引用符で囲まれた文字列。
fn string_after(rest: &str) -> Option<&str> {
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '(');
    let quote = rest
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let body = &rest[1..];
    let end = body.find([quote, '\n'])?;
    Some(&body[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(script: &str) -> Vec<String> {
        module_urls(script)
            .into_iter()
            .map(|(_, url)| url)
            .collect()
    }

    #[test]
    fn finds_static_and_dynamic_imports() {
        let script = r#"
            import * as THREE from "https://esm.sh/three";
            import 'https://cdn.example.com/side-effect.js';
            export { x } from './local.js';
            const m = await import(`https://esm.sh/d3`);
            const n = await import ( "//cdn.example.com/n.js" );
        "#;
        assert_eq!(
            urls(script),
            vec![
                "https://esm.sh/three",
                "https://cdn.example.com/side-effect.js",
                "./local.js",
                "https://esm.sh/d3",
                "//cdn.example.com/n.js",
            ]
        );
    }

    #[test]
    fn ignores_bare_specifiers_and_other_words() {
        assert!(urls("import x from 'three'; const important = 'https://e.com';").is_empty());
        assert!(urls("obj.import('https://e.com/x.js')").is_empty());
        assert!(urls("const from_ = 1; transform('https://e.com')").is_empty());
    }

    #[test]
    fn import_map_values_are_checked() {
        let json =
            r#"{"imports": {"three": "https://esm.sh/three", "app": "data:text/javascript,1"}}"#;
        let found: Vec<String> = import_map_urls(json).into_iter().map(|(_, u)| u).collect();
        assert_eq!(found, vec!["https://esm.sh/three"]);
    }
}
