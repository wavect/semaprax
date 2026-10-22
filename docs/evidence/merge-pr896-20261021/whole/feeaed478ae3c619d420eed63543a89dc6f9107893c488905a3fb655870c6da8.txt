use super::*;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::cleanup_plan::replay::selected_context::child::Permit;
use std::mem::size_of;
type Caller<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
type Local<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    BTreeSet<LivenessFlagId>,
    &'a CleanupPlace,
    LivenessFlagId,
    std::vec::IntoIter<LivenessFlagId>,
    std::slice::Iter<'a, CleanupPlace>,
);
type PermitCall<'a> = (
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
    Reason,
    &'a ResolvedFunction,
    Reason,
    &'a ExpressionId,
    ExpressionId,
    &'a ExpressionId,
    ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);
type OriginalAcquire<'a> = (
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
fn entry(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Error> {
    let storage = BTreeSet::new();
    let leaves = Leaves::new(BTreeMap::new());
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        Caller<'_>,
        5,
        "selected structural driver call receiving",
        super::super::entry_before_dynamic::validate_entry_before_dynamic(
            function, &storage, &leaves, budget
        )
    )
}
fn before_formals_entry(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let storage = BTreeSet::new();
    let leaves = Leaves::new(BTreeMap::new());
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        Caller<'_>,
        5,
        "selected structural driver call receiving",
        super::super::entry_before_formals::validate_entry_before_formals(
            function, &storage, &leaves, budget
        )
    )
}
#[test]
fn unknown_entry_exact_typed_frontier_and_one_short_keep_work_physical_and_no_allocation() {
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    function.cleanup_plan.entry = BlockId(u32::MAX);
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<Caller<'_>>()
        + size_of::<Local<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<OriginalAcquire<'_>>();
    assert_eq!(
        size_of::<Control<'_>>(),
        size_of::<selected_context::child::ControlReceiving<'_>>()
    );
    // 5 caller +9 local +3 permit caller +12 control +3 acquire caller +11 original.
    const WORK: usize = 43;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| entry(function, &mut budget));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert!(matches!(
                    receipt.borrowed_control_receipt().unwrap().1,
                    Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
                ));
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
}

fn current_entry(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Error> {
    let storage = BTreeSet::new();
    let leaves = Leaves::new(BTreeMap::new());
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        Caller<'_>,
        5,
        "selected structural driver call receiving",
        validate_entry(function, &storage, &leaves, budget)
    )
}
#[test]
fn current_unknown_entry_fresh_get_exact_short_zero_sticky_matches_complete_old_owner_before_formals(
) {
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    function.cleanup_plan.entry = BlockId(u32::MAX);
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<(
        &[crate::cleanup_plan::CleanupBlock],
        usize,
        Option<&crate::cleanup_plan::CleanupBlock>,
    )>() + size_of::<Caller<'_>>()
        + size_of::<Local<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<OriginalAcquire<'_>>();
    assert_eq!(
        size_of::<Control<'_>>(),
        size_of::<selected_context::child::ControlReceiving<'_>>()
    );
    // Original43 roles remain; current fresh get adds receiver/index/Option3.
    const WORK: usize = 46;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| before_formals_entry(function, &mut budget));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert!(matches!(
                    receipt.borrowed_control_receipt().unwrap().1,
                    Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
                ));
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
}

#[test]
fn current_unknown_entry_fresh_get_exact_short_zero_sticky_matches_complete_old_owner() {
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    function.cleanup_plan.entry = BlockId(u32::MAX);
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<(
        &[crate::cleanup_plan::CleanupBlock],
        usize,
        Option<&crate::cleanup_plan::CleanupBlock>,
    )>() + size_of::<Caller<'_>>()
        + size_of::<Local<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<WrapperFormal<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<OriginalAcquire<'_>>();
    assert_eq!(
        size_of::<Control<'_>>(),
        size_of::<selected_context::child::ControlReceiving<'_>>()
    );
    // Original43 roles remain; fresh get3 and wrapper formal3 are independent.
    const WORK: usize = 49;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| current_entry(function, &mut budget));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert!(matches!(
                    receipt.borrowed_control_receipt().unwrap().1,
                    Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
                ));
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
}

type PlaceCaller<'a> = (
    &'a ResolvedFunction,
    &'a CleanupPlace,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<Vec<LivenessFlagId>, Error>,
);
// Independently name the concrete two-scan frame; no anonymous closure
// layout or reference-width constant stands in for an iterator header.
type PlaceLocal<'a> = (
    &'a ResolvedFunction,
    &'a CleanupPlace,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    std::collections::btree_map::Values<'a, LivenessFlagId, crate::cleanup_plan::replay::Leaf>,
    Option<&'a crate::cleanup_plan::replay::Leaf>,
    &'a crate::cleanup_plan::replay::Leaf,
    usize,
    std::collections::btree_map::Iter<'a, LivenessFlagId, crate::cleanup_plan::replay::Leaf>,
    Option<(&'a LivenessFlagId, &'a crate::cleanup_plan::replay::Leaf)>,
    &'a LivenessFlagId,
    &'a crate::cleanup_plan::replay::Leaf,
    Vec<LivenessFlagId>,
    Result<Vec<LivenessFlagId>, Error>,
);
type PlaceAcquire<'a> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    semantic_refusal::Receipt,
    Error,
    Option<Error>,
    Result<Vec<LivenessFlagId>, Refusal>,
    Result<Vec<LivenessFlagId>, Error>,
    Result<Permit<'a>, Error>,
    Result<selected_context::Context<'a>, Error>,
    Result<(), Error>,
);

#[test]
fn unknown_place_full_iterator_frame_exact_short_zero_keeps_old_six_fields_before_formals() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    let place = CleanupPlace {
        storage: StorageId::ProvisionalResult,
        projections: Vec::new(),
    };
    let known = BTreeSet::new();
    let leaves = Leaves::new(BTreeMap::new());
    let mut old_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let expected = crate::cleanup_plan::replay::path_join::storage::validate_place(
        function,
        &place,
        &known,
        &leaves,
        &mut old_budget,
    )
    .unwrap_err();
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<PlaceCaller<'_>>()
        + size_of::<PlaceLocal<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<PlaceAcquire<'_>>();
    const WORK: usize = 6 + 15 + 3 + 12 + 3 + 11;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| {
            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                PlaceCaller<'_>,
                6,
                "selected test place caller receiving",
                super::super::place_before_formals::validate_place_before_formals(
                    function,
                    &place,
                    &known,
                    &leaves,
                    &mut budget
                )
            )
        });
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert_eq!(receipt.legacy(function), expected);
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong place first refusal semantic={semantic}: {other:?}"),
        }
    }
}

#[test]
fn unknown_place_full_iterator_frame_exact_short_zero_keeps_old_six_fields() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    let place = CleanupPlace {
        storage: StorageId::ProvisionalResult,
        projections: Vec::new(),
    };
    let known = BTreeSet::new();
    let leaves = Leaves::new(BTreeMap::new());
    let mut old_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let expected = crate::cleanup_plan::replay::path_join::storage::validate_place(
        function,
        &place,
        &known,
        &leaves,
        &mut old_budget,
    )
    .unwrap_err();
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<PlaceCaller<'_>>()
        + size_of::<PlaceLocal<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<WrapperFormal<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<PlaceAcquire<'_>>();
    const WORK: usize = 6 + 15 + 3 + 3 + 12 + 3 + 11;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| {
            crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                PlaceCaller<'_>,
                6,
                "selected test place caller receiving",
                super::super::helpers::validate_place(
                    function,
                    &place,
                    &known,
                    &leaves,
                    &mut budget
                )
            )
        });
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert_eq!(receipt.legacy(function), expected);
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong place first refusal semantic={semantic}: {other:?}"),
        }
    }
}

#[test]
fn genuine_nominal_zero_one_many_scan_pays_every_next_including_terminal_none_before_formals() {
    type ValuesNext<'a> = (
        &'a mut std::collections::btree_map::Values<
            'a,
            LivenessFlagId,
            crate::cleanup_plan::replay::Leaf,
        >,
        Option<&'a crate::cleanup_plan::replay::Leaf>,
        &'a crate::cleanup_plan::replay::Leaf,
    );
    type EntriesNext<'a> = (
        &'a mut std::collections::btree_map::Iter<
            'a,
            LivenessFlagId,
            crate::cleanup_plan::replay::Leaf,
        >,
        Option<(&'a LivenessFlagId, &'a crate::cleanup_plan::replay::Leaf)>,
        &'a LivenessFlagId,
        &'a crate::cleanup_plan::replay::Leaf,
    );
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(super::nominal::SOURCE);
    fixture.with_context(|context| {
        let function = context
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "flat.identity")
            .unwrap();
        let mut setup_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let known =
            crate::cleanup_plan::replay::structural_maps::paid::slots(function, &mut setup_budget)
                .unwrap();
        let leaves =
            crate::cleanup_plan::replay::structural_maps::paid::leaves(function, &mut setup_budget)
                .unwrap();
        let one = leaves.values().next().unwrap().place.clone();
        let root = CleanupPlace {
            storage: one.storage.clone(),
            projections: Vec::new(),
        };
        let mut absent = one.clone();
        absent.projections.push(function.id.clone());
        assert!(
            leaves.len() >= 2,
            "genuine record owns text and Bytes leaves"
        );
        let setup = ReplayBudget::with_physical(function, usize::MAX)
            .unwrap()
            .physical
            .as_ref()
            .unwrap()
            .charged();
        let base = size_of::<PlaceCaller<'_>>()
            + size_of::<PlaceLocal<'_>>()
            + 2 * size_of::<PermitCall<'_>>()
            + size_of::<Control<'_>>()
            + size_of::<PlaceAcquire<'_>>();
        for (case, place) in [absent, one, root].into_iter().enumerate() {
            let mut old_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
            let expected = crate::cleanup_plan::replay::path_join::storage::validate_place(
                function,
                &place,
                &known,
                &leaves,
                &mut old_budget,
            );
            let matches = match &expected {
                Ok(flags) => flags.len(),
                Err(_) => 0,
            };
            match case {
                0 => assert_eq!(matches, 0),
                1 => assert_eq!(matches, 1),
                _ => assert!(matches >= 2),
            }
            // Each full scan visits all leaves and its terminal None. The
            // no-match refusal ends after the first scan, before vector backing.
            let first = leaves.len() + 1;
            let second = if matches == 0 { 0 } else { leaves.len() + 1 };
            let request = base
                + first * size_of::<ValuesNext<'_>>()
                + second * size_of::<EntriesNext<'_>>()
                + matches * size_of::<LivenessFlagId>();
            let work = 50 + 3 * first + 4 * second;
            for (bytes, available, complete) in [
                (setup + request, work, true),
                (setup + request - 1, work, false),
                (setup + request, work - 1, false),
                (setup + request, 0, false),
            ] {
                let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
                budget.remaining = available;
                let result = crate::cleanup_plan::replay::inventory_coverage::paid::receive_observed!(
                    budget,
                    function,
                    PlaceCaller<'_>,
                    6,
                    "selected test place caller receiving",
                    super::super::place_before_formals::validate_place_before_formals(
                        function,
                        &place,
                        &known,
                        &leaves,
                        &mut budget
                    )
                );
                if complete {
                    assert_eq!(
                        result.map_err(|e| render(e, function)),
                        expected,
                        "full old six fields/query order case{case}"
                    );
                    assert_eq!(budget.remaining, 0);
                    assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                } else {
                    assert!(
                        matches!(result, Err(Error::Owned(_))),
                        "first refused packet case{case}"
                    );
                    let ledger = budget.physical.as_deref_mut().unwrap();
                    let first_refusal = ledger.reserve(0).unwrap_err();
                    assert_eq!(ledger.reserve(1).unwrap_err(), first_refusal);
                }
            }
        }
    });
    let (_, observed) = measure(|| {
        fixture.with_context(|context| {
            let function = context
                .functions()
                .iter()
                .find(|f| f.id.as_str() == "flat.identity")
                .unwrap();
            let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
            let known =
                crate::cleanup_plan::replay::structural_maps::paid::slots(function, &mut budget)
                    .unwrap();
            let leaves =
                crate::cleanup_plan::replay::structural_maps::paid::leaves(function, &mut budget)
                    .unwrap();
            let place = leaves.values().next().unwrap().place.clone();
            super::super::place_before_formals::validate_place_before_formals(
                function,
                &place,
                &known,
                &leaves,
                &mut budget,
            )
            .unwrap();
        });
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn genuine_nominal_zero_one_many_scan_pays_every_next_including_terminal_none() {
    type ValuesNext<'a> = (
        &'a mut std::collections::btree_map::Values<
            'a,
            LivenessFlagId,
            crate::cleanup_plan::replay::Leaf,
        >,
        Option<&'a crate::cleanup_plan::replay::Leaf>,
        &'a crate::cleanup_plan::replay::Leaf,
    );
    type EntriesNext<'a> = (
        &'a mut std::collections::btree_map::Iter<
            'a,
            LivenessFlagId,
            crate::cleanup_plan::replay::Leaf,
        >,
        Option<(&'a LivenessFlagId, &'a crate::cleanup_plan::replay::Leaf)>,
        &'a LivenessFlagId,
        &'a crate::cleanup_plan::replay::Leaf,
    );
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(super::nominal::SOURCE);
    fixture.with_context(|context| {
        let function = context
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "flat.identity")
            .unwrap();
        let mut setup_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let known =
            crate::cleanup_plan::replay::structural_maps::paid::slots(function, &mut setup_budget)
                .unwrap();
        let leaves =
            crate::cleanup_plan::replay::structural_maps::paid::leaves(function, &mut setup_budget)
                .unwrap();
        let one = leaves.values().next().unwrap().place.clone();
        let root = CleanupPlace {
            storage: one.storage.clone(),
            projections: Vec::new(),
        };
        let mut absent = one.clone();
        absent.projections.push(function.id.clone());
        assert!(
            leaves.len() >= 2,
            "genuine record owns text and Bytes leaves"
        );
        let setup = ReplayBudget::with_physical(function, usize::MAX)
            .unwrap()
            .physical
            .as_ref()
            .unwrap()
            .charged();
        let base = size_of::<PlaceCaller<'_>>()
            + size_of::<PlaceLocal<'_>>()
            + 2 * size_of::<PermitCall<'_>>()
            + size_of::<WrapperFormal<'_>>()
            + size_of::<Control<'_>>()
            + size_of::<PlaceAcquire<'_>>();
        for (case, place) in [absent, one, root].into_iter().enumerate() {
            let mut old_budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
            let expected = crate::cleanup_plan::replay::path_join::storage::validate_place(
                function,
                &place,
                &known,
                &leaves,
                &mut old_budget,
            );
            let matches = match &expected {
                Ok(flags) => flags.len(),
                Err(_) => 0,
            };
            match case {
                0 => assert_eq!(matches, 0),
                1 => assert_eq!(matches, 1),
                _ => assert!(matches >= 2),
            }
            // Each full scan visits all leaves and its terminal None. The
            // no-match refusal ends after the first scan, before vector backing.
            let first = leaves.len() + 1;
            let second = if matches == 0 { 0 } else { leaves.len() + 1 };
            let request = base
                + first * size_of::<ValuesNext<'_>>()
                + second * size_of::<EntriesNext<'_>>()
                + matches * size_of::<LivenessFlagId>();
            let work = 53 + 3 * first + 4 * second;
            for (bytes, available, complete) in [
                (setup + request, work, true),
                (setup + request - 1, work, false),
                (setup + request, work - 1, false),
                (setup + request, 0, false),
            ] {
                let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
                budget.remaining = available;
                let result = crate::cleanup_plan::replay::inventory_coverage::paid::receive_observed!(
                    budget,
                    function,
                    PlaceCaller<'_>,
                    6,
                    "selected test place caller receiving",
                    super::super::helpers::validate_place(
                        function,
                        &place,
                        &known,
                        &leaves,
                        &mut budget
                    )
                );
                if complete {
                    assert_eq!(
                        result.map_err(|e| render(e, function)),
                        expected,
                        "full old six fields/query order case{case}"
                    );
                    assert_eq!(budget.remaining, 0);
                    assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                } else {
                    assert!(
                        matches!(result, Err(Error::Owned(_))),
                        "first refused packet case{case}"
                    );
                    let ledger = budget.physical.as_deref_mut().unwrap();
                    let first_refusal = ledger.reserve(0).unwrap_err();
                    assert_eq!(ledger.reserve(1).unwrap_err(), first_refusal);
                }
            }
        }
    });
    let (_, observed) = measure(|| {
        fixture.with_context(|context| {
            let function = context
                .functions()
                .iter()
                .find(|f| f.id.as_str() == "flat.identity")
                .unwrap();
            let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
            let known =
                crate::cleanup_plan::replay::structural_maps::paid::slots(function, &mut budget)
                    .unwrap();
            let leaves =
                crate::cleanup_plan::replay::structural_maps::paid::leaves(function, &mut budget)
                    .unwrap();
            let place = leaves.values().next().unwrap().place.clone();
            super::super::helpers::validate_place(function, &place, &known, &leaves, &mut budget)
                .unwrap();
        });
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
