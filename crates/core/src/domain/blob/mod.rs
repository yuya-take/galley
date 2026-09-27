//! 資料の実体（HTML ファイル）。SHA-256 のハッシュで識別する。

mod store;

use std::fmt::{self, Write as _};

use jiff::Timestamp;
use sha2::{Digest, Sha256};

pub use store::BlobStore;

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

    /// 内容の SHA-256 を計算する。
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(Self::LEN);
        for byte in digest {
            // String への書き込みは失敗しない
            let _ = write!(hex, "{byte:02x}");
        }
        Self(hex)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// ストレージ上の置き場所（`ab/cd/<hash>.html`）。
    ///
    /// 1つのディレクトリにファイルが集まりすぎないよう、先頭4文字で2段に分ける。
    pub fn storage_key(&self) -> String {
        format!("{}/{}/{}.html", &self.0[..2], &self.0[2..4], self.0)
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
    /// 中身の SHA-256。
    pub hash: BlobHash,
    /// バイト数。
    pub size: u64,
    /// 初めて保存した日時。
    pub created_at: Timestamp,
}

/// ハッシュと中身の組。ハッシュは中身から計算するので、食い違うことはない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobContent {
    hash: BlobHash,
    bytes: Vec<u8>,
}

/// ストレージから読んだ中身がハッシュと一致しない。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("中身のハッシュが {expected} ではなく {actual} です")]
pub struct BlobHashMismatch {
    pub expected: BlobHash,
    pub actual: BlobHash,
}

impl BlobContent {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            hash: BlobHash::of(&bytes),
            bytes,
        }
    }

    /// ストレージから読んだ中身が、そのハッシュのものか確かめる。
    pub fn verify(expected: &BlobHash, bytes: Vec<u8>) -> Result<Self, BlobHashMismatch> {
        let content = Self::new(bytes);
        if &content.hash != expected {
            return Err(BlobHashMismatch {
                expected: expected.clone(),
                actual: content.hash,
            });
        }
        Ok(content)
    }

    pub fn hash(&self) -> &BlobHash {
        &self.hash
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// バイト数。
    pub fn size(&self) -> u64 {
        // usize は 64 ビット以下なので失われない
        self.bytes.len() as u64
    }

    /// DB に記録する情報。
    pub fn to_blob(&self, now: Timestamp) -> Blob {
        Blob {
            hash: self.hash.clone(),
            size: self.size(),
            created_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn parse_accepts_lowercase_sha256_hex() {
        assert_eq!(
            BlobHash::parse(EMPTY_SHA256).map(|h| h.to_string()),
            Ok(EMPTY_SHA256.to_owned())
        );
    }

    #[test]
    fn of_computes_sha256() {
        assert_eq!(BlobHash::of(b"").as_str(), EMPTY_SHA256);
        assert_eq!(
            BlobHash::of(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn storage_key_splits_by_leading_characters() {
        let hash = BlobHash::of(b"");
        assert_eq!(hash.storage_key(), format!("e3/b0/{EMPTY_SHA256}.html"));
    }

    #[test]
    fn content_hash_and_size_follow_bytes() {
        let content = BlobContent::new(b"abc".to_vec());
        assert_eq!(content.hash(), &BlobHash::of(b"abc"));
        assert_eq!(content.size(), 3);
        let blob = content.to_blob(Timestamp::UNIX_EPOCH);
        assert_eq!((blob.hash, blob.size), (BlobHash::of(b"abc"), 3));
    }

    #[test]
    fn verify_rejects_mismatched_bytes() {
        let expected = BlobHash::of(b"abc");
        assert!(BlobContent::verify(&expected, b"abc".to_vec()).is_ok());
        assert_eq!(
            BlobContent::verify(&expected, b"abd".to_vec()),
            Err(BlobHashMismatch {
                expected,
                actual: BlobHash::of(b"abd"),
            })
        );
    }

    #[test]
    fn parse_rejects_other_strings() {
        assert_eq!(BlobHash::parse("abc"), Err(BlobHashError));
        assert_eq!(BlobHash::parse(&"A".repeat(64)), Err(BlobHashError));
        assert_eq!(BlobHash::parse(&"g".repeat(64)), Err(BlobHashError));
    }
}
