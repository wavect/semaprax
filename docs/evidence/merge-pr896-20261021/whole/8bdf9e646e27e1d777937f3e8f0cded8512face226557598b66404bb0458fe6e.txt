//! Source-derived forest counts; no observed candidate delta supplies the floor.
use super::*;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::cleanup_plan::replay::selected_context::child::Permit;
use crate::test_allocation_observer::measure;
use std::mem::size_of;
type Caller<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<bool, EngineError>,
);
type Formal<'a> = (
    &'a dyn crate::cleanup::ReplayContext,
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<bool, EngineError>,
);
type Wrapper<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, EngineError>,
);
type WrapperFormal<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, EngineError>,
);
type Family<'a> = (
    &'a Permit<'a>,
    super::Reason,
    &'a ResolvedFunction,
    super::Reason,
    crate::hir::ExpressionId,
    super::super::super::semantic_refusal::Atom,
    super::super::super::semantic_refusal::Receipt,
    EngineError,
    Result<Permit<'a>, EngineError>,
);
type Acquire<'a> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    super::super::super::semantic_refusal::Receipt,
    EngineError,
    Option<EngineError>,
    Result<bool, Refusal>,
    Result<bool, EngineError>,
    Result<Permit<'a>, EngineError>,
    Result<super::super::super::selected_context::Context<'a>, EngineError>,
    Result<(), EngineError>,
);
type Entry<'a> = (
    usize,
    Option<(&'a ResolvedExpr, usize)>,
    &'a ResolvedExpr,
    usize,
);
type Pop<'a> = (
    &'a mut Option<(&'a ResolvedExpr, usize)>,
    &'a mut Option<(&'a ResolvedExpr, usize)>,
    Option<(&'a ResolvedExpr, usize)>,
    (&'a ResolvedExpr, usize),
    &'a ResolvedExpr,
    usize,
);
#[test]
fn owner_zero_one_many_contract_roots_pay_exact_reached_pops_short_zero_sticky_and_drop() {
    for source in [
        "module owner.pop; @id(\"main\") fn main()->i64 {0}",
        "module owner.pop; @id(\"main\") fn main()->i64 requires true {0}",
        "module owner.pop; @id(\"main\") fn main()->i64 requires true ensures result>=0 {let a=1;let b=a+2;b}",
    ] {
        let program = crate::hir::resolve(&crate::check(source, "owner-pop.spx").unwrap()).unwrap();
        let function = program.functions.iter().find(|f| f.id.as_str() == "main").unwrap();
        let expected = super::super::function_has_owner_admission(&program, function).unwrap();
        assert!(!expected, "pure Copy forest does not require owner admission");
        let mut nodes = 0usize;
        crate::hir::function_value::walk(function, |_| nodes += 1);
        let roots = function.requires.len() + function.ensures.len() + 1;
        // Each tree node is popped once plus once for each outgoing child.
        // Sum(children)=nodes-roots; the counted loop performs no terminal pop.
        let pops = 2 * nodes - roots;
        let request = size_of::<Caller<'_>>() + size_of::<Formal<'_>>()
            + 2 * size_of::<Wrapper<'_>>() + size_of::<WrapperFormal<'_>>() + size_of::<Family<'_>>() + size_of::<&crate::hir::ExpressionId>()
            + size_of::<Acquire<'_>>()
            + size_of::<[Option<(&ResolvedExpr, usize)>; 514]>()
            + size_of::<Entry<'_>>() + pops * size_of::<Pop<'_>>();
        // Caller4 + formal4 + family30 + retained entry5 + reached pop6 each.
        let work = 4 + 4 + 30 + 5 + 6 * pops;
        let initial = ReplayBudget::with_physical(function, usize::MAX).unwrap();
        let setup = initial.physical.as_ref().unwrap().charged();
        drop(initial);
        for (bytes, available, success) in [
            (setup + request, work, true),
            (setup + request - 1, work, false),
            (setup + request, work - 1, false),
            (setup + request, 0, false),
            (setup, work, false),
        ] {
            let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
            budget.remaining = available;
            let (result, observed) = measure(|| {
                let outcome = (|| {
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(budget, function, Formal<'_>, 4, "selected owner formal caller frontier", ());
                    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(budget, function, Caller<'_>, 4, "selected owner admission call receiving", super::owner_admission(&program, function, &mut budget))
                })();
                let success = outcome.is_ok();
                if let Ok(actual) = outcome.as_ref() { assert_eq!(*actual, expected); }
                drop(outcome);
                success
            });
            assert_eq!(result, success, "roots={roots} nodes={nodes} pops={pops}");
            if success {
                assert_eq!(budget.remaining, 0);
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
            } else {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
