//! 更新者名。ログインがないので、ブラウザの Cookie（`galley_author`）に保存した自己申告の名前を使う。
//! 保存は画面のスクリプト（`assets/app.js`）が行い、サーバーは読むだけ。

use galley_core::domain::revision::AuthorName;
use http::{HeaderMap, header};

/// 更新者名を保存する Cookie の名前（`assets/app.js` と揃える）。
pub const AUTHOR_COOKIE: &str = "galley_author";

/// Cookie から更新者名を読む。無い・読めない・名前として正しくなければ `None`。
pub fn from_headers(headers: &HeaderMap) -> Option<AuthorName> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            (name == AUTHOR_COOKIE).then_some(value)
        })
        .and_then(percent_decode)
        .and_then(|name| AuthorName::parse(&name).ok())
}

/// 頭文字（名前の最初の1文字）。
pub fn initial(name: &AuthorName) -> String {
    name.as_str()
        .chars()
        .next()
        .map(String::from)
        .unwrap_or_default()
}

/// `encodeURIComponent` で保存した値を戻す。UTF-8 として読めなければ `None`。
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookie: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            cookie.parse().unwrap_or_else(|e| panic!("{e}")),
        );
        headers
    }

    #[test]
    fn reads_percent_encoded_name() {
        let name = from_headers(&headers(
            "theme=dark; galley_author=%E4%BD%90%E8%97%A4; x=1",
        ));
        assert_eq!(name.as_ref().map(AuthorName::as_str), Some("佐藤"));
        assert_eq!(name.as_ref().map(initial).as_deref(), Some("佐"));
    }

    #[test]
    fn ignores_missing_or_invalid_names() {
        assert_eq!(from_headers(&HeaderMap::new()), None);
        assert_eq!(from_headers(&headers("galley_author=")), None);
        assert_eq!(from_headers(&headers("galley_author=%E4%BD")), None);
        assert_eq!(from_headers(&headers("galley_author=%zz")), None);
        // 文字の向きを変える文字（RLO）を含む名前は使わない
        assert_eq!(from_headers(&headers("galley_author=%E2%80%AEa")), None);
        assert_eq!(from_headers(&headers("other_galley_author=x")), None);
    }
}
