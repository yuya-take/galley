//! 名前やメモなど、画面から入力する文字列の値オブジェクトの共通部分。
//!
//! 前後の空白は取り除き、長さは文字数（`char` の数）で数える。
//! 個々の型（`ProjectName` など）は各集約のモジュールで [`text_value!`] を使って定義する。

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("{field}を入力してください")]
    Empty { field: &'static str },
    #[error("{field}は {max} 文字までです")]
    TooLong { field: &'static str, max: usize },
}

/// 前後の空白を取り除き、空と長さを検査する。
pub(crate) fn trimmed(
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

/// 文字列の値オブジェクトを定義する（`parse`・`as_str`・`Display`）。
macro_rules! text_value {
    ($(#[$meta:meta])* $name:ident, $field:literal, $max:expr, allow_empty = $allow_empty:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, $crate::domain::shared::TextError> {
                $crate::domain::shared::text::trimmed(value, $field, $max, $allow_empty).map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

pub(crate) use text_value;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_whitespace() {
        assert_eq!(
            trimmed("  営業部  ", "名前", 10, false),
            Ok("営業部".to_owned())
        );
    }

    #[test]
    fn required_text_rejects_blank() {
        assert_eq!(
            trimmed(" \t", "名前", 10, false),
            Err(TextError::Empty { field: "名前" })
        );
    }

    #[test]
    fn optional_text_accepts_blank() {
        assert_eq!(trimmed(" ", "メモ", 10, true), Ok(String::new()));
    }

    #[test]
    fn length_is_counted_in_chars() {
        assert!(trimmed(&"あ".repeat(3), "名前", 3, false).is_ok());
        assert_eq!(
            trimmed(&"あ".repeat(4), "名前", 3, false),
            Err(TextError::TooLong {
                field: "名前",
                max: 3
            })
        );
    }
}
