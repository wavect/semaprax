//! Independent old layout/acquisition and literal live-allocation owners.
use super::*;
#[allow(dead_code)]
struct OldStorage<'hir> {
    program: &'hir ResolvedProgram,
    frames: Option<Vec<Frame<'hir>>>,
    leased: bool,
}
fn checked(nested: bool) -> ResolvedProgram {
    let source = if nested {
        SOURCE.replace("string_len(string_concat(\"a\",\"b\"))==2 && positive(count)",
            "{ let inner=match choice { Choice::A {} if positive(count)=>true, _=>false, }; inner }")
    } else {
        SOURCE.to_owned()
    };
    let program = crate::hir::resolve(&crate::check(&source, "nested-pool.spx").unwrap()).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn empty(storage: &Storage<'_>) {
    assert!(storage.frames.as_ref().unwrap().is_empty());
    if let Some(nested) = &storage.nested {
        assert_eq!(nested.depth, 0);
        assert!(nested
            .slots
            .iter()
            .all(|slot| slot.as_ref().unwrap().is_empty()));
    }
}
#[test]
fn checked_nested_guards_keep_full_old_observations_work_and_result_failures() {
    // The pooled backing still replaces the old lease flag in the same space.
    // Invocation authentication additionally retains its owner and first error.
    assert_eq!(
        std::mem::size_of::<Storage<'_>>(),
        (std::mem::size_of::<OldStorage<'_>>()
            + std::mem::size_of::<
                Option<crate::bounded_output::classifier_string::ClassifierLedger>,
            >()
            + std::mem::size_of::<Option<paid_receiving::Failure>>())
        .next_multiple_of(std::mem::align_of::<Storage<'_>>())
    );
    for nested in [false, true] {
        let program = checked(nested);
        let function = &program.functions[1];
        for limit in [1, 8, 64, 512, MAX_REPLAY_WORK_UNITS] {
            let mut before_budget = ReplayBudget::with_skeleton_limit(limit);
            let mut after_budget = ReplayBudget::with_skeleton_limit(limit);
            let mut storage = Storage::new(&program);
            let before = old_expression(
                &program,
                function,
                &function.body,
                &mut SkeletonWork::new(&program, function, &mut before_budget),
            );
            let mut work = SkeletonWork::new(&program, function, &mut after_budget);
            work.continuations = Some(&mut storage);
            ENTRY_COUNT.with(|count| count.set(0));
            let after = expression_skeleton(&program, function, &function.body, &mut work);
            match (before, after) {
                (Ok(old), Ok(new)) => {
                    assert_eq!(old.len(), new.len());
                    for (old, new) in old.iter().zip(&new) {
                        assert_eq!(
                            observation_sequences(&old.observations),
                            observation_sequences(&new.observations)
                        );
                        assert_eq!(old.owned_source, new.owned_source);
                        assert_eq!(
                            (old.failed, old.residual, &old.continuation),
                            (new.failed, new.residual, &new.continuation)
                        );
                    }
                }
                (Err(old), Err(new)) => assert_eq!(old, new),
                _ => panic!("old/new result diverged"),
            }
            assert_eq!(before_budget.remaining, work.budget.remaining);
            assert_eq!(before_budget.skeleton_spent, work.budget.skeleton_spent);
            drop(work);
            empty(&storage);
            if limit == MAX_REPLAY_WORK_UNITS {
                let entries = ENTRY_TRACE.with(std::cell::Cell::get);
                assert_eq!(
                    ENTRY_COUNT.with(std::cell::Cell::get),
                    if nested { 3 } else { 2 }
                );
                assert_eq!(entries[0].0, 0);
                let last = if nested { 2 } else { 1 };
                assert_eq!(entries[0], entries[last]); // first guard has returned
                if nested {
                    assert_eq!(entries[1].0, 1);
                    assert_ne!(entries[0].1, entries[1].1);
                }
                let slots = &storage.nested.as_ref().unwrap().slots;
                assert_eq!(slots.len(), if nested { 2 } else { 1 });
                let outer = storage.frames.as_ref().unwrap().as_ptr();
                for (index, slot) in slots.iter().enumerate() {
                    assert_ne!(outer, slot.as_ref().unwrap().as_ptr());
                    for previous in &slots[..index] {
                        assert_ne!(
                            previous.as_ref().unwrap().as_ptr(),
                            slot.as_ref().unwrap().as_ptr()
                        );
                    }
                }
            }
        }
        // Old one-slot/local-nested dispatch is retained as a whole ordinary
        // receiver control, including both factored and enumerating consumers.
        assert_eq!(
            with_legacy_nested(|| validate_program(&program)),
            validate_program(&program)
        );
        let foreign = program.clone();
        let mut storage = Storage::new(&program);
        let mut budget = ReplayBudget::unobserved();
        let mut work = SkeletonWork::new(&foreign, &foreign.functions[1], &mut budget);
        work.continuations = Some(&mut storage);
        assert!(expression_skeleton(
            &foreign,
            &foreign.functions[1],
            &foreign.functions[1].body,
            &mut work
        )
        .err()
        .unwrap()
        .message
        .contains("another program"));
    }
}
#[test]
fn genuine_guard_receivers_remove_only_repeated_frames_and_pay_directory_overlap() {
    for nested in [false, true] {
        let program = checked(nested);
        let function = &program.functions[1];
        let observe = |pool, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    // This owner directly executes the enumerating skeleton,
                    // so pay its ordinary full census before the first root.
                    // A zero allowance fails before any genuine nested guard.
                    let upper = skeleton_work_upper(&program, function).unwrap();
                    budget.reserve_skeleton(function, upper).unwrap();
                    let mut storage = Storage::new(&program);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    work.continuations = Some(&mut storage);
                    let result = if pool {
                        expression_skeleton(&program, function, &function.body, &mut work)
                    } else {
                        with_legacy_nested(|| {
                            expression_skeleton(&program, function, &function.body, &mut work)
                        })
                    };
                    if result.is_err() {
                        assert!(work.budget.reserve_bytes(function, 0, "retry").is_err());
                    }
                    drop(result);
                    drop(work);
                    drop(storage);
                    drop(budget);
                })
            })
        };
        let (((), old), refused, old_charge, _) = observe(false, usize::MAX);
        assert!(!refused && !old.invalid);
        let (((), new), refused, charge, _) = observe(true, usize::MAX);
        assert!(!refused && !new.invalid);
        // Two sibling guards: N=2,K=1. With an inner guarded match: N=3,K=2.
        // Both avoid exactly one frame request. Directory replacements are
        // capacities 1, then 2; both actual allocations are fully counted.
        let metadata = std::mem::size_of::<Nested<'_>>()
            + (if nested { 1 + 2 } else { 1 }) * std::mem::size_of::<Option<Vec<Frame<'_>>>>();
        let removed = 516 * std::mem::size_of::<Frame<'_>>() - metadata;
        assert_eq!(old.allocated - new.allocated, removed);
        assert_eq!(old_charge - charge, removed);
        assert_eq!((old.live, new.live), (0, 0));
        assert_eq!(old.allocated, old.freed);
        assert_eq!(new.allocated, new.freed);
        assert!(new.highwater <= charge);
        let (((), exact), refused, paid, _) = observe(true, charge);
        assert!(!refused && !exact.invalid && paid == charge && exact.live == 0);
        let (((), short), refused, _, receipt) = observe(true, charge - 1);
        assert!(refused && receipt.is_some() && !short.invalid && short.live == 0);
    }
}
#[test]
fn retained_nested_backing_highwater_tradeoff_and_reuse_pointer_are_explicit() {
    let program = checked(false);
    let function = &program.functions[0];
    let frame_bytes = 516 * std::mem::size_of::<Frame<'_>>();
    let metadata =
        std::mem::size_of::<Nested<'_>>() + std::mem::size_of::<Option<Vec<Frame<'_>>>>();
    let observe = |pool| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut storage = Storage::new(&program);
                let (outer_lease, outer) = storage.acquire(function, &mut budget).unwrap();
                let outer_pointer = outer.as_ptr();
                let mut prior = None;
                for _ in 0..2 {
                    if pool {
                        let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
                        assert_ne!(outer_pointer, frames.as_ptr());
                        if let Some(pointer) = prior {
                            assert_eq!(pointer, frames.as_ptr());
                        }
                        prior = Some(frames.as_ptr());
                        storage.release(lease, frames);
                    } else {
                        drop(fresh(function, &mut budget).unwrap());
                    }
                }
                // A later, independently paid receiver lives after both guards. The
                // pool retains its nested buffer while the old owner has dropped it.
                budget
                    .reserve_array::<u8>(function, 3 * frame_bytes, "later receiver")
                    .unwrap();
                let later = Vec::<u8>::with_capacity(3 * frame_bytes);
                storage.release(outer_lease, outer);
                drop(later);
                drop(storage);
                drop(budget);
            })
        })
    };
    let (((), old), refused, old_charge, _) = observe(false);
    assert!(!refused && !old.invalid);
    let (((), new), refused, charge, _) = observe(true);
    assert!(!refused && !new.invalid);
    assert_eq!(old.allocated - new.allocated, frame_bytes - metadata);
    assert_eq!(old_charge - charge, frame_bytes - metadata);
    assert_eq!(new.highwater - old.highwater, frame_bytes + metadata);
    assert!(new.highwater <= charge);
    assert_eq!((old.live, new.live), (0, 0));
}
#[test]
fn exact_directory_frame_boundaries_and_reused_slot_refusal_restore_depth() {
    let program = checked(false);
    let function = &program.functions[0];
    let frame = 516 * std::mem::size_of::<Frame<'_>>();
    let carrier = std::mem::size_of::<Nested<'_>>();
    let slot = std::mem::size_of::<Option<Vec<Frame<'_>>>>();
    let observe = |limit, levels, sticky| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.prepare_builder(function).unwrap();
                let mut storage = Storage::new(&program);
                let mut failure = None;
                if levels > 0 {
                    match storage.acquire(function, &mut budget) {
                        Err(error) => failure = Some(error),
                        Ok((outer_lease, outer)) => {
                            if levels > 1 {
                                match storage.acquire(function, &mut budget) {
                                    Err(error) => failure = Some(error),
                                    Ok((inner_lease, inner)) => {
                                        if levels > 2 {
                                            match storage.acquire(function, &mut budget) {
                                                Err(error) => failure = Some(error),
                                                Ok((deep_lease, deep)) => {
                                                    storage.release(deep_lease, deep)
                                                }
                                            }
                                        }
                                        storage.release(inner_lease, inner);
                                        if sticky && failure.is_none() {
                                            assert!(budget
                                                .reserve_bytes(
                                                    function,
                                                    usize::MAX,
                                                    "force sticky refusal"
                                                )
                                                .is_err());
                                            assert!(storage
                                                .acquire(function, &mut budget)
                                                .is_err());
                                            assert_eq!(storage.nested.as_ref().unwrap().depth, 0);
                                            assert!(
                                                storage.nested.as_ref().unwrap().slots[0].is_some()
                                            );
                                        }
                                    }
                                }
                            }
                            storage.release(outer_lease, outer);
                        }
                    }
                }
                if failure.is_some() {
                    assert!(budget.reserve_bytes(function, 0, "retry").is_err());
                }
                assert!(storage.frames.as_ref().unwrap().is_empty());
                if let Some(nested) = &storage.nested {
                    assert_eq!(nested.depth, 0);
                    assert!(nested
                        .slots
                        .iter()
                        .all(|slot| slot.as_ref().is_none_or(Vec::is_empty)));
                }
                drop(failure);
                drop(storage);
                drop(budget);
            })
        })
    };
    let (((), baseline), refused, setup, _) = observe(usize::MAX, 0, false);
    assert!(!refused && !baseline.invalid && baseline.live == 0);
    for (levels, extra) in [
        (1, frame),
        (2, 2 * frame + carrier + slot),
        (3, 3 * frame + carrier + 3 * slot),
    ] {
        let (((), paid), refused, charge, _) = observe(setup + extra, levels, false);
        assert!(!refused && !paid.invalid && paid.live == 0);
        assert_eq!(charge, setup + extra);
        assert_eq!(paid.allocated - baseline.allocated, extra);
        assert_eq!(paid.allocated, paid.freed);
    }
    // First carrier, first directory, first nested frame, replacement directory,
    // and deeper frame each refuse one byte before that actual request completes.
    for (levels, extra) in [
        (1, frame),
        (2, frame + carrier),
        (2, frame + carrier + slot),
        (2, 2 * frame + carrier + slot),
        (3, 2 * frame + carrier + 3 * slot),
        (3, 3 * frame + carrier + 3 * slot),
    ] {
        let (((), short), refused, _, receipt) = observe(setup + extra - 1, levels, false);
        assert!(refused && receipt.is_some() && !short.invalid && short.live == 0);
        assert_eq!(short.allocated, short.freed);
    }
    let (((), sticky), refused, _, receipt) = observe(usize::MAX, 2, true);
    assert!(refused && receipt.is_some() && !sticky.invalid && sticky.live == 0);
}
#[test]
fn ordinary_instances_and_contracts_keep_the_exact_program_storage_owner() {
    let source = r#"module instances;
@id("pool.id") fn identity<T>(value:T)->T {value}
@id("pool.main") fn main()->i64 requires true ensures result==42 {identity<i64>(42)}
"#;
    let program =
        crate::hir::resolve(&crate::check(source, "pool-instances.spx").unwrap()).unwrap();
    crate::hir::validate(&program).unwrap();
    assert!(!program.function_instances.is_empty());
    assert_eq!(
        with_legacy_nested(|| validate_program(&program)),
        validate_program(&program)
    );
    let mut storage = Storage::new(&program);
    let mut budget = ReplayBudget::unobserved();
    reserve_program_skeleton_work(
        &program,
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        &mut budget,
    )
    .unwrap();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        work.continuations = Some(&mut storage);
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            drop(expression_skeleton(&program, function, expression, &mut work).unwrap());
        }
    }
    empty(&storage);
    assert!(storage.nested.is_none());
}
