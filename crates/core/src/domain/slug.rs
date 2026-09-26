//! URL に使う slug。
//!
//! 英小文字・数字・ハイフンだけで、先頭と末尾はハイフンにできず、ハイフンは連続できない。
//! プロジェクトと資料で予約語が違うので、型を分けている。

use std::fmt;

/// slug の最大の長さ（文字数）。
pub const SLUG_MAX_LEN: usize = 64;

/// プロジェクトの slug に使えない語。`/archive` などトップレベルのパスと衝突する。
pub const RESERVED_PROJECT_SLUGS: &[&str] = &[
    "archive", "connect", "api", "mcp", "settings", "assets", "static", "health",
];

/// 資料の slug に使えない語。`/:project/settings` と衝突する。
pub const RESERVED_DOCUMENT_SLUGS: &[&str] = &["settings"];

/// 資料の slug を自動で作るときの接頭辞（`doc-3f9a1c2e`）。
const GENERATED_DOCUMENT_SLUG_PREFIX: &str = "doc-";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SlugError {
    #[error("slug が空です")]
    Empty,
    #[error("slug は {SLUG_MAX_LEN} 文字までです")]
    TooLong,
    #[error("slug に使えるのは英小文字・数字・ハイフンだけです")]
    InvalidCharacter,
    #[error("slug の先頭と末尾にハイフンは使えず、ハイフンは連続できません")]
    InvalidHyphen,
    #[error("「{0}」は予約語のため slug に使えません")]
    Reserved(String),
}

/// 検査済みの slug。予約語の判定は [`ProjectSlug`] と [`DocumentSlug`] で行う。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Slug(String);

impl Slug {
    fn parse(value: &str) -> Result<Self, SlugError> {
        if value.is_empty() {
            return Err(SlugError::Empty);
        }
        if value.len() > SLUG_MAX_LEN {
            return Err(SlugError::TooLong);
        }
        if !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(SlugError::InvalidCharacter);
        }
        if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
            return Err(SlugError::InvalidHyphen);
        }
        Ok(Self(value.to_owned()))
    }

    /// 任意の文字列から slug を作る。英数字以外はハイフンにまとめる。英数字が1文字もなければ `None`。
    fn normalize(value: &str) -> Option<Self> {
        let mut slug = String::with_capacity(value.len());
        for c in value.chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        truncate(&mut slug, SLUG_MAX_LEN);
        let slug = slug.trim_end_matches('-');
        (!slug.is_empty()).then(|| Self(slug.to_owned()))
    }

    /// 末尾に `-2` などを付ける。長さの上限を超えるときは元の部分を削る。
    fn with_suffix(&self, n: u32) -> Self {
        let suffix = format!("-{n}");
        let mut base = self.0.clone();
        truncate(&mut base, SLUG_MAX_LEN - suffix.len());
        let base = base.trim_end_matches('-');
        Self(format!("{base}{suffix}"))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

/// ASCII だけの文字列を `max` バイトに切り詰める。
fn truncate(value: &mut String, max: usize) {
    if value.len() > max {
        value.truncate(max);
    }
}

/// `slug` から始めて、`is_taken` が偽になるまで `-2`、`-3` … を付ける。
fn first_available(slug: Slug, is_taken: impl Fn(&Slug) -> bool) -> Slug {
    if !is_taken(&slug) {
        return slug;
    }
    (2..)
        .map(|n| slug.with_suffix(n))
        .find(|candidate| !is_taken(candidate))
        .unwrap_or(slug)
}

/// プロジェクトの slug。予約語（[`RESERVED_PROJECT_SLUGS`]）は使えない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectSlug(Slug);

impl ProjectSlug {
    pub fn parse(value: &str) -> Result<Self, SlugError> {
        let slug = Slug::parse(value)?;
        if RESERVED_PROJECT_SLUGS.contains(&slug.as_str()) {
            return Err(SlugError::Reserved(slug.0));
        }
        Ok(Self(slug))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for ProjectSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 資料の slug。プロジェクトの中で一意。予約語（[`RESERVED_DOCUMENT_SLUGS`]）は使えない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentSlug(Slug);

impl DocumentSlug {
    pub fn parse(value: &str) -> Result<Self, SlugError> {
        let slug = Slug::parse(value)?;
        if Self::is_reserved(&slug) {
            return Err(SlugError::Reserved(slug.0));
        }
        Ok(Self(slug))
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
        Self(Slug(format!(
            "{GENERATED_DOCUMENT_SLUG_PREFIX}{random:08x}"
        )))
    }

    /// この slug が使えなければ（予約語か、`is_taken` が真なら）`-2`、`-3` … を付けて返す。
    pub fn first_available(self, is_taken: impl Fn(&Self) -> bool) -> Self {
        let slug = first_available(self.0, |slug| {
            Self::is_reserved(slug) || is_taken(&Self(slug.clone()))
        });
        Self(slug)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn is_reserved(slug: &Slug) -> bool {
        RESERVED_DOCUMENT_SLUGS.contains(&slug.as_str())
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

    #[test]
    fn parse_accepts_lowercase_digits_and_single_hyphens() {
        assert_eq!(
            ProjectSlug::parse("sales-2026").map(|s| s.to_string()),
            Ok("sales-2026".to_owned())
        );
    }

    #[test]
    fn parse_rejects_invalid_slugs() {
        assert_eq!(ProjectSlug::parse(""), Err(SlugError::Empty));
        assert_eq!(
            ProjectSlug::parse(&"a".repeat(SLUG_MAX_LEN + 1)),
            Err(SlugError::TooLong)
        );
        assert_eq!(
            ProjectSlug::parse("Sales"),
            Err(SlugError::InvalidCharacter)
        );
        assert_eq!(ProjectSlug::parse("営業"), Err(SlugError::InvalidCharacter));
        assert_eq!(ProjectSlug::parse("a_b"), Err(SlugError::InvalidCharacter));
        assert_eq!(ProjectSlug::parse("-a"), Err(SlugError::InvalidHyphen));
        assert_eq!(ProjectSlug::parse("a-"), Err(SlugError::InvalidHyphen));
        assert_eq!(ProjectSlug::parse("a--b"), Err(SlugError::InvalidHyphen));
    }

    #[test]
    fn project_slug_rejects_reserved_words() {
        for word in RESERVED_PROJECT_SLUGS {
            assert_eq!(
                ProjectSlug::parse(word),
                Err(SlugError::Reserved((*word).to_owned()))
            );
        }
        // 資料の予約語はプロジェクトでは使える
        assert!(ProjectSlug::parse("archive-2026").is_ok());
    }

    #[test]
    fn document_slug_rejects_reserved_words() {
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

        let slug = DocumentSlug::parse("free")
            .map(|s| s.first_available(|s| taken.contains(&s.as_str())))
            .map(|s| s.to_string());
        assert_eq!(slug, Ok("free".to_owned()));
    }

    #[test]
    fn first_available_avoids_reserved_words() {
        let slug = DocumentSlug::from_file_name("settings.html")
            .map(|s| s.first_available(|_| false))
            .map(|s| s.to_string());
        assert_eq!(slug, Some("settings-2".to_owned()));
    }

    #[test]
    fn suffix_keeps_max_length() {
        let long = DocumentSlug::parse(&"a".repeat(SLUG_MAX_LEN)).map(|s| {
            let original = s.clone();
            s.first_available(|candidate| *candidate == original)
        });
        let long = long.map(|s| s.to_string());
        let expected = format!("{}-2", "a".repeat(SLUG_MAX_LEN - 2));
        assert_eq!(long, Ok(expected));
    }
}
