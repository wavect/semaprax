use super::*;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use std::mem::size_of;
fn empty() -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: Vec::new(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    }
}
type Caller<'a> = (
    &'a ResolvedFunction,
    &'a mut PathState,
    Vec<LivenessFlagId>,
    Kind,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
type Body<'a> = (
    &'a LivenessFlagId,
    &'a ReplayConditionalVariant,
    &'a Vec<LivenessFlagId>,
    std::slice::Iter<'a, LivenessFlagId>,
    std::slice::Iter<'a, ReplayConditionalVariant>,
);
type CallPermit<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, Error>,
);
type WrapperFormal<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, Error>,
);
type Control<'a> = (
    &'a Permit<'a>,
    structural_control::selected::reason::Reason,
    &'a ResolvedFunction,
    structural_control::selected::reason::Reason,
    &'a ExpressionId,
    ExpressionId,
    &'a ExpressionId,
    ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);
type Acquire<'a> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    semantic_refusal::Receipt,
    Error,
    Option<Error>,
    Result<(), Refusal>,
    Result<(), Error>,
    Result<Permit<'a>, Error>,
    Result<selected_context::Context<'a>, Error>,
    Result<(), Error>,
);
#[test]
fn empty_append_exact_typed_floor_short_zero_work_source_and_sticky_refusal_before_formals() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.bytes")
        .unwrap();
    let mut old_state = empty();
    let expected = crate::cleanup_plan::replay::transition::append_dead_flags(
        function,
        &mut old_state,
        Vec::new(),
        "entry state",
    )
    .unwrap_err();
    let mut initial = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let setup = initial.physical.as_ref().unwrap().charged();
    // Literal caller6, body5, callee6, permit caller3, control12,
    // acquire caller3 and original acquire11. No observed candidate delta.
    let request = 2 * size_of::<Caller<'_>>()
        + size_of::<Body<'_>>()
        + 2 * size_of::<CallPermit<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<Acquire<'_>>();
    const WORK: usize = 6 + 5 + 6 + 3 + 12 + 3 + 11;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let mut state = empty();
        let (result, observed) = measure(|| {
            receive!(
                budget,
                function,
                Caller<'_>,
                6,
                "selected test append caller receiving",
                append_before_formals::append_flags_before_formals(
                    function,
                    &mut state,
                    Vec::new(),
                    Kind::EntryStateInitializesAnEmptyOrLiveCleanupPlace,
                    &mut budget
                )
            )
        });
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        assert_eq!(state, old_state);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(receipt.legacy(function), expected);
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
                let remaining = budget.remaining;
                assert!(budget.charge(function, 0, "later").is_err());
                assert_eq!(budget.remaining, remaining);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
    // This setup probe itself is unused and held only for the inherited floor.
    assert!(initial.charge(function, 0, "unchanged setup").is_ok());
}

#[test]
fn empty_append_exact_typed_floor_short_zero_work_source_and_sticky_refusal() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.bytes")
        .unwrap();
    let mut old_state = empty();
    let expected = crate::cleanup_plan::replay::transition::append_dead_flags(
        function,
        &mut old_state,
        Vec::new(),
        "entry state",
    )
    .unwrap_err();
    let mut initial = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let setup = initial.physical.as_ref().unwrap().charged();
    // Literal caller6, body5, callee6, permit caller3, control12,
    // acquire caller3 and original acquire11. No observed candidate delta.
    let request = 2 * size_of::<Caller<'_>>()
        + size_of::<Body<'_>>()
        + 2 * size_of::<CallPermit<'_>>()
        + size_of::<WrapperFormal<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<Acquire<'_>>();
    const WORK: usize = 6 + 5 + 6 + 3 + 3 + 12 + 3 + 11;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let mut state = empty();
        let (result, observed) = measure(|| {
            receive!(
                budget,
                function,
                Caller<'_>,
                6,
                "selected test append caller receiving",
                finish::append_flags(
                    function,
                    &mut state,
                    Vec::new(),
                    Kind::EntryStateInitializesAnEmptyOrLiveCleanupPlace,
                    &mut budget
                )
            )
        });
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        assert_eq!(state, old_state);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(receipt.legacy(function), expected);
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
                let remaining = budget.remaining;
                assert!(budget.charge(function, 0, "later").is_err());
                assert_eq!(budget.remaining, remaining);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
    // This setup probe itself is unused and held only for the inherited floor.
    assert!(initial.charge(function, 0, "unchanged setup").is_ok());
}
