//! Complete old reference validator against closed paid refusal data.
use super::{program, validate_reference_coverage};
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::cleanup_plan::replay::selected_context::child::{
    ArrayReceiving, Deferred, NodeReceiving, Permit,
};
use crate::cleanup_plan::replay::{
    expression_facts, selected_context, semantic_refusal, ReplayBudget,
};
use crate::cleanup_plan::ExitContinuation;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedFunction;
use crate::test_allocation_observer::measure;
use std::mem::size_of;
type Error = expression_facts::errors::EngineError;
fn current(f: &ResolvedFunction, b: &mut ReplayBudget) -> Result<(), Error> {
    super::super::paid_reference::validate(f, b)
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn permit_bytes() -> usize {
    size_of::<(
        Deferred,
        Option<Deferred>,
        Result<(), Deferred>,
        Result<(), Deferred>,
    )>() + size_of::<(
        Permit<'static>,
        Refusal,
        Option<Refusal>,
        semantic_refusal::Receipt,
        Error,
        Option<Error>,
        Result<(), Refusal>,
        Result<(), Error>,
        Result<Permit<'static>, Error>,
        Result<selected_context::Context<'static>, Error>,
        Result<(), Error>,
    )>()
}
#[test]
fn complete_reference_oracle_keeps_all_three_refusals_and_first_error_order() {
    let p = program();
    let f = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "structural.main")
        .unwrap();
    assert!(!f.cleanup_plan.status_sources.is_empty());
    for mask in 0..8 {
        let mut changed = f.clone();
        if mask & 1 != 0 {
            changed
                .cleanup_plan
                .edges
                .push(changed.cleanup_plan.edges[0].clone());
        }
        if mask & 2 != 0 {
            let mut extra = changed.cleanup_plan.exits[0].clone();
            extra.continuation = ExitContinuation::ReturnUnit;
            changed.cleanup_plan.exits.push(extra);
        }
        if mask & 4 != 0 {
            changed.cleanup_plan.status_sources.pop().unwrap();
        }
        let old = validate_reference_coverage(&changed, &mut ReplayBudget::new());
        let mut budget = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
        match (old, current(&changed, &mut budget)) {
            (Ok(()), Ok(())) => assert_eq!(mask, 0),
            (Err(old), Err(Error::Structural(receipt))) => {
                assert_eq!(receipt.stage(), "selected child");
                same(&old, &receipt.legacy(&changed));
                let detail = if mask & 1 != 0 {
                    "each cleanup edge must be referenced by exactly one control-flow owner"
                } else if mask & 2 != 0 {
                    "each cleanup exit must be referenced by exactly one block"
                } else {
                    "declared and referenced cleanup status sources differ"
                };
                assert!(old.message.ends_with(detail));
            }
            pair => panic!("old/current complete reference verdict differs: {pair:?}"),
        }
    }
}
#[test]
fn reference_per_actual_call_banks_exact_short_zero_duplicates_and_whole_drop() {
    let p = program();
    let f = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "structural.main")
        .unwrap();
    let count = f.cleanup_plan.status_sources.len();
    let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
    let setup = old.physical.as_deref().unwrap().charged();
    let initial_work = old.remaining;
    validate_reference_coverage(f, &mut old).unwrap();
    let backing = old.physical.as_deref().unwrap().charged() - setup;
    let extra = permit_bytes()
        + 2 * size_of::<ArrayReceiving<'static>>()
        + 2 * count * size_of::<NodeReceiving<'static>>();
    let work = 15 + 2 * 9 + 2 * count * 8;
    // The original two backing vectors and two unique status sets are unchanged.
    // A repeated declaration performs only the old membership lookup.
    let mut duplicate = f.clone();
    duplicate
        .cleanup_plan
        .status_sources
        .push(duplicate.cleanup_plan.status_sources[0].clone());
    for subject in [f, &duplicate] {
        let mut b = ReplayBudget::with_physical(subject, usize::MAX).unwrap();
        let own_setup = b.physical.as_deref().unwrap().charged();
        current(subject, &mut b).unwrap();
        assert_eq!(
            b.physical.as_deref().unwrap().charged() - own_setup,
            backing + extra
        );
        assert_eq!(initial_work - b.remaining, work);
    }
    let exact = setup + backing + extra;
    for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
        let ((), seen) = measure(|| {
            let mut b = ReplayBudget::with_physical(f, limit).unwrap();
            let ledger = b.physical.as_deref().unwrap() as *const _;
            let text = b.refusal_text.as_ref().unwrap().as_ptr();
            let result = current(f, &mut b);
            assert_eq!(result.is_ok(), accepted);
            if let Err(Error::Owned(error)) = &result {
                assert_eq!(error.code, "SPX-H006");
                assert_eq!(error.message.as_ptr(), text);
                let physical = b.physical.as_deref_mut().unwrap();
                let first = physical.reserve(0).unwrap_err();
                assert_eq!(physical.reserve(1).unwrap_err(), first);
            } else {
                assert!(accepted);
            }
            assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
            if accepted {
                assert_eq!(b.physical.as_deref().unwrap().charged(), exact);
            }
            drop(result);
            drop(b);
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
    }
    for remaining in [0, 14, work - 1, work] {
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        b.remaining = remaining;
        assert_eq!(current(f, &mut b).is_ok(), remaining == work);
    }
}
