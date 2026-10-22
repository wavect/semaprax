//! Selected schema/inventory refusal data. No source, ledger or admission authority.
use super::super::{replay_error, Diagnostic, ResolvedFunction};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) enum Reason {
    SelectedInventoryCoverageRequiresAPhysicalLedger,
    CleanupInventoryDisagreesWithIndependentlyRebuiltTypedHir,
    CleanupPlanOmitsStorageRequiredByTheCleanupInventory,
    SelectedInventoryCoverageDoesNotSupportConditionalEntryOwners,
    CleanupPlanEntryOwnershipDisagreesWithTheIndependentInventory,
    CleanupInventoryEntryReferencesUnknownStorage,
    TooManyInventoryLivenessFlags,
    CleanupPlanHasMissingOrExtraCallArgumentStorage,
    TooManyCleanupSlots,
    SelectedParameterReceiverHasNoCall,
    SelectedVecParameterLacksExactElement,
    SelectedSupplementalCallHasNoAuthenticatedBody,
    SupplementalExpressionCursorCapacityDisagreesWithReservation,
    SelectedSupplementalCallDoesNotSupportGenericInstances,
    SelectedSupplementalCallHasInconsistentArity,
    SupplementalSlotTraversalExceedsTheAdmittedDepth,
    SelectedSupplementalInventoryDoesNotSupportDelegatedCalls,
    SupplementalContinuationLacksACall,
    SupplementalCallParameterIsMissing,
    SupplementalOwnedArgumentTypeDisagreesWithItsParameter,
    TooManyCallParameters,
    SelectedFieldHasNoPrimitiveLifecycle,
    SelectedFieldFlagsExceedTheOwnedLeafLimit,
    SelectedFieldFlagsOverflowed,
    BaseSlot(usize),
    SupplementalSlot(usize),
    OwnerAdmissionDepth,
}
impl Reason {
    /// Allocating compatibility observation only, after leaving the paid route.
    pub(in crate::cleanup_plan::replay) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        match self {
            Self::SelectedInventoryCoverageRequiresAPhysicalLedger => replay_error(
                function,
                "selected inventory coverage requires a physical ledger",
            ),
            Self::CleanupInventoryDisagreesWithIndependentlyRebuiltTypedHir => replay_error(
                function,
                "cleanup inventory disagrees with independently rebuilt typed HIR",
            ),
            Self::CleanupPlanOmitsStorageRequiredByTheCleanupInventory => replay_error(
                function,
                "cleanup plan omits storage required by the cleanup inventory",
            ),
            Self::SelectedInventoryCoverageDoesNotSupportConditionalEntryOwners => replay_error(
                function,
                "selected inventory coverage does not support conditional entry owners",
            ),
            Self::CleanupPlanEntryOwnershipDisagreesWithTheIndependentInventory => replay_error(
                function,
                "cleanup plan entry ownership disagrees with the independent inventory",
            ),
            Self::CleanupInventoryEntryReferencesUnknownStorage => replay_error(
                function,
                "cleanup inventory entry references unknown storage",
            ),
            Self::TooManyInventoryLivenessFlags => {
                replay_error(function, "too many inventory liveness flags")
            }
            Self::CleanupPlanHasMissingOrExtraCallArgumentStorage => replay_error(
                function,
                "cleanup plan has missing or extra call-argument storage",
            ),
            Self::TooManyCleanupSlots => replay_error(function, "too many cleanup slots"),
            Self::SelectedParameterReceiverHasNoCall => {
                replay_error(function, "selected parameter receiver has no call")
            }
            Self::SelectedVecParameterLacksExactElement => {
                replay_error(function, "selected Vec parameter lacks exact element")
            }
            Self::SelectedSupplementalCallHasNoAuthenticatedBody => replay_error(
                function,
                "selected supplemental call has no authenticated body",
            ),
            Self::SupplementalExpressionCursorCapacityDisagreesWithReservation => replay_error(
                function,
                "supplemental expression cursor capacity disagrees with reservation",
            ),
            Self::SelectedSupplementalCallDoesNotSupportGenericInstances => replay_error(
                function,
                "selected supplemental call does not support generic instances",
            ),
            Self::SelectedSupplementalCallHasInconsistentArity => replay_error(
                function,
                "selected supplemental call has inconsistent arity",
            ),
            Self::SupplementalSlotTraversalExceedsTheAdmittedDepth => replay_error(
                function,
                "supplemental-slot traversal exceeds the admitted depth",
            ),
            Self::SelectedSupplementalInventoryDoesNotSupportDelegatedCalls => replay_error(
                function,
                "selected supplemental inventory does not support delegated calls",
            ),
            Self::SupplementalContinuationLacksACall => {
                replay_error(function, "supplemental continuation lacks a call")
            }
            Self::SupplementalCallParameterIsMissing => {
                replay_error(function, "supplemental call parameter is missing")
            }
            Self::SupplementalOwnedArgumentTypeDisagreesWithItsParameter => replay_error(
                function,
                "supplemental owned argument type disagrees with its parameter",
            ),
            Self::TooManyCallParameters => replay_error(function, "too many call parameters"),
            Self::SelectedFieldHasNoPrimitiveLifecycle => {
                replay_error(function, "selected field has no primitive lifecycle")
            }
            Self::SelectedFieldFlagsExceedTheOwnedLeafLimit => {
                replay_error(function, "selected field flags exceed the owned-leaf limit")
            }
            Self::SelectedFieldFlagsOverflowed => {
                replay_error(function, "selected field flags overflowed")
            }
            Self::BaseSlot(index) => replay_error(
                function,
                format_args!(
                    "cleanup plan base slot {index} disagrees with the independent inventory"
                ),
            ),
            Self::SupplementalSlot(offset) => replay_error(
                function,
                format_args!("supplemental call-argument slot {offset} disagrees with typed HIR"),
            ),
            Self::OwnerAdmissionDepth => {
                replay_error(function, "owner admission depth exceeds 512")
            }
        }
    }
}
