//! アップロード時の検査。すり抜けの書き方を含めて、外部リソースを見逃さないことを確かめる。

use galley_core::domain::upload::{
    ExternalResources, HtmlDocument, MAX_HTML_BYTES, REJECTION_REASON, ResourceKind, UploadError,
};

use ResourceKind::{Embed, Font, Image, Media, Navigation, Other, Script, Stylesheet};

fn parse(html: &str) -> Result<HtmlDocument, UploadError> {
    HtmlDocument::parse(html.as_bytes().to_vec())
}

#[track_caller]
fn rejected(html: &str) -> ExternalResources {
    match parse(html) {
        Err(UploadError::ExternalResources(resources)) => resources,
        other => panic!("拒否されるはずが {other:?}\n{html}"),
    }
}

/// 見つかった種類（行の順）。
#[track_caller]
fn kinds(html: &str) -> Vec<ResourceKind> {
    rejected(html).resources().iter().map(|r| r.kind).collect()
}

#[track_caller]
fn assert_accepted(html: &str) {
    if let Err(err) = parse(html) {
        panic!("受け付けるはずが {err:?}\n{html}");
    }
}

// ---- 受け付けるもの ----

#[test]
fn accepts_self_contained_document() {
    assert_accepted(
        r##"<!doctype html>
<html lang="ja">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width">
  <meta property="og:image" content="https://example.com/og.png">
  <link rel="canonical" href="https://example.com/doc">
  <link rel="alternate" type="application/rss+xml" href="/feed.xml">
  <title>売上レポート</title>
  <style>
    body { background: url(data:image/png;base64,AAAA); }
    .chart { fill: url(#gradient); }
    a::after { content: "https://example.com"; }
  </style>
</head>
<body>
  <a href="https://example.com" ping="https://example.com/ping">参考リンク</a>
  <area href="https://example.com">
  <form action="https://example.com/submit"><button formaction="/x">送信</button></form>
  <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E" alt="">
  <img srcset="data:image/png;base64,AAA= 1x, data:image/png;base64,BBB= 2x">
  <video src="blob:https://example.com/1"></video>
  <iframe src="about:blank"></iframe>
  <iframe srcdoc="<p style='color:red'>埋め込み</p>"></iframe>
  <svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
    <defs><linearGradient id="gradient"/></defs>
    <use href="#icon"/><use xlink:href="#icon"/>
    <rect fill="url(#gradient)" style="stroke:url(#gradient)"/>
    <a href="https://example.com"><text>リンク</text></a>
  </svg>
  <script>
    fetch("https://api.example.com");
    const doc = "import x from 'y'";
  </script>
  <script type="module">import { a } from "three"; export default a;</script>
</body>
</html>"##,
    );
}

#[test]
fn accepts_scripts_that_only_mention_urls() {
    assert_accepted(r#"<script>const important = "https://example.com/x.js";</script>"#);
}

#[test]
fn ignores_noscript_contents_because_scripts_run_in_viewer() {
    assert_accepted(r#"<noscript><img src="https://example.com/pixel.gif"></noscript>"#);
}

// ---- サイズ・文字コード ----

#[test]
fn rejects_empty_file() {
    assert_eq!(parse(""), Err(UploadError::Empty));
    assert_eq!(parse(" \n\t"), Err(UploadError::Empty));
}

#[test]
fn rejects_non_utf8() {
    let shift_jis = vec![0x82, 0xa0, 0x82, 0xa2];
    assert_eq!(HtmlDocument::parse(shift_jis), Err(UploadError::NotUtf8));
}

#[test]
fn rejects_too_large_file() {
    let html = vec![b' '; MAX_HTML_BYTES + 1];
    assert_eq!(
        HtmlDocument::parse(html),
        Err(UploadError::TooLarge {
            size: MAX_HTML_BYTES + 1,
            max: MAX_HTML_BYTES
        })
    );
    assert!(
        HtmlDocument::parse(format!("<p>{}", " ".repeat(MAX_HTML_BYTES - 3)).into_bytes()).is_ok()
    );
}

// ---- 要素の属性 ----

#[test]
fn detects_element_sources() {
    let html = r#"<script src="https://cdn.example.com/chart.js"></script>
<img src="logo.png">
<img srcset="data:image/png;base64,AAA= 1x, https://example.com/b.png 2x">
<picture><source srcset="a.webp"><img src="data:,"></picture>
<video src="movie.mp4" poster="poster.jpg"><track src="subs.vtt"></video>
<audio><source src="sound.mp3"></audio>
<iframe src="https://example.com"></iframe>
<embed src="x.pdf"><object data="x.swf"></object><frame src="f.html">
<input type="image" src="button.png">
<table background="bg.png"><tr><td background="cell.png"></td></tr></table>
<body background="body.png">"#;
    assert_eq!(
        kinds(html),
        vec![
            Script, Image, Image, Image, Media, Image, Media, Media, Embed, Embed, Embed, Embed,
            Image, Image, Image, Image
        ]
    );
}

#[test]
fn detects_links_by_rel() {
    let html = r#"<link rel="stylesheet" href="style.css">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Noto+Sans+JP">
<link rel="preload" href="font.woff2" as="font" crossorigin>
<link rel="preload" href="hero.png" as="image">
<link rel="modulepreload" href="app.js">
<link rel="icon" href="favicon.ico">
<link rel="preconnect" href="https://fonts.gstatic.com">
<link rel="manifest" href="app.webmanifest">
<link rel="alternate stylesheet" href="dark.css">
<link rel="preload" as="image" imagesrcset="a.png 1x, b.png 2x">"#;
    assert_eq!(
        kinds(html),
        vec![
            Stylesheet, Font, Font, Image, Script, Image, Font, Other, Stylesheet, Image, Image
        ]
    );
}

#[test]
fn detects_base_and_meta_refresh() {
    let html = r#"<base href="https://example.com/">
<meta http-equiv="Refresh" content="0; URL='https://example.com/next'">
<meta http-equiv="refresh" content="30">"#;
    let resources = rejected(html);
    assert_eq!(
        resources
            .resources()
            .iter()
            .map(|r| (r.line, r.kind))
            .collect::<Vec<_>>(),
        vec![(1, Other), (2, Navigation)]
    );
    assert_eq!(resources.resources()[1].url, "https://example.com/next");
}

#[test]
fn detects_svg_references() {
    let html = r#"<svg>
<image href="photo.png"/>
<image xlink:href="https://example.com/photo.png"/>
<use href="sprite.svg#icon"/>
<feImage href="texture.png"/>
<script href="https://example.com/svg.js"/>
<rect fill="url(https://example.com/p.svg#pattern)"/>
<textPath href="other.svg#path"/>
</svg>"#;
    assert_eq!(
        kinds(html),
        vec![Image, Image, Image, Image, Script, Image, Other]
    );
}

#[test]
fn html_image_tag_is_treated_as_img() {
    assert_eq!(
        kinds(r#"<image src="https://example.com/a.png">"#),
        vec![Image]
    );
}

// ---- すり抜けの書き方 ----

#[test]
fn character_references_in_attributes_do_not_hide_urls() {
    for html in [
        r#"<img src="h&#116;tps://example.com/a.png">"#,
        r#"<img src="&#x68;ttps://example.com/a.png">"#,
        r#"<img src="https&colon;//example.com/a.png">"#,
        r#"<img src="&#104&#116&#116&#112&#115://example.com/a.png">"#,
        r#"<img src="data&#58image/png,x" srcset="&#104;ttps://example.com/b.png">"#,
    ] {
        assert_eq!(kinds(html), vec![Image], "{html}");
    }
}

#[test]
fn case_quotes_and_whitespace_do_not_hide_urls() {
    for html in [
        r#"<IMG SRC="https://example.com/a.png">"#,
        "<img src=https://example.com/a.png>",
        "<img src='https://example.com/a.png'>",
        "<img\nsrc\n=\n\"https://example.com/a.png\">",
        "<img src=\"  https://example.com/a.png  \">",
        "<img src=\"ht\ntps://example.com/a.png\">",
        "<img src=\"\u{1}https://example.com/a.png\">",
        "<img/src=\"https://example.com/a.png\">",
        r#"<img src="HTTPS://EXAMPLE.COM/A.PNG">"#,
        r#"<img src="//example.com/a.png">"#,
    ] {
        assert_eq!(kinds(html), vec![Image], "{html:?}");
    }
}

#[test]
fn inline_scheme_look_alikes_are_still_external() {
    for html in [
        r#"<img src="datax:image/png,x">"#,
        r#"<img src="data">"#,
        r#"<img src="./data:image/png,x">"#,
        r#"<img src="file:///etc/passwd">"#,
    ] {
        assert_eq!(kinds(html), vec![Image], "{html}");
    }
}

#[test]
fn duplicate_attributes_are_all_checked() {
    assert_eq!(
        kinds(r#"<img src="data:," src="https://example.com/a.png">"#),
        vec![Image]
    );
}

#[test]
fn template_contents_are_checked() {
    assert_eq!(
        kinds(r#"<template><img src="https://example.com/a.png"></template>"#),
        vec![Image]
    );
}

// ---- CSS ----

#[test]
fn detects_css_in_style_elements_and_attributes() {
    let html = r#"<style>
@import "reset.css";
@import url(https://example.com/theme.css);
@font-face { font-family: X; src: url(x.woff2) format("woff2"); }
body { background: url('bg.png'); }
.a { background-image: image-set("a.png" 1x, "a2.png" 2x); }
</style>
<p style="background: url(inline.png)">"#;
    let resources = rejected(html);
    assert_eq!(
        resources
            .resources()
            .iter()
            .map(|r| (r.line, r.kind))
            .collect::<Vec<_>>(),
        vec![
            (2, Stylesheet),
            (3, Stylesheet),
            (4, Font),
            (5, Image),
            (6, Image),
            (6, Image),
            (8, Image),
        ]
    );
    assert_eq!(resources.resources()[3].code, "url('bg.png')");
}

#[test]
fn css_escapes_and_comments_do_not_hide_urls() {
    for html in [
        r"<style>a{background:\75 rl(https://example.com/a.png)}</style>",
        r"<style>a{background:u\72l(https://example.com/a.png)}</style>",
        r"<style>a{background:url(h\74tps://example.com/a.png)}</style>",
        r#"<style>a{background:url(  "https://example.com/a.png"  )}</style>"#,
        r#"<style>a{background:/* x */url(https://example.com/a.png)}</style>"#,
        r#"<style>a{background:URL("https://example.com/a.png")}</style>"#,
        r#"<p style="background:u&#114;l(https://example.com/a.png)">"#,
        r#"<p style="background:url(&quot;https://example.com/a.png&quot;)">"#,
        r#"<style>@media screen { @supports (display:grid) { a { background: url(https://example.com/a.png) } } }</style>"#,
    ] {
        assert_eq!(kinds(html).len(), 1, "{html}");
    }
    assert_eq!(
        kinds(r#"<style>@\69mport "x.css";</style>"#),
        vec![Stylesheet]
    );
}

#[test]
fn svg_style_split_by_comments_or_cdata_is_joined() {
    assert_eq!(
        kinds("<svg><style>a{fill:u<!---->rl(https://example.com/p.svg)}</style></svg>"),
        vec![Image]
    );
    assert_eq!(
        kinds("<svg><style><![CDATA[@import 'https://example.com/a.css';]]></style></svg>"),
        vec![Stylesheet]
    );
    assert_eq!(
        kinds("<svg><style>a{fill:url(h&#116;tps://example.com/p.svg)}</style></svg>"),
        vec![Image]
    );
}

#[test]
fn invalid_css_urls_are_ignored_like_browsers() {
    // 引用符の前にコメントがある url( は不正な URL になり、ブラウザーも読み込まない
    assert_accepted(r#"<style>a{background:url(/* x */"https://example.com/a.png")}</style>"#);
}

#[test]
fn positions_in_svg_style_with_references_point_to_source_lines() {
    let html = "<svg>\n<style>\n.a{fill:#&#x30;00}\n.b{fill:url(h&#116;tps://example.com/p.svg)}\n</style>\n</svg>";
    let resources = rejected(html);
    let found = &resources.resources()[0];
    assert_eq!(found.line, 4);
    assert_eq!(found.code, "url(https://example.com/p.svg)");
}

#[test]
fn positions_in_svg_script_with_references_point_to_source_lines() {
    let html = "<svg>\n<script>\nconst a = '&amp;&amp;&amp;';\nimport('https://esm.sh/x')\n</script>\n</svg>";
    let resources = rejected(html);
    let found = &resources.resources()[0];
    assert_eq!(found.line, 4);
    assert_eq!(found.code, "import('https://esm.sh/x')");
}

#[test]
fn consecutive_style_elements_are_checked_separately() {
    assert_eq!(
        kinds("<style>a{}</style><style>b{background:url(x.png)}</style>"),
        vec![Image]
    );
}

// ---- スクリプト ----

#[test]
fn detects_module_imports_and_import_maps() {
    let html = r#"<script type="importmap">
{ "imports": { "three": "https://esm.sh/three@0.160.0" } }
</script>
<script type="module">
import * as THREE from "three";
import { Chart } from "https://cdn.jsdelivr.net/npm/chart.js@4/+esm";
const d3 = await import("https://esm.sh/d3");
</script>"#;
    let resources = rejected(html);
    assert_eq!(
        resources
            .resources()
            .iter()
            .map(|r| (r.line, r.kind))
            .collect::<Vec<_>>(),
        vec![(2, Script), (6, Script), (7, Script)]
    );
    assert_eq!(
        resources.resources()[1].code,
        r#"import { Chart } from "https://cdn.jsdelivr.net/npm/chart.js@4/+esm";"#
    );
}

#[test]
fn import_map_type_with_references_is_detected() {
    let html = r#"<script type="imp&#111;rtmap">{"imports":{"chart":"https://example.com/chart.js"}}</script>
<script type="module">import chart from "chart"</script>"#;
    assert_eq!(kinds(html), vec![Script]);
}

#[test]
fn link_as_with_references_is_classified() {
    assert_eq!(
        kinds(r#"<link rel="preload" as="scr&#105;pt" href="https://example.com/a.js">"#),
        vec![Script]
    );
}

// ---- 入れ子（srcdoc、data: URL） ----

#[test]
fn detects_resources_inside_srcdoc() {
    assert_eq!(
        kinds(r#"<iframe srcdoc="<img src='https://example.com/a.png'>"></iframe>"#),
        vec![Image]
    );
    assert_eq!(
        kinds(
            r#"<iframe srcdoc="&lt;script src=&quot;https://example.com/a.js&quot;&gt;&lt;/script&gt;"></iframe>"#
        ),
        vec![Script]
    );
}

#[test]
fn detects_resources_inside_data_urls() {
    assert_eq!(
        kinds(
            r#"<iframe src="data:text/html,%3Cimg%20src%3Dhttps://example.com/a.png%3E"></iframe>"#
        ),
        vec![Image]
    );
    assert_eq!(
        kinds(
            r#"<link rel="stylesheet" href="data:text/css,@import 'https://example.com/a.css';">"#
        ),
        vec![Stylesheet]
    );
    // <image href="https://example.com/a.png"/> を base64 にしたもの
    assert_eq!(
        kinds(
            r#"<object data="data:image/svg+xml;base64,PHN2Zz48aW1hZ2UgaHJlZj0iaHR0cHM6Ly9leGFtcGxlLmNvbS9hLnBuZyIvPjwvc3ZnPg=="></object>"#
        ),
        vec![Image]
    );
    assert_eq!(
        kinds(
            r#"<script type="module" src="data:text/javascript,import 'https://esm.sh/x'"></script>"#
        ),
        vec![Script]
    );
    assert_eq!(
        kinds(
            r#"<style>a{background:url("data:image/svg+xml,<svg><image href='https://example.com/a.png'/></svg>")}</style>"#
        ),
        vec![Image]
    );
}

#[test]
fn nesting_depth_is_limited() {
    let mut html = r#"<img src="https://example.com/a.png">"#.to_owned();
    for _ in 0..10 {
        html = format!(
            r#"<iframe srcdoc="{}"></iframe>"#,
            html.replace('&', "&amp;").replace('"', "&quot;")
        );
    }
    assert_accepted(&html);
}

// ---- 報告の内容 ----

#[test]
fn reports_line_code_and_url() {
    let html = "<!doctype html>\n<html>\n<body>\n  <img\n    alt=\"ロゴ\"\n    src=\"https://example.com/logo.png\">\n</body>";
    let resources = rejected(html);
    let found = &resources.resources()[0];
    assert_eq!(found.line, 6);
    assert_eq!(found.kind, Image);
    assert_eq!(
        found.code,
        r#"<img alt="ロゴ" src="https://example.com/logo.png">"#
    );
    assert_eq!(found.url, "https://example.com/logo.png");
}

#[test]
fn long_code_is_truncated() {
    let html = format!(
        r#"<img src="https://example.com/a.png" alt="{}">"#,
        "あ".repeat(300)
    );
    let code = rejected(&html).resources()[0].code.clone();
    assert_eq!(code.chars().count(), 121);
    assert!(code.ends_with('…'));
}

#[test]
fn lists_at_most_max_listed_but_counts_all() {
    let html = r#"<img src="https://example.com/a.png">"#.repeat(150);
    let resources = rejected(&html);
    assert_eq!(resources.resources().len(), ExternalResources::MAX_LISTED);
    assert_eq!(resources.total(), 150);
}

#[test]
fn error_message_contains_rejection_reason() {
    let err = parse(r#"<script src="https://cdn.example.com/a.js"></script>"#).unwrap_err();
    assert!(err.to_string().contains(REJECTION_REASON));
}

#[test]
fn ai_request_lists_found_kinds() {
    let resources = rejected(
        r#"<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0"></script>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Noto+Sans+JP">
<img src="https://example.com/a.png">"#,
    );
    assert_eq!(
        resources.ai_request(),
        "chart.js、Web フォント、画像を外部から読み込まず、すべてを HTML の中に埋め込んだ、1ファイルで完結する HTML に作り直してください。"
    );
}

// ---- 壊れた入力 ----

#[test]
fn broken_or_unusual_markup_does_not_panic() {
    let cases = [
        "<svg><script>import('h&#116;tps://e.com/あ.js')</script></svg>",
        "<svg><style>a{b:url(&#x3042;&#x3042;.png)}</style></svg>",
        "<img src=\"",
        "<style>a{background:url(",
        "<script>import('",
        "<meta http-equiv=refresh content=\"0;あいう\">",
        "<img srcset=\",,, ,\">",
        "<iframe srcdoc=\"<iframe srcdoc='<iframe srcdoc=&quot;<img src=x>&quot;>'>\">",
        "<link rel=stylesheet href=\"data:text/css;base64,@@@\">",
        "<a href=x><<<>>></a>\u{0}\u{feff}",
        "<![CDATA[<img src=https://e.com>]]>",
        "<plaintext><img src=https://e.com/a.png>",
    ];
    for html in cases {
        let _ = parse(html);
    }
    let mut noise = String::new();
    for i in 0..5000u32 {
        noise.push(char::from_u32(0x20 + (i * 7919) % 0x3000).unwrap_or('あ'));
        if i % 13 == 0 {
            noise.push_str("<img src='");
        }
        if i % 17 == 0 {
            noise.push_str("url(&#");
        }
    }
    let _ = parse(&noise);
}

#[test]
fn deeply_nested_css_does_not_overflow_stack() {
    let depth = 300_000;
    let html = format!(r#"<div style="a:{}"></div>"#, "b(".repeat(depth));
    let _ = parse(&html);
    let html = format!("<style>a{{b:{}}}</style>", "((".repeat(depth));
    let _ = parse(&html);
    let html = format!("<style>{}</style>", "@media x{".repeat(depth));
    let _ = parse(&html);
}

#[test]
fn deeply_nested_elements_do_not_overflow_stack() {
    let depth = 300_000;
    let html = format!(
        "{}<img src=https://example.com/a.png>",
        "<div><svg><g>".repeat(depth)
    );
    assert_eq!(kinds(&html), vec![Image]);
}

// ---- 資料名 ----

#[test]
fn extracts_title() {
    let doc = parse("<html><head><title>\n  売上 &amp; 利益\n  レポート </title></head></html>")
        .map_err(|e| e.to_string());
    assert_eq!(
        doc.as_ref().map(|d| d.title()),
        Ok(Some("売上 & 利益 レポート"))
    );
}

#[test]
fn title_is_none_when_missing_or_blank() {
    assert_eq!(
        parse("<p>本文").map(|d| d.title().map(str::to_owned)),
        Ok(None)
    );
    assert_eq!(
        parse("<title>  </title>").map(|d| d.title().map(str::to_owned)),
        Ok(None)
    );
}

#[test]
fn svg_title_is_not_document_title() {
    let doc = parse("<svg><title>図</title></svg><title>資料</title>");
    assert_eq!(
        doc.map(|d| d.title().map(str::to_owned)),
        Ok(Some("資料".to_owned()))
    );
}

#[test]
fn keeps_bytes_as_uploaded() {
    let html = "<!doctype html><title>x</title>\r\n<p>そのまま</p>";
    let doc = parse(html).map(HtmlDocument::into_bytes);
    assert_eq!(doc, Ok(html.as_bytes().to_vec()));
}
