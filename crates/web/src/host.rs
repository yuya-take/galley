//! Host ヘッダーや URL のホスト名の読み取り（Host の検査と資料配信の URL で共通に使う）。

/// `host[:port]` / `[ipv6][:port]` からホスト名を取り出し、小文字にする。
/// ホスト名は英数字・ハイフン・ドット、または角括弧で囲んだ IPv6 アドレス。
/// 形が正しくなければ（ポートが数でない、使えない文字がある）`None`。
pub fn hostname(authority: &str) -> Option<String> {
    let authority = authority.trim();
    let (host, port) = if authority.starts_with('[') {
        let end = authority.find(']')?;
        let port = match &authority[end + 1..] {
            "" => None,
            rest => Some(rest.strip_prefix(':')?),
        };
        (&authority[..=end], port)
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if port.is_some_and(|p| p.parse::<u16>().is_err()) {
        return None;
    }
    let is_valid = if let Some(ipv6) = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
        !ipv6.is_empty()
            && ipv6
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
    } else {
        !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    };
    is_valid.then(|| host.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_host_and_drops_port() {
        assert_eq!(
            hostname("Galley.Internal:8080").as_deref(),
            Some("galley.internal")
        );
        assert_eq!(hostname("192.168.1.10").as_deref(), Some("192.168.1.10"));
        assert_eq!(hostname("[::1]:8081").as_deref(), Some("[::1]"));
        assert_eq!(hostname("[::1]").as_deref(), Some("[::1]"));
    }

    #[test]
    fn rejects_malformed_authorities() {
        for value in [
            "",
            ":8080",
            "a b",
            "evil.com/x",
            "user@host",
            "host:99999",
            "host:x",
            "[::1",
            "[::1]x",
            "[]",
            "\"><",
        ] {
            assert_eq!(hostname(value), None, "{value:?}");
        }
    }
}
