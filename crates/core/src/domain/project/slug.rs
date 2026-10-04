//! プロジェクトの slug。

use std::fmt;

use crate::domain::shared::{Slug, SlugError};

/// プロジェクトの slug に使えない語。`/archive` などトップレベルのパスと衝突する。
pub const RESERVED_PROJECT_SLUGS: &[&str] = &[
    "archive", "connect", "api", "mcp", "settings", "assets", "static", "health", "new",
];

/// プロジェクトの slug。予約語（[`RESERVED_PROJECT_SLUGS`]）は使えない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectSlug(Slug);

impl ProjectSlug {
    pub fn parse(value: &str) -> Result<Self, SlugError> {
        Slug::parse(value)?
            .reject_reserved(RESERVED_PROJECT_SLUGS)
            .map(Self)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reserved_words() {
        for word in RESERVED_PROJECT_SLUGS {
            assert_eq!(
                ProjectSlug::parse(word),
                Err(SlugError::Reserved((*word).to_owned()))
            );
        }
        assert!(ProjectSlug::parse("archive-2026").is_ok());
    }
}
