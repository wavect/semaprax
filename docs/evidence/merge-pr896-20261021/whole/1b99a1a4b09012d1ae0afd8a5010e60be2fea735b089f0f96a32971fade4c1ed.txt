//! Complete old enums preserve real layout; no padding or current-Reason alias.
use super::*;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
#[path = "reason_before_path.rs"]
mod reason_before_path;
#[derive(Debug)]
enum AtomBefore {
    SelectedCall(structural_control::call_receiving::Kind),
    UnknownCallCallee(DeclarationId),
    StatusUnknownCallee(DeclarationId),
    SelectedSession(&'static str),
    SelectedFamily(selected_context::family::Reason),
    SelectedControl(reason_before_path::Reason),
    SelectedChild(crate::cleanup_plan::renewal_profile::child_refusal::Refusal),
    RepeatedStatus(ExpressionId),
    UnknownStatus(ExpressionId),
    NoncontiguousSlot(usize),
    RepeatedStorage(StorageId),
    DeadCallFlag(ExpressionId, LivenessFlagId),
    RepeatedCallFlag(ExpressionId, LivenessFlagId),
    RepeatedVariantEpoch(ExpressionId),
    MismatchedFailure(ExpressionId),
}

#[derive(Debug)]
struct ReceiptBefore {
    source: ExpressionId,
    atom: AtomBefore,
}
#[derive(Debug)]
enum ErrorBefore {
    Failed,
    Owned(Diagnostic),
    Fact(expression_facts::errors::FactReceipt),
    Structural(ReceiptBefore),
}
type ControlBefore<'a> = (
    &'a Permit<'a>,
    reason_before_path::Reason,
    &'a ResolvedFunction,
    reason_before_path::Reason,
    &'a ExpressionId,
    ExpressionId,
    &'a ExpressionId,
    ExpressionId,
    AtomBefore,
    ReceiptBefore,
    ErrorBefore,
    Result<Permit<'a>, ErrorBefore>,
);
type AcquireBefore<'a, T> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    ReceiptBefore,
    ErrorBefore,
    Option<ErrorBefore>,
    Result<T, Refusal>,
    Result<T, ErrorBefore>,
    Result<Permit<'a>, ErrorBefore>,
    Result<selected_context::Context<'a>, ErrorBefore>,
    Result<(), ErrorBefore>,
);
type AcquireCurrent<'a, T> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    semantic_refusal::Receipt,
    Error,
    Option<Error>,
    Result<T, Refusal>,
    Result<T, Error>,
    Result<Permit<'a>, Error>,
    Result<selected_context::Context<'a>, Error>,
    Result<(), Error>,
);
#[test]
fn nested_path_reason_preserves_complete_original_control_layouts() {
    use std::mem::{align_of, size_of};
    macro_rules! same {
        ($old:ty,$current:ty) => {{
            assert_eq!(
                (size_of::<$old>(), align_of::<$old>()),
                (size_of::<$current>(), align_of::<$current>()),
                "whole original carrier differs: {} -> {}",
                stringify!($old),
                stringify!($current)
            );
        }};
    }
    same!(
        reason_before_path::Reason,
        structural_control::selected::reason::Reason
    );
    same!(AtomBefore, semantic_refusal::Atom);
    same!(ReceiptBefore, semantic_refusal::Receipt);
    same!(ErrorBefore, Error);
    same!(Result<(),ErrorBefore>,Result<(),Error>);
    same!(
        ControlBefore<'_>,
        selected_context::child::ControlReceiving<'_>
    );
    same!(AcquireBefore<'_, ()>, AcquireCurrent<'_, ()>);
    same!(AcquireBefore<'_, PathState>, AcquireCurrent<'_, PathState>);
    same!(
        AcquireBefore<'_, Vec<LivenessFlagId>>,
        AcquireCurrent<'_, Vec<LivenessFlagId>>
    );
    same!(
        AcquireBefore<'_, Option<Vec<LivenessFlagId>>>,
        AcquireCurrent<'_, Option<Vec<LivenessFlagId>>>
    );
}
