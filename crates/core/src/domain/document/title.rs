use crate::domain::shared::text_value;

pub const DOCUMENT_TITLE_MAX_CHARS: usize = 200;

text_value!(
    /// 資料名。
    DocumentTitle, "資料名", DOCUMENT_TITLE_MAX_CHARS, allow_empty = false, multiline = false
);

/// 資料名が無いときの資料名。
pub const UNTITLED_DOCUMENT: &str = "無題の資料";

impl DocumentTitle {
    /// 登録するときの資料名。入力があればそれを、なければ HTML の `<title>` を
    /// （長ければ切り詰めて）使い、どちらも無ければ「無題の資料」にする。
    pub fn for_new_document(
        input: Option<&str>,
        html_title: Option<&str>,
    ) -> Result<Self, crate::domain::shared::TextError> {
        if let Some(input) = input.filter(|t| !t.trim().is_empty()) {
            return Self::parse(input);
        }
        let title = html_title
            .map(|t| {
                t.trim()
                    .chars()
                    .take(DOCUMENT_TITLE_MAX_CHARS)
                    .collect::<String>()
            })
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| UNTITLED_DOCUMENT.to_owned());
        Ok(Self(title.trim().to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_title_prefers_input_then_html_title() {
        let title =
            |input, html| DocumentTitle::for_new_document(input, html).map(|t| t.to_string());
        assert_eq!(
            title(Some(" 事業計画 "), Some("HTML")),
            Ok("事業計画".to_owned())
        );
        assert_eq!(
            title(Some("  "), Some("HTML の題")),
            Ok("HTML の題".to_owned())
        );
        assert_eq!(title(None, None), Ok(UNTITLED_DOCUMENT.to_owned()));
        assert_eq!(title(None, Some(" ")), Ok(UNTITLED_DOCUMENT.to_owned()));
        assert!(title(Some(&"あ".repeat(DOCUMENT_TITLE_MAX_CHARS + 1)), None).is_err());
    }

    #[test]
    fn long_html_title_is_truncated() {
        let long = "あ".repeat(DOCUMENT_TITLE_MAX_CHARS + 10);
        let title =
            DocumentTitle::for_new_document(None, Some(&long)).map(|t| t.as_str().chars().count());
        assert_eq!(title, Ok(DOCUMENT_TITLE_MAX_CHARS));
    }
}
