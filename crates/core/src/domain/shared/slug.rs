//! URL に使う slug の共通の検査。
//!
//! 英小文字・数字・ハイフンだけで、先頭と末尾はハイフンにできず、ハイフンは連続できない。
//! 予約語はプロジェクトと資料で違うので、公開する型は `ProjectSlug` と `DocumentSlug` に分けている。

/// slug の最大の長さ（文字数）。
pub const SLUG_MAX_LEN: usize = 64;

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

/// 検査済みの slug。予約語の判定は `ProjectSlug` と `DocumentSlug` で行う。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Slug(String);

impl Slug {
    pub(crate) fn parse(value: &str) -> Result<Self, SlugError> {
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

    /// `reserved` に含まれていれば予約語のエラーにする。
    pub(crate) fn reject_reserved(self, reserved: &[&str]) -> Result<Self, SlugError> {
        if reserved.contains(&self.as_str()) {
            return Err(SlugError::Reserved(self.0));
        }
        Ok(self)
    }

    /// 任意の文字列から slug を作る。英数字以外はハイフンにまとめる。英数字が1文字もなければ `None`。
    pub(crate) fn normalize(value: &str) -> Option<Self> {
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

    /// `prefix` に8桁の16進数を続けた slug（`doc-3f9a1c2e`）。`prefix` は英小文字とハイフンで終わる前提。
    pub(crate) fn with_random(prefix: &str, random: u32) -> Self {
        Self(format!("{prefix}{random:08x}"))
    }

    /// この slug から始めて、`is_taken` が偽になるまで `-2`、`-3` … を付ける。
    pub(crate) fn first_available(self, is_taken: impl Fn(&Self) -> bool) -> Self {
        if !is_taken(&self) {
            return self;
        }
        (2..)
            .map(|n| self.with_suffix(n))
            .find(|candidate| !is_taken(candidate))
            .unwrap_or(self)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// 末尾に `-2` などを付ける。長さの上限を超えるときは元の部分を削る。
    fn with_suffix(&self, n: u32) -> Self {
        let suffix = format!("-{n}");
        let mut base = self.0.clone();
        truncate(&mut base, SLUG_MAX_LEN - suffix.len());
        let base = base.trim_end_matches('-');
        Self(format!("{base}{suffix}"))
    }
}

/// ASCII だけの文字列を `max` バイトに切り詰める。
fn truncate(value: &mut String, max: usize) {
    if value.len() > max {
        value.truncate(max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_lowercase_digits_and_single_hyphens() {
        assert_eq!(
            Slug::parse("sales-2026").map(|s| s.as_str().to_owned()),
            Ok("sales-2026".to_owned())
        );
    }

    #[test]
    fn parse_rejects_invalid_slugs() {
        assert_eq!(Slug::parse(""), Err(SlugError::Empty));
        assert_eq!(
            Slug::parse(&"a".repeat(SLUG_MAX_LEN + 1)),
            Err(SlugError::TooLong)
        );
        assert_eq!(Slug::parse("Sales"), Err(SlugError::InvalidCharacter));
        assert_eq!(Slug::parse("営業"), Err(SlugError::InvalidCharacter));
        assert_eq!(Slug::parse("a_b"), Err(SlugError::InvalidCharacter));
        assert_eq!(Slug::parse("-a"), Err(SlugError::InvalidHyphen));
        assert_eq!(Slug::parse("a-"), Err(SlugError::InvalidHyphen));
        assert_eq!(Slug::parse("a--b"), Err(SlugError::InvalidHyphen));
    }

    #[test]
    fn first_available_appends_suffix_on_collision() {
        let taken = ["plan", "plan-2"];
        let slug = Slug::parse("plan").map(|s| s.first_available(|s| taken.contains(&s.as_str())));
        assert_eq!(slug.map(|s| s.0), Ok("plan-3".to_owned()));

        let slug = Slug::parse("free").map(|s| s.first_available(|s| taken.contains(&s.as_str())));
        assert_eq!(slug.map(|s| s.0), Ok("free".to_owned()));
    }

    #[test]
    fn suffix_keeps_max_length() {
        let long = Slug::parse(&"a".repeat(SLUG_MAX_LEN)).map(|s| {
            let original = s.clone();
            s.first_available(|candidate| *candidate == original)
        });
        let expected = format!("{}-2", "a".repeat(SLUG_MAX_LEN - 2));
        assert_eq!(long.map(|s| s.0), Ok(expected));
    }
}
