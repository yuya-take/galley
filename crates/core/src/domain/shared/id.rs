//! エンティティの ID。UUID v7（時系列でソートできる）を使う。

use std::fmt;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("ID の形式が正しくありません")]
pub struct IdError;

macro_rules! entity_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            /// 新しい ID を作る（UUID v7）。
            pub fn generate() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// URL などの文字列から読む。表記は小文字・ハイフン区切りの1通りだけ受け付ける
            /// （同じ ID に URL が何通りもできないようにする）。
            pub fn parse(value: &str) -> Result<Self, IdError> {
                let uuid = Uuid::try_parse(value).map_err(|_| IdError)?;
                if uuid.hyphenated().to_string() != value {
                    return Err(IdError);
                }
                Ok(Self(uuid))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

entity_id!(
    /// プロジェクトの ID。
    ProjectId
);
entity_id!(
    /// 資料の ID。
    DocumentId
);
entity_id!(
    /// 版の ID。
    RevisionId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_only_canonical_form() {
        let id = RevisionId::generate();
        assert_eq!(RevisionId::parse(&id.to_string()), Ok(id));
        assert_eq!(
            RevisionId::parse(&id.to_string().to_uppercase()),
            Err(IdError)
        );
        assert_eq!(
            RevisionId::parse(&id.as_uuid().simple().to_string()),
            Err(IdError)
        );
        assert_eq!(RevisionId::parse(&format!("{{{id}}}")), Err(IdError));
        assert_eq!(RevisionId::parse("../etc/passwd"), Err(IdError));
        assert_eq!(RevisionId::parse(""), Err(IdError));
    }
}
