//! 名前やメモなど、画面から入力する文字列の値オブジェクト。
//!
//! 前後の空白は取り除き、長さは文字数（`char` の数）で数える。

use std::fmt;

pub const PROJECT_NAME_MAX_CHARS: usize = 100;
pub const PROJECT_DESCRIPTION_MAX_CHARS: usize = 1000;
pub const DOCUMENT_TITLE_MAX_CHARS: usize = 200;
pub const AUTHOR_NAME_MAX_CHARS: usize = 50;
pub const REVISION_MESSAGE_MAX_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("{field}を入力してください")]
    Empty { field: &'static str },
    #[error("{field}は {max} 文字までです")]
    TooLong { field: &'static str, max: usize },
}

fn trimmed(
    value: &str,
    field: &'static str,
    max: usize,
    allow_empty: bool,
) -> Result<String, TextError> {
    let value = value.trim();
    if value.is_empty() && !allow_empty {
        return Err(TextError::Empty { field });
    }
    if value.chars().count() > max {
        return Err(TextError::TooLong { field, max });
    }
    Ok(value.to_owned())
}

macro_rules! text_value {
    ($(#[$meta:meta])* $name:ident, $field:literal, $max:expr, allow_empty = $allow_empty:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, TextError> {
                trimmed(value, $field, $max, $allow_empty).map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

text_value!(
    /// プロジェクトの名前。
    ProjectName, "プロジェクト名", PROJECT_NAME_MAX_CHARS, allow_empty = false
);
text_value!(
    /// プロジェクトの説明。空でもよい。
    ProjectDescription, "説明", PROJECT_DESCRIPTION_MAX_CHARS, allow_empty = true
);
text_value!(
    /// 資料名。
    DocumentTitle, "資料名", DOCUMENT_TITLE_MAX_CHARS, allow_empty = false
);
text_value!(
    /// 更新者の名前。ログインがないので自己申告。
    AuthorName, "更新者の名前", AUTHOR_NAME_MAX_CHARS, allow_empty = false
);
text_value!(
    /// 版の変更メモ。空でもよい。
    RevisionMessage, "変更メモ", REVISION_MESSAGE_MAX_CHARS, allow_empty = true
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(
            ProjectName::parse("  営業部  ").map(|n| n.to_string()),
            Ok("営業部".to_owned())
        );
    }

    #[test]
    fn required_text_rejects_blank() {
        assert_eq!(
            AuthorName::parse(" \t"),
            Err(TextError::Empty {
                field: "更新者の名前"
            })
        );
    }

    #[test]
    fn optional_text_accepts_blank() {
        assert_eq!(
            RevisionMessage::parse(" ").map(|m| m.to_string()),
            Ok(String::new())
        );
    }

    #[test]
    fn length_is_counted_in_chars() {
        let max = "あ".repeat(AUTHOR_NAME_MAX_CHARS);
        assert!(AuthorName::parse(&max).is_ok());
        assert_eq!(
            AuthorName::parse(&format!("{max}あ")),
            Err(TextError::TooLong {
                field: "更新者の名前",
                max: AUTHOR_NAME_MAX_CHARS
            })
        );
    }
}
