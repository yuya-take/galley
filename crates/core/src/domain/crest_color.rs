//! プロジェクトの紋章の色。

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("紋章の色「{0}」はありません")]
pub struct CrestColorError(String);

/// サイドバーや一覧でプロジェクトを見分けるための色。値の見た目は `web-standards.md` を参照。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrestColor {
    Red,
    Blue,
    Green,
    Gold,
    Purple,
    Silver,
}

impl CrestColor {
    pub const ALL: [Self; 6] = [
        Self::Red,
        Self::Blue,
        Self::Green,
        Self::Gold,
        Self::Purple,
        Self::Silver,
    ];

    pub fn parse(value: &str) -> Result<Self, CrestColorError> {
        Self::ALL
            .into_iter()
            .find(|color| color.as_str() == value)
            .ok_or_else(|| CrestColorError(value.to_owned()))
    }

    /// DB と URL で使う名前（`red` など）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Gold => "gold",
            Self::Purple => "purple",
            Self::Silver => "silver",
        }
    }
}

impl fmt::Display for CrestColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_round_trips_every_color() {
        for color in CrestColor::ALL {
            assert_eq!(CrestColor::parse(color.as_str()), Ok(color));
        }
    }

    #[test]
    fn parse_rejects_unknown_color() {
        assert_eq!(
            CrestColor::parse("Red"),
            Err(CrestColorError("Red".to_owned()))
        );
    }
}
