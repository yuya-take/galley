//! 資料の実体（HTML ファイル）。SHA-256 のハッシュで識別する。

use std::fmt;

use jiff::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ブロブのハッシュは 64 文字の16進数（小文字）です")]
pub struct BlobHashError;

/// SHA-256 のハッシュ（64 文字の小文字の16進数）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobHash(String);

impl BlobHash {
    const LEN: usize = 64;

    pub fn parse(value: &str) -> Result<Self, BlobHashError> {
        let is_valid = value.len() == Self::LEN
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !is_valid {
            return Err(BlobHashError);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// DB に記録するブロブの情報。同じハッシュは1回だけ記録する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    pub hash: BlobHash,
    /// バイト数。
    pub size: u64,
    pub created_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_lowercase_sha256_hex() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(
            BlobHash::parse(hex).map(|h| h.to_string()),
            Ok(hex.to_owned())
        );
    }

    #[test]
    fn parse_rejects_other_strings() {
        assert_eq!(BlobHash::parse("abc"), Err(BlobHashError));
        assert_eq!(BlobHash::parse(&"A".repeat(64)), Err(BlobHashError));
        assert_eq!(BlobHash::parse(&"g".repeat(64)), Err(BlobHashError));
    }
}
