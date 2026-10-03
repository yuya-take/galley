//! 複数の集約で使う値オブジェクト。

mod archive;
mod id;
pub(crate) mod slug;
pub(crate) mod text;

pub use archive::ArchiveFilter;
pub use id::{DocumentId, IdError, ProjectId, RevisionId};
pub(crate) use slug::Slug;
pub use slug::{SLUG_MAX_LEN, SlugError};
pub use text::TextError;
pub(crate) use text::text_value;
