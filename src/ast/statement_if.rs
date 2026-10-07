//! Source syntax provenance; semantic visitors use the normalized Let value.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LetSyntax {
    #[default]
    Authored,
    StatementIf(StatementIfSyntax),
    /// Parser-generated discard of a nonliteral statement branch tail.
    BranchTail,
}

/// Only the parser mints this marker. It carries no alternate expression tree.
#[derive(Clone, Debug, PartialEq)]
pub struct StatementIfSyntax {
    pub(crate) branches: Vec<BranchTail>,
    pub(crate) alternative: Option<BranchTail>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BranchTail {
    Absent,
    Retained,
    Discarded,
}

impl StatementIfSyntax {
    pub fn branches(&self) -> &[BranchTail] {
        &self.branches
    }
    pub fn alternative(&self) -> Option<BranchTail> {
        self.alternative
    }
}
