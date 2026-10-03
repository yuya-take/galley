//! 資料配信の URL（アプリの iframe に入れる `src`）。
//!
//! 資料はアプリと別のオリジンから配信する。既定はアプリと同じホスト名で、資料配信のポート（8081）。
//! リバースプロキシで資料に別のホスト名を割り当てるときは `GALLEY_VIEWER_URL` で指定する。

use std::fmt;

use galley_core::domain::shared::RevisionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerUrl {
    /// 指定した URL（`https://galley-docs.example.com` など。末尾の `/` は除く）。
    Fixed(String),
    /// アプリへのリクエストと同じホスト名で、このポートにする。
    SameHost { port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "資料配信の URL「{0}」は http(s)://<ホスト名>[:ポート] の形で指定してください（パスは付けない）"
)]
pub struct ViewerUrlError(String);

impl ViewerUrl {
    /// `http(s)://<ホスト名>[:ポート]` を読む。パス・クエリ・ユーザー情報は受け付けない。
    pub fn parse(value: &str) -> Result<Self, ViewerUrlError> {
        let error = || ViewerUrlError(value.to_owned());
        let trimmed = value.trim().trim_end_matches('/');
        let lower = trimmed.to_ascii_lowercase();
        let authority = ["https://", "http://"]
            .iter()
            .find_map(|scheme| lower.strip_prefix(scheme))
            .ok_or_else(error)?;
        let (host, port) = split_host(authority).ok_or_else(error)?;
        if !is_valid_host(host) || port.is_some_and(|p| p.parse::<u16>().is_err()) {
            return Err(error());
        }
        Ok(Self::Fixed(lower))
    }

    /// 版の HTML の URL。`app_host` はアプリへのリクエストの Host ヘッダー（`SameHost` のときに使う）。
    /// Host ヘッダーがホスト名として正しくなければ `None`。
    pub fn revision_url(&self, app_host: &str, id: RevisionId) -> Option<String> {
        let base = match self {
            Self::Fixed(base) => base.clone(),
            Self::SameHost { port } => {
                let (host, _) = split_host(app_host)?;
                if !is_valid_host(host) {
                    return None;
                }
                format!("http://{host}:{port}")
            }
        };
        Some(format!("{base}/r/{id}"))
    }
}

impl fmt::Display for ViewerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(base) => f.write_str(base),
            Self::SameHost { port } => write!(f, "http://<アプリと同じホスト名>:{port}"),
        }
    }
}

/// `host[:port]` / `[ipv6][:port]` をホスト名とポートに分ける。
fn split_host(authority: &str) -> Option<(&str, Option<&str>)> {
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        let host = &authority[..=end];
        return match &authority[end + 1..] {
            "" => Some((host, None)),
            rest => Some((host, Some(rest.strip_prefix(':')?))),
        };
    }
    match authority.split_once(':') {
        Some((host, port)) => Some((host, Some(port))),
        None => Some((authority, None)),
    }
}

/// ホスト名（英数字・ハイフン・ドット）または `[` `]` で囲んだ IPv6 アドレス。
fn is_valid_host(host: &str) -> bool {
    if let Some(ipv6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        return !ipv6.is_empty()
            && ipv6
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.');
    }
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> RevisionId {
        RevisionId::generate()
    }

    #[test]
    fn same_host_uses_viewer_port() {
        let url = ViewerUrl::SameHost { port: 8081 };
        let id = id();
        assert_eq!(
            url.revision_url("galley.internal:8080", id),
            Some(format!("http://galley.internal:8081/r/{id}"))
        );
        assert_eq!(
            url.revision_url("192.168.1.10", id),
            Some(format!("http://192.168.1.10:8081/r/{id}"))
        );
        assert_eq!(
            url.revision_url("[::1]:8080", id),
            Some(format!("http://[::1]:8081/r/{id}"))
        );
    }

    #[test]
    fn same_host_rejects_odd_host_headers() {
        let url = ViewerUrl::SameHost { port: 8081 };
        for host in ["", "evil.com/x", "a b", "\"><script>", "[::1"] {
            assert_eq!(url.revision_url(host, id()), None, "{host:?}");
        }
    }

    #[test]
    fn fixed_url_is_used_as_is() {
        let url = ViewerUrl::parse("https://Galley-Docs.example.com/");
        assert_eq!(
            url,
            Ok(ViewerUrl::Fixed(
                "https://galley-docs.example.com".to_owned()
            ))
        );
        let id = id();
        assert_eq!(
            url.ok().and_then(|u| u.revision_url("ignored", id)),
            Some(format!("https://galley-docs.example.com/r/{id}"))
        );
        assert!(ViewerUrl::parse("http://localhost:18081").is_ok());
        assert!(ViewerUrl::parse("http://[::1]:8081").is_ok());
    }

    #[test]
    fn parse_rejects_paths_and_other_schemes() {
        for value in [
            "galley-docs.example.com",
            "ftp://example.com",
            "https://example.com/docs",
            "https://user@example.com",
            "https://example.com?x=1",
            "https://example.com:99999",
            "https://",
        ] {
            assert!(ViewerUrl::parse(value).is_err(), "{value}");
        }
    }
}
