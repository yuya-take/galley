use crate::domain::shared::text_value;

pub const PROJECT_NAME_MAX_CHARS: usize = 100;
pub const PROJECT_DESCRIPTION_MAX_CHARS: usize = 1000;

text_value!(
    /// プロジェクトの名前。
    ProjectName, "プロジェクト名", PROJECT_NAME_MAX_CHARS, allow_empty = false, multiline = false
);
text_value!(
    /// プロジェクトの説明。空でもよい。
    ProjectDescription, "説明", PROJECT_DESCRIPTION_MAX_CHARS, allow_empty = true, multiline = true
);
