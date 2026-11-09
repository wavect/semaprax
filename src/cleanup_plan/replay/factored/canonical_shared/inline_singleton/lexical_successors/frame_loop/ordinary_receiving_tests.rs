//! Full immediate-old frame owners remain independent of the new ordinary arm.
use super::*;
use crate::cleanup_plan::replay::{SkeletonObservation, SkeletonTerminal, MAX_REPLAY_WORK_UNITS};
use crate::hir::ExpressionId;
use crate::test_allocation_observer::{measure, Metrics};

fn function() -> ResolvedFunction {
    let checked = crate::check(
        "module ordinary.frames; @id(\"main\") fn main()->i64 {0}",
        "ordinary-frames.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap().functions.remove(0)
}

fn graph(
    f: &ResolvedFunction,
    length: usize,
    width: usize,
    cycle: bool,
    unequal: bool,
) -> (Automaton, usize, [usize; 2]) {
    let mut a = Automaton::default();
    let mut b = ReplayBudget::unobserved();
    let sink = a.node(f, &mut b).unwrap();
    let roots = [a.node(f, &mut b).unwrap(), a.node(f, &mut b).unwrap()];
    for (side, root) in roots.iter().copied().enumerate() {
        let mut current = root;
        for depth in 0..length {
            let next = a.node(f, &mut b).unwrap();
            for arm in 0..width {
                // Both sides own distinct labels/ID backing with equal content.
                let label = Label::Observation(Rc::new(SkeletonObservation::Boolean {
                    expression: ExpressionId::from_owned(format!(
                        "frame.{depth}.{}.{}",
                        arm % 2,
                        usize::from(unequal && side == 1 && depth + 1 == length)
                    )),
                    value: arm % 2 == 0,
                }));
                a.movement(f, &mut b, current, label, next).unwrap();
            }
            current = next;
        }
        for terminal in [
            SkeletonTerminal::Success,
            SkeletonTerminal::Failure,
            SkeletonTerminal::LoopLatch,
        ] {
            a.movement(f, &mut b, current, Label::End(terminal), sink)
                .unwrap();
        }
        if cycle {
            // A movement cycle reaches the original open-state refusal. An
            // epsilon self-cycle only spends the entire cursor work budget.
            a.movement(
                f,
                &mut b,
                root,
                Label::End(SkeletonTerminal::LoopLatch),
                root,
            )
            .unwrap();
        }
    }
    (a, sink, roots)
}

type States = Vec<(Vec<(usize, u64)>, usize)>;
type Signatures = Vec<((u64, Vec<(Label, usize)>), usize)>;
struct Run {
    result: Result<Option<bool>, Diagnostic>,
    states: States,
    signatures: Signatures,
    remaining: usize,
    skeleton_remaining: usize,
    spent: usize,
}

fn call(
    c: &mut Canonicalizer<'_>,
    f: &ResolvedFunction,
    root: Weighted,
    b: &mut ReplayBudget,
    r: &mut closure_receiving::ClosureReceiving,
    old: bool,
    compact: bool,
) -> Result<Option<usize>, Diagnostic> {
    match (old, compact) {
        (true, true) => c.id_old_compact_receiving(f, root, b, r),
        (true, false) => c.id_old_general_receiving(f, root, b, r),
        (false, true) if matches!(b.payer, AllocationPayer::LegacyUnobserved) => {
            let dispatch = c.canonical_dispatch(&b.payer);
            dispatch(c, f, root, b, r)
        }
        (false, true) => c.id_with_compact_receiving(f, root, b, r),
        (false, false) => c.id_with_receiving(f, root, b, r),
    }
}

fn run(
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    roots: [usize; 2],
    old: bool,
    compact: bool,
    work: usize,
    weight: u64,
) -> Run {
    let mut b = ReplayBudget::unobserved();
    b.remaining = work;
    b.skeleton_remaining = work;
    let mut c = Canonicalizer::new(a, sink);
    let mut r = closure_receiving::ClosureReceiving::from_prepaid(f, &b);
    let result = (|| {
        let left = call(
            &mut c,
            f,
            vec![(roots[0], weight)],
            &mut b,
            &mut r,
            old,
            compact,
        )?;
        let right = call(
            &mut c,
            f,
            vec![(roots[1], weight)],
            &mut b,
            &mut r,
            old,
            compact,
        )?;
        Ok(left.zip(right).map(|(l, r)| l == r))
    })();
    // Complete-state captures are outside any paid account and are not used
    // as physical-allocation evidence. Both entire old and current owners run.
    Run {
        result,
        states: c
            .states
            .iter()
            .map(|(k, v)| (k.as_slice().to_vec(), *v))
            .collect(),
        signatures: c.ids.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        remaining: b.remaining,
        skeleton_remaining: b.skeleton_remaining,
        spent: b.skeleton_spent,
    }
}
fn same(a: &Run, b: &Run) {
    match (&a.result, &b.result) {
        (Ok(a), Ok(b)) => assert_eq!(a, b),
        (Err(a), Err(b)) => assert_eq!(
            (&a.code, &a.severity, &a.message, &a.path, &a.span, &a.help),
            (&b.code, &b.severity, &b.message, &b.path, &b.span, &b.help)
        ),
        _ => panic!("old/current result class differs"),
    }
    assert_eq!(a.states, b.states);
    assert_eq!(a.signatures, b.signatures);
    assert_eq!(
        (a.remaining, a.skeleton_remaining, a.spent),
        (b.remaining, b.skeleton_remaining, b.spent)
    );
}

#[test]
fn ordinary_general_and_actual_compact_frames_keep_complete_states_labels_and_terminals() {
    let f = function();
    for (length, width, unequal) in [
        (0, 0, false),
        (1, 0, false),
        (1, 1, false),
        (8, 1, false),
        (5, 3, false),
        (5, 3, true),
        (64, 1, false),
    ] {
        let (a, sink, roots) = graph(&f, length, width, false, unequal);
        for compact in [false, true] {
            let old = run(&f, &a, sink, roots, true, compact, MAX_REPLAY_WORK_UNITS, 1);
            let current = run(
                &f,
                &a,
                sink,
                roots,
                false,
                compact,
                MAX_REPLAY_WORK_UNITS,
                1,
            );
            same(&old, &current);
            assert_eq!(current.result.as_ref().unwrap(), &Some(!unequal));
        }
    }
}

#[test]
fn every_original_small_work_cut_cycle_and_overflow_keeps_all_six_diagnostic_fields() {
    let f = function();
    for (cycle, weight) in [(false, 1), (false, u64::MAX), (true, 1)] {
        let (a, sink, roots) = graph(&f, 2, 3, cycle, false);
        for compact in [false, true] {
            let ample = run(
                &f,
                &a,
                sink,
                roots,
                true,
                compact,
                MAX_REPLAY_WORK_UNITS,
                weight,
            );
            let used = MAX_REPLAY_WORK_UNITS - ample.remaining;
            for work in 0..=used + 1 {
                same(
                    &run(&f, &a, sink, roots, true, compact, work, weight),
                    &run(&f, &a, sink, roots, false, compact, work, weight),
                );
            }
        }
    }
}

fn physical(
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    roots: [usize; 2],
    old: bool,
    compact: bool,
) -> ((bool, usize, usize), Metrics) {
    measure(|| {
        let mut b = ReplayBudget::unobserved();
        let mut c = Canonicalizer::new(a, sink);
        let mut r = closure_receiving::ClosureReceiving::from_prepaid(f, &b);
        let result = (|| {
            let left = call(&mut c, f, vec![(roots[0], 1)], &mut b, &mut r, old, compact)?;
            let right = call(&mut c, f, vec![(roots[1], 1)], &mut b, &mut r, old, compact)?;
            Ok::<_, Diagnostic>(left.zip(right).map(|(l, r)| l == r))
        })();
        let values = (
            matches!(result, Ok(Some(true))),
            b.remaining,
            b.skeleton_spent,
        );
        drop(result);
        drop(r);
        drop(c);
        drop(b);
        values
    })
}
#[test]
fn actual_cold_constructor_to_final_drop_eliminates_ordinary_owned_successor_vectors() {
    let f = function();
    let (a, sink, roots) = graph(&f, 64, 1, false, false);
    for compact in [false, true] {
        let old = physical(&f, &a, sink, roots, true, compact);
        let current = physical(&f, &a, sink, roots, false, compact);
        assert_eq!(old.0, current.0);
        assert!(current.0 .0);
        for metrics in [old.1, current.1] {
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(current.1.allocations < old.1.allocations);
        assert!(current.1.allocated < old.1.allocated);
        assert!(current.1.highwater <= old.1.highwater);
        eprintln!(
            "ordinary canonical frames compact={compact} complete_old={:?} current={:?}",
            old.1, current.1
        );
    }
}

fn funded(
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    roots: [usize; 2],
    old: bool,
    compact: bool,
    cap: usize,
    floor: usize,
) -> (
    (Result<Option<bool>, Diagnostic>, usize, usize),
    bool,
    usize,
) {
    crate::bounded_output::with_limit_usage(cap, || {
        let mut b = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        assert!(matches!(b.payer, AllocationPayer::ActiveBuilder));
        assert!(crate::bounded_output::set_active_floor(floor));
        let result = (|| {
            b.reserve_bytes(
                f,
                Canonicalizer::CONSTRUCTOR_CONTROLS
                    + closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "matched ordinary frame whole owners",
            )?;
            let mut c = Canonicalizer::new(a, sink);
            let mut r = closure_receiving::ClosureReceiving::from_prepaid(f, &b);
            // Each actual test compatibility call has its own full argument,
            // formal and Result inventory. No test bank is an algorithm credit.
            let invocation = 2 * std::mem::size_of::<(
                &mut Canonicalizer<'_>,
                &ResolvedFunction,
                Weighted,
                &mut ReplayBudget,
                &mut closure_receiving::ClosureReceiving,
                bool,
                bool,
            )>() + 2 * std::mem::size_of::<Result<Option<usize>, Diagnostic>>();
            b.reserve_bytes(f, invocation, "matched ordinary first invocation")?;
            b.reserve_array::<(usize, u64)>(f, 1, "matched ordinary first root")?;
            let left = call(&mut c, f, vec![(roots[0], 1)], &mut b, &mut r, old, compact)?;
            b.reserve_bytes(f, invocation, "matched ordinary second invocation")?;
            b.reserve_array::<(usize, u64)>(f, 1, "matched ordinary second root")?;
            let right = call(&mut c, f, vec![(roots[1], 1)], &mut b, &mut r, old, compact)?;
            Ok(left.zip(right).map(|(l, r)| l == r))
        })();
        crate::bounded_output::clear_active_floor();
        (result, b.remaining, b.skeleton_spent)
    })
}
#[test]
fn existing_paid_route_retains_exact_short_zero_floor_invoices_and_final_drop() {
    let f = function();
    let (a, sink, roots) = graph(&f, 3, 3, false, false);
    for compact in [false, true] {
        let ample = funded(&f, &a, sink, roots, true, compact, 64 * 1024 * 1024, 0);
        assert!(matches!(ample.0 .0, Ok(Some(true))) && !ample.1);
        let paid = ample.2;
        for (cap, floor) in [(0, 0), (paid - 1, 0), (paid, 0), (paid, 1)] {
            let (_, heap) = measure(|| {
                let old = funded(&f, &a, sink, roots, true, compact, cap, floor);
                let current = funded(&f, &a, sink, roots, false, compact, cap, floor);
                match (&old.0 .0, &current.0 .0) {
                    (Ok(a), Ok(b)) => assert_eq!(a, b),
                    (Err(a), Err(b)) => assert_eq!(
                        (&a.code, &a.severity, &a.message, &a.path, &a.span, &a.help),
                        (&b.code, &b.severity, &b.message, &b.path, &b.span, &b.help)
                    ),
                    _ => panic!("paid old/current result class differs"),
                }
                assert_eq!(
                    (old.0 .1, old.0 .2, old.1, old.2),
                    (current.0 .1, current.0 .2, current.1, current.2)
                );
                if cap == paid && floor == 0 {
                    assert!(matches!(current.0 .0, Ok(Some(true))) && !current.1);
                    assert_eq!(current.2, paid);
                } else {
                    assert!(current.0 .0.is_err() && current.1);
                }
            });
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
        }
    }
}

#[test]
fn held_invocation_rejects_foreign_function_and_keeps_the_receiver_failed() {
    let f = function();
    let foreign = f.clone();
    let (a, sink, roots) = graph(&f, 3, 1, false, false);
    for old in [false, true] {
        for compact in [false, true] {
            let mut b = ReplayBudget::unobserved();
            let mut c = Canonicalizer::new(&a, sink);
            let mut r = closure_receiving::ClosureReceiving::from_prepaid(&f, &b);
            assert!(call(
                &mut c,
                &f,
                vec![(roots[0], 1)],
                &mut b,
                &mut r,
                old,
                compact
            )
            .is_ok());
            let before = b.remaining;
            let error = call(
                &mut c,
                &foreign,
                vec![(roots[1], 1)],
                &mut b,
                &mut r,
                old,
                compact,
            )
            .unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(error.message, "");
            assert_eq!(b.remaining, before);
            assert!(c.closures.failed());
            let retry = call(
                &mut c,
                &f,
                vec![(roots[1], 1)],
                &mut b,
                &mut r,
                old,
                compact,
            )
            .unwrap_err();
            assert_eq!(
                (
                    &error.code,
                    &error.severity,
                    &error.message,
                    &error.path,
                    &error.span,
                    &error.help
                ),
                (
                    &retry.code,
                    &retry.severity,
                    &retry.message,
                    &retry.path,
                    &retry.span,
                    &retry.help
                )
            );
            assert_eq!(b.remaining, before);
        }
    }
}

fn replace_once(source: &str, after: &str, before: &str) -> String {
    assert_eq!(source.matches(after).count(), 1);
    source.replacen(after, before, 1)
}
#[test]
fn both_complete_immediate_old_loops_and_wrapper_restore_exact_frozen_bytes() {
    let general = include_str!("../frame_loop.rs");
    let compact = include_str!("compact.rs");
    let general_registration = "\n#[cfg(test)]\n#[allow(\n    dead_code,\n    reason = \"Compile the complete immediate-old frame owner independently.\"\n)]\nmod ordinary_receiving_before;\n#[cfg(test)]\nmod ordinary_receiving_tests;\n";
    let compact_registration = "\n#[cfg(test)]\n#[allow(\n    dead_code,\n    reason = \"Compile the complete immediate-old compact owner independently.\"\n)]\nmod ordinary_receiving_before;\n";
    for (source, registration, before) in [
        (
            general,
            general_registration,
            include_str!("ordinary_receiving.general.before.txt"),
        ),
        (
            compact,
            compact_registration,
            include_str!("compact/ordinary_receiving.before.txt"),
        ),
    ] {
        let restored = replace_once(source, registration, "");
        let restored = replace_once(
            &restored,
            "// order for every payer outside the ordinary routes.",
            "// order for every non-ActiveBuilder payer.",
        );
        let restored = replace_once(&restored,
            "payer:\n                                    AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved,",
            "payer: AllocationPayer::ActiveBuilder,");
        assert_eq!(restored, before);
    }
    let general_before = include_str!("ordinary_receiving_before.rs");
    let (loop_source, wrapper) = general_before
        .split_once("\nimpl Canonicalizer<'_> {\n")
        .unwrap();
    let loop_source = replace_once(loop_source, "use super::*;\n", concat!(
        "use super::super::*;\n",
        "pub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) mod borrowed_single;\n"));
    let loop_source = replace_once(&loop_source, "fn id_old_general_loop(", "fn id_inner_into(");
    let loop_source = loop_source + "\npub(in crate::cleanup_plan::replay::factored::canonical_shared::inline_singleton) mod compact;\n";
    assert_eq!(
        loop_source,
        include_str!("ordinary_receiving.general.before.txt")
    );
    let wrapper = wrapper.strip_suffix("\n}\n").unwrap();
    let wrapper = replace_once(
        wrapper,
        "fn id_old_general_receiving(",
        "fn id_with_receiving(",
    );
    let wrapper = replace_once(&wrapper, "self.id_old_general_loop(", "self.id_inner_into(");
    let wrapper = replace_once(
        &wrapper,
        "super::super::super::super::tests::note_actual_entry(budget);",
        "super::tests::note_actual_entry(budget);",
    );
    assert_eq!(
        wrapper,
        include_str!("ordinary_receiving.wrapper.before.txt")
    );
    let mut compact_before = include_str!("compact/ordinary_receiving_before.rs").to_owned();
    for (after, before, count) in [
        ("id_old_compact_loop(", "id_inner_into_compact(", 2),
        ("id_old_compact_receiving(", "id_with_compact_receiving(", 2),
        ("id_old_compact(", "id_compact(", 1),
    ] {
        assert_eq!(compact_before.matches(after).count(), count);
        compact_before = compact_before.replacen(after, before, count);
    }
    let compact_before = replace_once(&compact_before,
        "use super::payments as compact_successors;\nuse super::*;\n",
        "use super::*;\nuse payments as compact_successors;\n#[macro_use]\nmod payments;\nmod active;\nmod compact_targets;\n");
    assert_eq!(
        compact_before,
        include_str!("compact/ordinary_receiving.before.txt")
    );
}

#[test]
fn all_bound_unchanged_canonical_helpers_remain_complete_compiled_and_physical_bytes() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled, before_sha) in [
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared.rs")).as_slice(), "371265c7d8cc05edfe08f5a7f160ab4d1d4a68b50d323aa63bf9c7989b820729"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs")).as_slice(), "87257e50e24a64d047eff17fd2ef9de7e1ed4c1049697222223947c62b28f514"),
        ("src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs")).as_slice(), "006a2efb76b4d30c6062d0af713a7d2b4f650c01b43a6e0a3b80c11c3b986a66"),
        ("src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs")).as_slice(), "fc70f96c6f3e58b4fc22d545579f0bf6af576a5d0211514528daff71e6e8f6f6"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors.rs")).as_slice(), "39412583a543bf0b2d2f3f38c11d882183d89abfef4b8ae650f827142a3d2470"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/borrowed_single.rs")).as_slice(), "b20fa6833b1d03a2b28fd78b4dbc5be9eb31125dc1616295ab4de1de98570073"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/payments.rs")).as_slice(), "9d7181c923df3c53a31ebdd1571d3c1f9eac990f605345b06a708dc19d251e8c"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/compact_targets.rs")).as_slice(), "75270c55c77ae6ea0697f52348e7c6ad78c5dbbca706bdd4b8faabd3849d1e10"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/active.rs")).as_slice(), "991492eeee4ed2c8783ed2bf0bffddd661f76633738074ad78b4301b032ab75e"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/inline_exit.rs")).as_slice(), "d485b165d89d23d063fed7688b8df6e0c297f088982448c8c6dfec17b96ba5e1"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/terminal_leaf.rs")).as_slice(), "e96169bac2c81637e3a741eb51dad2d48604d59eec0c3c441d9db1e2b7a4d649"),
        ("src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs")).as_slice(), "a61fc4565585ea5abec72d9821f1a93e16aba863b2fafb0bed120ced1d81e3d9"),
        ("src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs")).as_slice(), "883a182228e20ef867483585b83f6c81a6b44000e2bff5766e1ade6571c40325"),
        ("src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs")).as_slice(), "9a2cb441c09a5bc431c551bd7d5e54f06a89f88f712088b4529f366e4f517c0c"),
        ("src/cleanup_plan/replay/factored/canonical_shared/inline_weighted.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/inline_weighted.rs")).as_slice(), "109c4167a9b98a414307ba801f668fc20be3e73e04331a2ac9c3e4f959df90fb"),
        ("src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/paid_many.rs")).as_slice(), "11032a973ff4ea3a7521d2a678f4abe73593b6ac4916f115af60331540dd0b82"),
        ("src/cleanup_plan/replay/physical_budget.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_budget.rs")).as_slice(), "dca3a126f4c0dfde7e45db233d011100c5f9411cd77766260a9b1a4669c05cbd"),
        ("src/cleanup_plan/replay/allocation.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation.rs")).as_slice(), "d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59"),
        ("src/cleanup_plan/replay/physical_storage.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_storage.rs")).as_slice(), "5735899d648a1a00c84a00b1d32ebb2f75f3f623a4bda19196afee6f1994dead"),
        ("src/cleanup_plan/replay/physical_clone.rs", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/physical_clone.rs")).as_slice(), "f730ed63dbd193546319a8aa9bdadf15c7cd44c984bbafd96bb9756628695ac3"),
    ] {
        let physical = std::fs::read(root.join(path)).unwrap();
        assert_eq!(physical, compiled, "complete actual helper {path}");
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(compiled))), before_sha,
            "complete immediate-old helper {path}");
    }
}
