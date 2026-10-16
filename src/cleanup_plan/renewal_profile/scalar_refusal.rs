//! Closed scalar replay refusal labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarRefusal {
    Layout,
    RootWork,
    TypeWork,
    FieldWork,
    Cursor,
    NonCopy,
    Lexical,
    Depth,
    Loan,
    Schema,
    Lease,
    Signature,
    Trace,
    Multiplicity,
}
impl ScalarRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Loan => "pure scalar helper carries an unexpected loan proof",
            Self::Schema => "selected scalar helper requires its exact nonlexical schema",
            Self::Lease => "selected scalar helper lacks its function lease",

            Self::Layout => "scalar record helper lacks a current flat layout",
            Self::RootWork => "scalar record root work overflow",
            Self::TypeWork => "scalar record type work overflow",
            Self::FieldWork => "scalar record field work overflow",
            Self::Cursor => "scalar profile cursor is empty",
            Self::NonCopy => "selected scalar helper body contains non-Copy storage",
            Self::Lexical => "selected scalar helper contains lexical control",
            Self::Depth => "scalar helper exceeds HIR depth",
            Self::Signature => "selected helper signature is outside Copy scalars",
            Self::Trace => "selected scalar helper control trace disagrees with typed HIR",
            Self::Multiplicity => "selected scalar helper trace multiplicity overflowed",
        }
    }
}
