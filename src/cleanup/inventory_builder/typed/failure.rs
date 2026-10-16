//! Opaque typed source failures, never a Diagnostic relabelling API.
use crate::cleanup_plan::renewal_profile::{
    profile_refusal::ProfileRefusal, type_drop_refusal::TypeDropRefusal,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InventoryFailure(pub(super) Kind);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Work,
    Receiving,
    Finite(Reason),
    Profile(ProfileRefusal),
    TypeDrop(TypeDropRefusal),
    Physical(Physical),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Physical {
    Vector,
    Clone,
    Lifecycle,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Reason {
    SelectedOptionMetadataIsUnauthenticated,
    TooManyFunctionParameters,
    TooManyCleanupStorageCandidates,
    ConditionalEntryReferencesUnknownStorage,
    BudgetedHelperInventoryPatternIsOutsideExactCopyMetadata,
    RecordPatternHasNoFieldInventory,
    RecordPatternFieldInventoryIsIncomplete,
    NestedRecordPatternHasNoFieldInventory,
    NestedRecordPatternFieldInventoryIsIncomplete,
    SelectedInventoryCloneBackingOverflow,
    SelectedInventoryHasNoPrimitiveLifecycle,
    TooManyCleanupLivenessFlags,
    SelectedInventoryExceedsOwnedLeafLimit,
    SelectedCollectionProbePrepaymentRefused,
    SelectedInventoryTypeHasUnsupportedArguments,
    SelectedInventoryKeyLengthOverflow,
}
impl InventoryFailure {
    pub(crate) fn detail(self) -> &'static str {
        match self.0 {
            Kind::Work => "selected inventory work exhausted",
            Kind::Receiving => "selected inventory receiving reservation refused",
            Kind::Profile(reason) => reason.detail(),
            Kind::TypeDrop(reason) => reason.detail(),
            Kind::Physical(reason) => match reason {
                Physical::Vector => "selected inventory physical reservation refused",
                Physical::Clone => "selected inventory clone reservation refused",
                Physical::Lifecycle => "selected inventory lifecycle reservation refused",
            },
            Kind::Finite(reason) => match reason {
                Reason::SelectedOptionMetadataIsUnauthenticated => {
                    "selected Option metadata is unauthenticated"
                }
                Reason::TooManyFunctionParameters => "too many function parameters",
                Reason::TooManyCleanupStorageCandidates => "too many cleanup storage candidates",
                Reason::ConditionalEntryReferencesUnknownStorage => {
                    "conditional entry references unknown storage"
                }
                Reason::BudgetedHelperInventoryPatternIsOutsideExactCopyMetadata => {
                    "budgeted helper inventory pattern is outside exact Copy metadata"
                }
                Reason::RecordPatternHasNoFieldInventory => "record pattern has no field inventory",
                Reason::RecordPatternFieldInventoryIsIncomplete => {
                    "record pattern field inventory is incomplete"
                }
                Reason::NestedRecordPatternHasNoFieldInventory => {
                    "nested record pattern has no field inventory"
                }
                Reason::NestedRecordPatternFieldInventoryIsIncomplete => {
                    "nested record pattern field inventory is incomplete"
                }
                Reason::SelectedInventoryCloneBackingOverflow => {
                    "selected inventory clone backing overflow"
                }
                Reason::SelectedInventoryHasNoPrimitiveLifecycle => {
                    "selected inventory has no primitive lifecycle"
                }
                Reason::TooManyCleanupLivenessFlags => "too many cleanup liveness flags",
                Reason::SelectedInventoryExceedsOwnedLeafLimit => {
                    "selected inventory exceeds owned-leaf limit"
                }
                Reason::SelectedCollectionProbePrepaymentRefused => {
                    "selected collection probe prepayment refused"
                }
                Reason::SelectedInventoryTypeHasUnsupportedArguments => {
                    "selected inventory type has unsupported arguments"
                }
                Reason::SelectedInventoryKeyLengthOverflow => {
                    "selected inventory key length overflow"
                }
            },
        }
    }
    pub(crate) fn prefixed(self) -> bool {
        match self.0 {
            Kind::Work => true,
            Kind::Profile(reason) => reason.prefixed(),
            _ => false,
        }
    }
}
