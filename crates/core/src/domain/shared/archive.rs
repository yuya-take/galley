/// アーカイブしたものを一覧に含めるか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArchiveFilter {
    /// アーカイブしていないものだけ。
    #[default]
    Active,
    /// アーカイブしたものだけ。
    Archived,
}
