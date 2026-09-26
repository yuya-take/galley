use crate::domain::shared::text_value;

pub const AUTHOR_NAME_MAX_CHARS: usize = 50;
pub const REVISION_MESSAGE_MAX_CHARS: usize = 500;

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
    use crate::domain::shared::TextError;

    #[test]
    fn author_name_is_required_and_limited() {
        assert!(AuthorName::parse(" ").is_err());
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

    #[test]
    fn revision_message_may_be_empty() {
        assert_eq!(
            RevisionMessage::parse(" ").map(|m| m.to_string()),
            Ok(String::new())
        );
    }
}
