//! AI（Claude Code など）に Galley の MCP を登録するコマンド。空のプロジェクトの案内と、
//! 接続の手引き（`/connect`）で使う。

/// 名前が未設定のときにコマンドへ入れる文字。
pub const AUTHOR_PLACEHOLDER: &str = "あなたの名前";

/// Claude Code に登録するコマンド。`author` は更新者名（`X-Author-Name` ヘッダー）。
///
/// 更新者名は自己申告で記号も入るので、コピーしたコマンドがシェルでそのまま動くようにクォートする。
pub fn claude_code(mcp_url: &str, author: Option<&str>) -> String {
    let header = format!("X-Author-Name: {}", author.unwrap_or(AUTHOR_PLACEHOLDER));
    format!(
        "claude mcp add --transport http galley {} --header {}",
        shell_word(mcp_url),
        shell_word(&header)
    )
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
    fn quotes_ipv6_urls() {
        // zsh では角括弧がファイル名の展開になる
        assert_eq!(
            shell_word("http://[::1]:8080/mcp"),
            "\"http://[::1]:8080/mcp\""
        );
    }
}
