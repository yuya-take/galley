use crate::domain::shared::text_value;

pub const DOCUMENT_TITLE_MAX_CHARS: usize = 200;

text_value!(
    /// 資料名。
    DocumentTitle, "資料名", DOCUMENT_TITLE_MAX_CHARS, allow_empty = false
);
