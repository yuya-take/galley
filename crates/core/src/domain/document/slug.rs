//! 資料の slug。

use std::fmt;

use crate::domain::shared::{Slug, SlugError};

/// 資料の slug に使えない語。`/:project/settings` と衝突する。
pub const RESERVED_DOCUMENT_SLUGS: &[&str] = &["settings"];

/// 資料の slug を自動で作るときの接頭辞（`doc-3f9a1c2e`）。
const GENERATED_SLUG_PREFIX: &str = "doc-";

/// 資料の slug。プロジェクトの中で一意。予約語（[`RESERVED_DOCUMENT_SLUGS`]）は使えない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentSlug(Slug);

impl DocumentSlug {
    pub fn parse(value: &str) -> Result<Self, SlugError> {
        Slug::parse(value)?
            .reject_reserved(RESERVED_DOCUMENT_SLUGS)
            .map(Self)
    }

    /// ファイル名から slug を作る（`jigyo-keikaku.html` → `jigyo-keikaku`）。
    ///
    /// 拡張子を除き、英字は小文字に、英数字以外はハイフンにまとめる。英数字が1文字もない
    /// ファイル名（`事業計画.html` など）は `None` を返すので、[`Self::generated`] を使う。
    /// 予約語になった場合もそのまま返し、[`Self::first_available`] で `-2` を付ける。
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
        let stem = match name.rsplit_once('.') {
            Some((stem, ext))
                if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") =>
            {
                stem
            }
            _ => name,
        };
        Slug::normalize(stem).map(Self)
    }

    /// ファイル名がない資料の slug（`doc-3f9a1c2e`）。`random` は呼び出し側で乱数から作る。
    pub fn generated(random: u32) -> Self {
        Self(Slug::with_random(GENERATED_SLUG_PREFIX, random))
    }

    /// この slug が使えなければ（予約語か、`is_taken` が真なら）`-2`、`-3` … を付けて返す。
    pub fn first_available(self, is_taken: impl Fn(&Self) -> bool) -> Self {
        let slug = self.0.first_available(|slug| {
            RESERVED_DOCUMENT_SLUGS.contains(&slug.as_str()) || is_taken(&Self(slug.clone()))
        });
        Self(slug)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for DocumentSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shared::SLUG_MAX_LEN;

    #[test]
    fn rejects_reserved_words() {
        assert_eq!(
            DocumentSlug::parse("settings"),
            Err(SlugError::Reserved("settings".to_owned()))
        );
        assert!(DocumentSlug::parse("archive").is_ok());
    }

    #[test]
    fn from_file_name_strips_extension_and_normalizes() {
        let slug = |name| DocumentSlug::from_file_name(name).map(|s| s.to_string());
        assert_eq!(slug("jigyo-keikaku.html"), Some("jigyo-keikaku".to_owned()));
        assert_eq!(slug("Jigyo Keikaku.HTM"), Some("jigyo-keikaku".to_owned()));
        assert_eq!(
            slug("q3_report (final).html"),
            Some("q3-report-final".to_owned())
        );
        assert_eq!(slug("dir/sub\\plan.v2.html"), Some("plan-v2".to_owned()));
        assert_eq!(slug("notes.txt"), Some("notes-txt".to_owned()));
        assert_eq!(slug("事業計画_2026.html"), Some("2026".to_owned()));
        assert_eq!(slug("事業計画.html"), None);
        assert_eq!(slug(".html"), None);
    }

    #[test]
    fn from_file_name_truncates_long_names() {
        let name = format!("{}-b.html", "a".repeat(SLUG_MAX_LEN - 1));
        let slug = DocumentSlug::from_file_name(&name).map(|s| s.to_string());
        assert_eq!(slug, Some("a".repeat(SLUG_MAX_LEN - 1)));
    }

    #[test]
    fn generated_slug_is_valid() {
        let slug = DocumentSlug::generated(0x3f9a_1c2e);
        assert_eq!(slug.as_str(), "doc-3f9a1c2e");
        assert_eq!(DocumentSlug::parse(slug.as_str()), Ok(slug));
    }

    #[test]
    fn first_available_appends_suffix_on_collision() {
        let taken = ["plan", "plan-2"];
        let slug = DocumentSlug::parse("plan")
            .map(|s| s.first_available(|s| taken.contains(&s.as_str())))
            .map(|s| s.to_string());
        assert_eq!(slug, Ok("plan-3".to_owned()));
    }

    #[test]
    fn first_available_avoids_reserved_words() {
        let slug = DocumentSlug::from_file_name("settings.html")
            .map(|s| s.first_available(|_| false))
            .map(|s| s.to_string());
        assert_eq!(slug, Some("settings-2".to_owned()));
    }
}
