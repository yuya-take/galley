//! 生の SQL（`toasty::sql`）で値を受け渡すための変換。
//!
//! Toasty 0.11 の SQLite ドライバーは、生の SQL で `Uuid` や `Timestamp` を渡したり
//! 型を指定して受け取ったりすると、エラーではなくパニック（`todo!()`）になる。
//! そのため UUID はバイト列（保存形式の BLOB）、日時は文字列（保存形式の TEXT）で扱う。

use jiff::Timestamp;
use toasty::stmt::Value;
use uuid::Uuid;

use crate::domain::error::RepositoryError;

use super::convert::corrupted;

/// UUID を保存形式（16 バイトの BLOB）で渡す。
pub(super) fn uuid_param(id: Uuid) -> Value {
    Value::Bytes(id.as_bytes().to_vec())
}

/// 日時を保存形式で渡す。Toasty は小数9桁固定の RFC 3339（UTC）で保存するので、
/// 文字列の大小と時刻の前後が一致する。
pub(super) fn timestamp_param(at: Timestamp) -> Value {
    Value::String(format!("{at:.9}"))
}

pub(super) fn uuid(value: Value, column: &str) -> Result<Uuid, RepositoryError> {
    match value {
        Value::Bytes(bytes) => Uuid::from_slice(&bytes).map_err(|e| corrupted(column, e)),
        other => Err(corrupted(column, format!("{other:?}"))),
    }
}

pub(super) fn timestamp(value: Value, column: &str) -> Result<Timestamp, RepositoryError> {
    match value {
        Value::String(text) => text.parse().map_err(|e| corrupted(column, e)),
        other => Err(corrupted(column, format!("{other:?}"))),
    }
}

pub(super) fn string(value: Value, column: &str) -> Result<String, RepositoryError> {
    match value {
        Value::String(text) => Ok(text),
        other => Err(corrupted(column, format!("{other:?}"))),
    }
}

pub(super) fn int(value: Value, column: &str) -> Result<i64, RepositoryError> {
    match value {
        Value::I64(n) => Ok(n),
        other => Err(corrupted(column, format!("{other:?}"))),
    }
}
