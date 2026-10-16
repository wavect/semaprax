//! Closed error data for the selected lexical/renewal receiving cut only.
//! No conversion from Diagnostic and no source, lookup or payer authority.
use crate::diagnostic::Diagnostic;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    Profile(super::profile_refusal::ProfileRefusal),
    ReferenceCoverage(ReferenceCoverage),
    Structural(super::structural_refusal::StructuralRefusal),
    LexicalDepth,
    RenewalCensus,
    HelperCensus,
    CollectionProbe,
    RenewalWork,
    LostPhysical,
}
impl Refusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Profile(reason) => reason.detail(),
            Self::ReferenceCoverage(reason) => reason.detail(),
            Self::Structural(reason) => reason.detail(),
            Self::LexicalDepth => "lexical control scan exceeds admitted expression depth",
            Self::RenewalCensus => "selected primitive renewal census refused",
            Self::HelperCensus => {
                "byte helper renewal census exceeds admitted scratch, work or depth"
            }
            Self::CollectionProbe => "selected collection probe prepayment refused",
            Self::RenewalWork => "selected renewal work exhausted",
            Self::LostPhysical => "selected replay lost physical reservation",
        }
    }
    pub(crate) fn prefixed(self) -> bool {
        match self {
            Self::Profile(reason) => reason.prefixed(),
            Self::ReferenceCoverage(_) | Self::Structural(_) => true,
            _ => matches!(self, Self::RenewalWork | Self::LostPhysical),
        }
    }
    /// Explicit legacy renderer; callers alone own this allocating boundary.
    pub(crate) fn legacy(self) -> Diagnostic {
        Diagnostic::io("SPX-H006", self.detail())
    }
}

/// Closed labels from the complete reference-coverage validator, no identity text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReferenceCoverage {
    EdgeOwners,
    ExitOwners,
    StatusDeclarations,
}
impl ReferenceCoverage {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::EdgeOwners => {
                "each cleanup edge must be referenced by exactly one control-flow owner"
            }
            Self::ExitOwners => "each cleanup exit must be referenced by exactly one block",
            Self::StatusDeclarations => "declared and referenced cleanup status sources differ",
        }
    }
}
