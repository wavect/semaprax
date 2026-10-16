//! Closed status reconstruction refusal data; no source or replay authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatusRefusal {
    Interfaces,
    Preconditions,
    Postconditions,
    Coverage,
    Depth,
    Delegated,
    RangeIdentity,
}
impl StatusRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Interfaces => {
                "selected physical status reconstruction does not support interfaces"
            }
            Self::Preconditions => "too many preconditions",
            Self::Postconditions => "too many postconditions",
            Self::Coverage => {
                "cleanup status sources do not exactly cover typed HIR failure producers"
            }
            Self::Depth => "cleanup status expression depth exceeds 512",
            Self::Delegated => {
                "selected physical status reconstruction does not support delegated producers"
            }
            Self::RangeIdentity => "byte range status source carries an unknown operation identity",
        }
    }
}
