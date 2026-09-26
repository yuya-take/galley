//! エンティティの ID。UUID v7（時系列でソートできる）を使う。

use std::fmt;

use uuid::Uuid;

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
