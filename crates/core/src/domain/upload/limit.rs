//! 1ファイルのサイズ上限。

use std::{
    fmt,
    num::{IntErrorKind, ParseIntError},
};

const KIB: usize = 1024;
const MIB: usize = 1024 * 1024;

/// 1ファイルのサイズ上限。
///
/// Chart.js などを埋め込むと数百 KB、画像入りなら数 MB になる前提で、既定は 10MB にする。
/// 起動時に環境変数（`GALLEY_MAX_HTML_SIZE`）で [`HtmlSizeLimit::MAX`] まで変えられる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HtmlSizeLimit(usize);

/// サイズ上限として使えない値。起動時に、環境変数の名前を添えて止める。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HtmlSizeLimitError {
    #[error("「20MB」「512KB」のように、整数に KB か MB を付けて書いてください")]
    Invalid,
    #[error("0 より大きくしてください")]
    Zero,
    #[error("{} 以下にしてください", HtmlSizeLimit::MAX)]
    TooLarge,
}

impl HtmlSizeLimit {
    /// 環境変数で指定しないときの上限。
    pub const DEFAULT: Self = Self(10 * MIB);
    /// 設定できる上限。桁を間違えて、1回のリクエストでメモリを使い切るような値にしないため。
    pub const MAX: Self = Self(100 * MIB);

    /// 「20MB」「512KB」を読む（1MB = 1024 × 1024 バイト。画面の表示と同じ）。単位は大文字小文字を問わず、
    /// 数字との間に空白があってもよい。
    ///
    /// 単位の無い数字（「10」）は断る。MB のつもりで書いて 10 バイトになる取り違えを防ぐため。
    pub fn parse(value: &str) -> Result<Self, HtmlSizeLimitError> {
        let value = value.trim();
        let (number, unit) = value.split_at(
            value
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(value.len()),
        );
        let unit = match unit.trim_start().to_ascii_uppercase().as_str() {
            "KB" => KIB,
            "MB" => MIB,
            _ => return Err(HtmlSizeLimitError::Invalid),
        };
        let number: usize = number
            .parse()
            .map_err(|error: ParseIntError| match error.kind() {
                IntErrorKind::PosOverflow => HtmlSizeLimitError::TooLarge,
                _ => HtmlSizeLimitError::Invalid,
            })?;
        Self::from_bytes(
            number
                .checked_mul(unit)
                .ok_or(HtmlSizeLimitError::TooLarge)?,
        )
    }

    /// バイト数で指定する。
    pub fn from_bytes(bytes: usize) -> Result<Self, HtmlSizeLimitError> {
        match bytes {
            0 => Err(HtmlSizeLimitError::Zero),
            bytes if bytes > Self::MAX.0 => Err(HtmlSizeLimitError::TooLarge),
            bytes => Ok(Self(bytes)),
        }
    }

    /// 上限のバイト数。HTML がこれより大きければ断る。
    pub fn bytes(self) -> usize {
        self.0
    }

    /// HTML を JSON の文字列に入れて送るリクエスト（画面の登録の API、MCP）のボディの上限。
    /// JSON のエスケープで増える分の余裕を持たせる。
    pub fn json_body_limit(self) -> usize {
        self.0 * 2 + MIB
    }
}

/// 「10MB」「512KB」（[`HtmlSizeLimit::parse`] で読める形）。KB で割り切れなければバイト数。
impl fmt::Display for HtmlSizeLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            bytes if bytes.is_multiple_of(MIB) => write!(f, "{}MB", bytes / MIB),
            bytes if bytes.is_multiple_of(KIB) => write!(f, "{}KB", bytes / KIB),
            bytes => write!(f, "{bytes} バイト"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_size_with_unit() {
        for (value, bytes) in [
            ("10MB", 10 * MIB),
            ("20mb", 20 * MIB),
            (" 512 KB ", 512 * KIB),
            ("1kb", KIB),
            ("100MB", 100 * MIB),
            ("0100MB", 100 * MIB),
        ] {
            assert_eq!(
                HtmlSizeLimit::parse(value).map(HtmlSizeLimit::bytes),
                Ok(bytes),
                "{value}"
            );
        }
    }

    #[test]
    fn rejects_value_without_unit_or_with_other_forms() {
        for value in [
            "", "10", "10485760", "MB", "1.5MB", "10M", "10GB", "10 M B", "-1MB", "+1MB", "１０MB",
        ] {
            assert_eq!(
                HtmlSizeLimit::parse(value),
                Err(HtmlSizeLimitError::Invalid),
                "{value}"
            );
        }
    }

    #[test]
    fn rejects_zero_and_too_large() {
        assert_eq!(HtmlSizeLimit::parse("0MB"), Err(HtmlSizeLimitError::Zero));
        assert_eq!(HtmlSizeLimit::parse("0KB"), Err(HtmlSizeLimitError::Zero));
        assert_eq!(
            HtmlSizeLimit::parse("101MB"),
            Err(HtmlSizeLimitError::TooLarge)
        );
        assert_eq!(
            HtmlSizeLimit::parse("102401KB"),
            Err(HtmlSizeLimitError::TooLarge)
        );
        assert_eq!(
            HtmlSizeLimit::parse("99999999999999999999999MB"),
            Err(HtmlSizeLimitError::TooLarge)
        );
        assert_eq!(
            HtmlSizeLimit::parse(&format!("{}MB", usize::MAX / 2)),
            Err(HtmlSizeLimitError::TooLarge)
        );
    }

    #[test]
    fn json_body_limit_leaves_room_for_escaping() {
        assert_eq!(HtmlSizeLimit::DEFAULT.json_body_limit(), 10 * MIB * 2 + MIB);
        assert_eq!(HtmlSizeLimit::MAX.json_body_limit(), 201 * MIB);
    }

    #[test]
    fn displays_in_parsable_form() {
        assert_eq!(HtmlSizeLimit::DEFAULT.to_string(), "10MB");
        assert_eq!(
            HtmlSizeLimit::parse("512KB").map(|l| l.to_string()),
            Ok("512KB".to_owned())
        );
        assert_eq!(
            HtmlSizeLimit::from_bytes(1500).map(|l| l.to_string()),
            Ok("1500 バイト".to_owned())
        );
    }
}
