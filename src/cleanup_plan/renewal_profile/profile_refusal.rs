//! Closed finite labels from the exact selected Context child inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProfileRefusal {
    SelectedVecCalleeWorkOverflow,
    SelectedVecCallRequiresASeparatelyAuthenticatedOperation,
    SelectedVecCallHasInvalidTypeArguments,
    SelectedVecSourceIsAbsent,
    SelectedVecElementAndRootDisagree,
    SelectedVecOperationOrArityIsOutsideTheExactElementFamily,
    SelectedVecArgumentTypeOrModeDisagrees,
    SelectedVecResultTypeOrModeDisagrees,
    SelectedVecFieldReadHasInvalidArity,
    SelectedFieldKeyOverflow,
    SelectedVecFieldElementIsNotCurrentFlatRecord,
    SelectedVecFieldIdentityIsForeign,
    SelectedVecFieldModeIsUnsupported,
    SelectedVecFieldRootIndexResultTypeOrModeDisagrees,
    SelectedAppendLostReservation,
    SelectedCollectionElementIsNotABoundedLeaf,
    SelectedTypeHasUnsupportedArgumentCount,
    SelectedTypeKeyLengthOverflow,
    SelectedRowLoanElementIsNotACurrentFlatRecord,
    SelectedRowLoanHasInvalidArity,
    SelectedRowLoanCarrierIndexOrBorrowedResultDisagrees,
    SelectedStringTransferRequiresOneNamedOwnedString,
    SelectedTypeInventoryIsIncomplete,
    RecordClassificationWorkOverflowed,
    SelectedConstructionHasNoExactDirectRecord,
    SelectedConstructionFieldIdentityOrderTypeOrOwnershipDisagrees,
    SelectedProjectionHasNoExactDirectRecord,
    SelectedProjectionFieldIsForeign,
    SelectedProjectionTypeDisagreesWithCurrentField,
    SelectedVecHasInvalidTypeArgumentCount,
    SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily,
    SelectedVecCurrentDirectoryExceedsItsBoundedTranche,
    SelectedVecCurrentDirectoryIsIncomplete,
    SelectedVecCurrentDirectoryRepeatsItsIdentity,
    SelectedVecDeclarationIsAbsentFromCurrentTypes,
    SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection,
    SelectedCollectionProbePrepaymentRefused,
    SelectedFlatRecordArgumentsAreUnsupported,
    SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields,
    LexicalLoopLatchDiffersFromItsIndependentlyAuthenticatedLowering,
    LexicalFunctionOwnerDiffersFromCanonicalResultBodyIdentities,
    StaticUtf8LiteralExceeds65536DecodedBytes,
}
impl ProfileRefusal {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::SelectedVecCalleeWorkOverflow => "selected Vec callee work overflow",
            Self::SelectedVecCallRequiresASeparatelyAuthenticatedOperation => {
                "selected Vec call requires a separately authenticated operation"
            }
            Self::SelectedVecCallHasInvalidTypeArguments => {
                "selected Vec call has invalid type arguments"
            }
            Self::SelectedVecSourceIsAbsent => "selected Vec source is absent",
            Self::SelectedVecElementAndRootDisagree => "selected Vec element and root disagree",
            Self::SelectedVecOperationOrArityIsOutsideTheExactElementFamily => {
                "selected Vec operation or arity is outside the exact element family"
            }
            Self::SelectedVecArgumentTypeOrModeDisagrees => {
                "selected Vec argument type or mode disagrees"
            }
            Self::SelectedVecResultTypeOrModeDisagrees => {
                "selected Vec result type or mode disagrees"
            }
            Self::SelectedVecFieldReadHasInvalidArity => {
                "selected Vec field read has invalid arity"
            }
            Self::SelectedFieldKeyOverflow => "selected field key overflow",
            Self::SelectedVecFieldElementIsNotCurrentFlatRecord => {
                "selected Vec field element is not current flat record"
            }
            Self::SelectedVecFieldIdentityIsForeign => "selected Vec field identity is foreign",
            Self::SelectedVecFieldModeIsUnsupported => "selected Vec field mode is unsupported",
            Self::SelectedVecFieldRootIndexResultTypeOrModeDisagrees => {
                "selected Vec field root, index, result type or mode disagrees"
            }
            Self::SelectedAppendLostReservation => "selected append lost reservation",
            Self::SelectedCollectionElementIsNotABoundedLeaf => {
                "selected collection element is not a bounded leaf"
            }
            Self::SelectedTypeHasUnsupportedArgumentCount => {
                "selected type has unsupported argument count"
            }
            Self::SelectedTypeKeyLengthOverflow => "selected type key length overflow",
            Self::SelectedRowLoanElementIsNotACurrentFlatRecord => {
                "selected row loan element is not a current flat record"
            }
            Self::SelectedRowLoanHasInvalidArity => "selected row loan has invalid arity",
            Self::SelectedRowLoanCarrierIndexOrBorrowedResultDisagrees => {
                "selected row loan carrier, index or borrowed result disagrees"
            }
            Self::SelectedStringTransferRequiresOneNamedOwnedString => {
                "selected String transfer requires one named owned String"
            }
            Self::SelectedTypeInventoryIsIncomplete => "selected type inventory is incomplete",
            Self::RecordClassificationWorkOverflowed => "record classification work overflowed",
            Self::SelectedConstructionHasNoExactDirectRecord => {
                "selected construction has no exact direct record"
            }
            Self::SelectedConstructionFieldIdentityOrderTypeOrOwnershipDisagrees => {
                "selected construction field identity, order, type or ownership disagrees"
            }
            Self::SelectedProjectionHasNoExactDirectRecord => {
                "selected projection has no exact direct record"
            }
            Self::SelectedProjectionFieldIsForeign => "selected projection field is foreign",
            Self::SelectedProjectionTypeDisagreesWithCurrentField => {
                "selected projection type disagrees with current field"
            }
            Self::SelectedVecHasInvalidTypeArgumentCount => {
                "selected Vec has invalid type argument count"
            }
            Self::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily => {
                "selected Vec element is outside the current bounded flat family"
            }
            Self::SelectedVecCurrentDirectoryExceedsItsBoundedTranche => {
                "selected Vec current directory exceeds its bounded tranche"
            }
            Self::SelectedVecCurrentDirectoryIsIncomplete => {
                "selected Vec current directory is incomplete"
            }
            Self::SelectedVecCurrentDirectoryRepeatsItsIdentity => {
                "selected Vec current directory repeats its identity"
            }
            Self::SelectedVecDeclarationIsAbsentFromCurrentTypes => {
                "selected Vec declaration is absent from current types"
            }
            Self::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection => {
                "selected Vec declaration is not the exact compiler-owned collection"
            }
            Self::SelectedCollectionProbePrepaymentRefused => {
                "selected collection probe prepayment refused"
            }
            Self::SelectedFlatRecordArgumentsAreUnsupported => {
                "selected flat-record arguments are unsupported"
            }
            Self::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields => {
                "selected flat-record inventory has unauthenticated or unsupported fields"
            }
            Self::LexicalLoopLatchDiffersFromItsIndependentlyAuthenticatedLowering => {
                "lexical loop latch differs from its independently authenticated lowering"
            }
            Self::LexicalFunctionOwnerDiffersFromCanonicalResultBodyIdentities => {
                "lexical function owner differs from canonical result/body identities"
            }
            Self::StaticUtf8LiteralExceeds65536DecodedBytes => {
                "static UTF-8 literal exceeds 65536 decoded bytes"
            }
        }
    }
    pub(crate) fn prefixed(self) -> bool {
        match self {
            Self::SelectedVecCalleeWorkOverflow => true,
            Self::SelectedVecCallRequiresASeparatelyAuthenticatedOperation => true,
            Self::SelectedVecCallHasInvalidTypeArguments => true,
            Self::SelectedVecSourceIsAbsent => true,
            Self::SelectedVecElementAndRootDisagree => true,
            Self::SelectedVecOperationOrArityIsOutsideTheExactElementFamily => true,
            Self::SelectedVecArgumentTypeOrModeDisagrees => true,
            Self::SelectedVecResultTypeOrModeDisagrees => true,
            Self::SelectedVecFieldReadHasInvalidArity => true,
            Self::SelectedFieldKeyOverflow => true,
            Self::SelectedVecFieldElementIsNotCurrentFlatRecord => true,
            Self::SelectedVecFieldIdentityIsForeign => true,
            Self::SelectedVecFieldModeIsUnsupported => true,
            Self::SelectedVecFieldRootIndexResultTypeOrModeDisagrees => true,
            Self::SelectedAppendLostReservation => true,
            Self::SelectedCollectionElementIsNotABoundedLeaf => false,
            Self::SelectedTypeHasUnsupportedArgumentCount => false,
            Self::SelectedTypeKeyLengthOverflow => false,
            Self::SelectedRowLoanElementIsNotACurrentFlatRecord => true,
            Self::SelectedRowLoanHasInvalidArity => true,
            Self::SelectedRowLoanCarrierIndexOrBorrowedResultDisagrees => true,
            Self::SelectedStringTransferRequiresOneNamedOwnedString => true,
            Self::SelectedTypeInventoryIsIncomplete => true,
            Self::RecordClassificationWorkOverflowed => true,
            Self::SelectedConstructionHasNoExactDirectRecord => true,
            Self::SelectedConstructionFieldIdentityOrderTypeOrOwnershipDisagrees => true,
            Self::SelectedProjectionHasNoExactDirectRecord => true,
            Self::SelectedProjectionFieldIsForeign => true,
            Self::SelectedProjectionTypeDisagreesWithCurrentField => true,
            Self::SelectedVecHasInvalidTypeArgumentCount => false,
            Self::SelectedVecElementIsOutsideTheCurrentBoundedFlatFamily => false,
            Self::SelectedVecCurrentDirectoryExceedsItsBoundedTranche => false,
            Self::SelectedVecCurrentDirectoryIsIncomplete => false,
            Self::SelectedVecCurrentDirectoryRepeatsItsIdentity => false,
            Self::SelectedVecDeclarationIsAbsentFromCurrentTypes => false,
            Self::SelectedVecDeclarationIsNotTheExactCompilerOwnedCollection => false,
            Self::SelectedCollectionProbePrepaymentRefused => false,
            Self::SelectedFlatRecordArgumentsAreUnsupported => false,
            Self::SelectedFlatRecordInventoryHasUnauthenticatedOrUnsupportedFields => false,
            Self::LexicalLoopLatchDiffersFromItsIndependentlyAuthenticatedLowering => false,
            Self::LexicalFunctionOwnerDiffersFromCanonicalResultBodyIdentities => false,
            Self::StaticUtf8LiteralExceeds65536DecodedBytes => false,
        }
    }
}
