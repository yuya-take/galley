//! AI（Claude Code など）に Galley の MCP を登録するコマンド。空のプロジェクトの案内と、
//! 接続の手引き（`/connect`）で使う。

use crate::AppUrl;

/// 名前が未設定のときにコマンドへ入れる文字。
pub const AUTHOR_PLACEHOLDER: &str = "あなたの名前";

/// 更新者名を渡すヘッダー（`crates/server` の MCP が読む）。
pub const AUTHOR_HEADER: &str = "X-Author-Name";

/// `MCP_TOKEN` を設定したサーバーが求めるヘッダー。
pub const TOKEN_HEADER: &str = "Authorization";

/// [`TOKEN_HEADER`] の値。トークンそのものは画面に出さないので、読む人が置き換える文字を入れておく。
pub const TOKEN_VALUE: &str = "Bearer <トークン>";

/// MCP の URL。リクエストのホストが読めないときは、置き換える場所が分かる文字にする。
pub fn url(app_url: &AppUrl, host: Option<&str>) -> String {
    app_url
        .mcp_url(host)
        .unwrap_or_else(|| "http://<アプリのホスト>/mcp".to_owned())
}

/// ヘッダーに入れる更新者名（未設定なら [`AUTHOR_PLACEHOLDER`]）。
pub fn author_value(author: Option<&str>) -> &str {
    author.unwrap_or(AUTHOR_PLACEHOLDER)
}

/// Claude Code に登録するコマンド。`author` は更新者名（`X-Author-Name` ヘッダー）。
///
/// 更新者名は自己申告で記号も入るので、コピーしたコマンドがシェルでそのまま動くようにクォートする。
pub fn claude_code(mcp_url: &str, author: Option<&str>) -> String {
    let header = format!("{AUTHOR_HEADER}: {}", author_value(author));
    format!(
        "claude mcp add --transport http galley {} --header {}",
        shell_word(mcp_url),
        shell_word(&header)
    )
}

/// `MCP_TOKEN` を設定したサーバーに登録するコマンド。[`claude_code`] に `Authorization` ヘッダーを足す。
pub fn claude_code_with_token(mcp_url: &str, author: Option<&str>) -> String {
    format!(
        "{} --header {}",
        claude_code(mcp_url, author),
        shell_word(&format!("{TOKEN_HEADER}: {TOKEN_VALUE}"))
    )
}

/// 設定ファイル（JSON）に接続先を書く MCP クライアント向けの例。キーの名前はクライアントで少し違うので、
/// 画面ではあくまで例として出す。
pub fn json_config(mcp_url: &str, author: Option<&str>) -> String {
    format!(
        r#"{{
  "mcpServers": {{
    "galley": {{
      "type": "http",
      "url": {url},
      "headers": {{
        {header}: {author}
      }}
    }}
  }}
}}"#,
        url = json_string(mcp_url),
        header = json_string(AUTHOR_HEADER),
        author = json_string(author_value(author)),
    )
}

/// JSON の文字列にする。更新者名は自己申告なので、`"` や `\` を含んでも設定ファイルが壊れないようにする。
fn json_string(value: &str) -> String {
    let mut json = String::with_capacity(value.len() + 2);
    json.push('"');
    for c in value.chars() {
        match c {
            '"' => json.push_str(r#"\""#),
            '\\' => json.push_str(r"\\"),
            c if c.is_control() => json.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => json.push(c),
        }
    }
    json.push('"');
    json
}

/// シェルの1語にする。記号を含まなければそのまま、空白などを含めばダブルクォート、
/// ダブルクォートの中でも特別な意味を持つ文字（`"` `$` `` ` `` `\` `!`）を含めばシングルクォートで囲む。
fn shell_word(value: &str) -> String {
    let is_plain = |c: char| {
        c.is_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '/' | ':' | '@' | '%' | '+' | '=' | ',')
    };
    if !value.is_empty() && value.chars().all(is_plain) {
        return value.to_owned();
    }
    if !value.contains(['"', '$', '`', '\\', '!']) {
        return format!("\"{value}\"");
    }
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_matches_the_design() {
        assert_eq!(
            claude_code("http://galley.internal:8080/mcp", Some("佐藤")),
            r#"claude mcp add --transport http galley http://galley.internal:8080/mcp --header "X-Author-Name: 佐藤""#
        );
        assert_eq!(
            claude_code("http://localhost:8080/mcp", None),
            r#"claude mcp add --transport http galley http://localhost:8080/mcp --header "X-Author-Name: あなたの名前""#
        );
    }

    #[test]
    fn quotes_names_with_shell_characters() {
        assert_eq!(
            claude_code("http://localhost:8080/mcp", Some(r#"a"$(rm -rf ~)"#)),
            r#"claude mcp add --transport http galley http://localhost:8080/mcp --header 'X-Author-Name: a"$(rm -rf ~)'"#
        );
        assert_eq!(shell_word("O'Brien!"), r"'O'\''Brien!'");
        assert_eq!(shell_word("佐藤 太郎"), "\"佐藤 太郎\"");
    }

    #[test]
    fn token_command_adds_authorization_header() {
        assert_eq!(
            claude_code_with_token("http://localhost:8080/mcp", Some("佐藤")),
            r#"claude mcp add --transport http galley http://localhost:8080/mcp --header "X-Author-Name: 佐藤" --header "Authorization: Bearer <トークン>""#
        );
    }

    #[test]
    fn url_falls_back_to_placeholder_without_host() {
        assert_eq!(
            url(&AppUrl::FromRequest, Some("localhost:8080")),
            "http://localhost:8080/mcp"
        );
        assert_eq!(
            url(&AppUrl::FromRequest, None),
            "http://<アプリのホスト>/mcp"
        );
    }

    #[test]
    fn json_config_lists_url_and_author_header() {
        assert_eq!(
            json_config("http://localhost:8080/mcp", Some("佐藤")),
            r#"{
  "mcpServers": {
    "galley": {
      "type": "http",
      "url": "http://localhost:8080/mcp",
      "headers": {
        "X-Author-Name": "佐藤"
      }
    }
  }
}"#
        );
        assert!(
            json_config("http://localhost:8080/mcp", None)
                .contains(r#""X-Author-Name": "あなたの名前""#)
        );
    }

    #[test]
    fn json_string_escapes_quotes_backslashes_and_controls() {
        assert_eq!(json_string(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(json_string("a\tb\u{7f}"), r#""a\u0009b\u007f""#);
        assert_eq!(json_string("佐藤 太郎"), "\"佐藤 太郎\"");
    }

    #[test]
    fn quotes_ipv6_urls() {
        // zsh では角括弧がファイル名の展開になる
        assert_eq!(
            shell_word("http://[::1]:8080/mcp"),
            "\"http://[::1]:8080/mcp\""
        );
    }
}
