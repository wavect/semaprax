//! Complete immediate-owner controls. All measurements remain authored/unrun.
use super::super::lexical_successors::census as old_census;
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::MAX_REPLAY_WORK_UNITS;
use crate::hir::ResolvedProgram;
use crate::test_allocation_observer::{measure, Metrics};
use inline_exit::census as shape_census;
use std::mem::{align_of, size_of};
type OldCall<'a> = (
    &'a mut super::super::Canonicalizer<'a>,
    &'a ResolvedFunction,
    Weighted,
    &'a mut ReplayBudget,
);
type NewCall<'a> = (
    &'a mut Canonicalizer<'a>,
    &'a ResolvedFunction,
    Weighted,
    &'a mut ReplayBudget,
);
const OLD_CALL: usize = 2 * size_of::<OldCall<'static>>()
    + 2 * size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * size_of::<Weighted>();
const NEW_CALL: usize = 2 * size_of::<NewCall<'static>>()
    + 2 * size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * size_of::<Weighted>();
fn function() -> ResolvedFunction {
    super::super::super::lexical_plan_tests::fixture(super::super::super::lexical_plan_tests::WHILE)
        .1
}
fn graph(
    f: &ResolvedFunction,
    length: usize,
    width: usize,
    cycle: bool,
) -> (Automaton, usize, usize) {
    let mut a = Automaton::default();
    let mut b = ReplayBudget::unobserved();
    let sink = a.node(f, &mut b).unwrap();
    let mut next = sink;
    for index in 0..length {
        let node = a.node(f, &mut b).unwrap();
        for branch in 0..width {
            let terminal = if branch % 2 == 0 {
                super::super::super::SkeletonTerminal::Success
            } else {
                super::super::super::SkeletonTerminal::Failure
            };
            a.movement(f, &mut b, node, Label::End(terminal), next)
                .unwrap();
        }
        if cycle && index == 0 {
            a.movement(
                f,
                &mut b,
                node,
                Label::End(super::super::super::SkeletonTerminal::Success),
                node,
            )
            .unwrap();
        }
        next = node;
    }
    (a, sink, next)
}
#[derive(Debug, PartialEq, Eq)]
struct Failure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 2048], usize),
    path: Option<([u8; 2048], usize)>,
    span: Option<crate::ast::Span>,
    help: Option<([u8; 2048], usize)>,
}
fn text(s: &str) -> ([u8; 2048], usize) {
    assert!(s.len() <= 2048);
    let mut bytes = [0; 2048];
    bytes[..s.len()].copy_from_slice(s.as_bytes());
    (bytes, s.len())
}
fn failure(d: Diagnostic) -> Failure {
    Failure {
        code: d.code,
        severity: d.severity,
        message: text(&d.message),
        path: d.path.as_deref().map(text),
        span: d.span,
        help: d.help.as_deref().map(text),
    }
}
#[derive(Debug)]
struct Run {
    result: Result<Option<usize>, Failure>,
    remaining: usize,
    skeleton: usize,
    spent: usize,
    paid: usize,
    refused: bool,
    heap: Metrics,
    shapes: shape_census::Row,
    successors: old_census::Report,
}
#[allow(
    clippy::too_many_arguments,
    reason = "Matched whole-owner controls retain the actual independent old/current calls."
)]
fn run(
    old: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    work: usize,
    floor: usize,
) -> Run {
    let ((result, remaining, skeleton, spent, heap, shapes, successors), refused, paid, _) =
        with_limit_usage_refusal(limit, || {
            let ((result, remaining, skeleton, spent, shapes, successors), heap) = measure(|| {
                let mut b = ReplayBudget::with_skeleton_limit(work);
                b.remaining = work;
                let observer = old_census::observe(f, &b);
                let shapes = shape_census::observe(f, &b);
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = (|| {
                    if old {
                        b.reserve_bytes(
                            f,
                            super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + OLD_CALL,
                            "canonical ordered dictionary owner receiving",
                        )?;
                        let mut c = super::super::Canonicalizer::new(a, sink);
                        b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
                        c.id(f, vec![(root, weight)], &mut b)
                    } else {
                        b.reserve_bytes(
                            f,
                            Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                            "canonical ordered dictionary owner receiving",
                        )?;
                        let mut c = Canonicalizer::new(a, sink);
                        b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
                        c.id(f, vec![(root, weight)], &mut b)
                    }
                })()
                .map_err(failure);
                let row = shapes.finish(matches!(result, Ok(Some(_))));
                let successor = old_census::take();
                drop(shapes);
                drop(observer);
                (
                    result,
                    b.remaining,
                    b.skeleton_remaining,
                    b.skeleton_spent,
                    row,
                    successor,
                )
            });
            (result, remaining, skeleton, spent, heap, shapes, successors)
        });
    Run {
        result,
        remaining,
        skeleton,
        spent,
        paid,
        refused,
        heap,
        shapes,
        successors,
    }
}
fn balanced(r: &Run) {
    assert!(!r.heap.invalid);
    assert_eq!(r.heap.live, 0);
    assert_eq!(r.heap.allocated, r.heap.freed);
}
fn same(old: &Run, current: &Run) {
    assert_eq!(current.result, old.result);
    assert_eq!(
        (current.remaining, current.skeleton, current.spent),
        (old.remaining, old.skeleton, old.spent)
    );
    balanced(old);
    balanced(current);
}
#[test]
fn actual_old_and_inline_frame_and_output_layouts_keep_the_original_capacity_invoice() {
    type OldOutput = (u64, Vec<(Label, SharedWeighted)>);
    type NewOutput = (u64, &'static Label, SharedWeighted);
    assert_eq!(
        size_of::<CanonicalFrame<'static>>(),
        size_of::<super::super::CanonicalFrame>()
    );
    assert_eq!(
        align_of::<CanonicalFrame<'static>>(),
        align_of::<super::super::CanonicalFrame>()
    );
    assert!(size_of::<Option<NewOutput>>() <= size_of::<Option<OldOutput>>());
    assert_eq!(
        Canonicalizer::CONSTRUCTOR_CONTROLS,
        super::super::Canonicalizer::CONSTRUCTOR_CONTROLS
    );
    assert_eq!(OLD_CALL, NEW_CALL);
    eprintln!(
        "OPT889 actual historical owned-candidate frame={}",
        size_of::<inline_exit::OwnedFrameBefore>()
    );
    assert_eq!(
        inline_exit::OWNED_SCHEDULE_PRIMITIVES,
        4 * inline_exit::SCHEDULE_PRIMITIVES
    );
    eprintln!("OPT889 actual layouts frame old={} current={} align old={} current={} output old={} current={} fields={} query={} selectors={} root_primitives={}",size_of::<super::super::CanonicalFrame>(),size_of::<CanonicalFrame<'static>>(),align_of::<super::super::CanonicalFrame>(),align_of::<CanonicalFrame<'static>>(),size_of::<Option<OldOutput>>(),size_of::<Option<NewOutput>>(),inline_exit::FIELDS,inline_exit::RECEIVING,inline_exit::SELECT,inline_exit::PRIMITIVES);
}
#[test]
fn zero_one_many_and_repeated_labels_preserve_complete_immediate_ids_work_and_drop() {
    let f = function();
    for length in [0, 1, 4, 17] {
        for width in [0, 1, 2, 4] {
            for weight in [1, 2, u64::MAX] {
                let (a, sink, root) = graph(&f, length, width, false);
                let old = run(
                    true,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                let current = run(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                same(&old, &current);
                assert_eq!(old.refused, current.refused);
                eprintln!("OPT889 shape length={length} width={width} weight={weight} old={} current={} new={:?}",old.paid,current.paid,current.shapes);
            }
        }
    }
}
#[test]
fn genuine_long_singleton_chain_has_positive_complete_typed_net_without_padding_old() {
    let f = function();
    let (a, sink, root) = graph(&f, 64, 1, false);
    let old = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    let current = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    same(&old, &current);
    assert!(current.result.is_ok() && !old.refused && !current.refused);
    assert!(current.shapes.complete && !current.shapes.overflowed);
    assert!(current.successors.complete && !current.successors.foreign_observation);
    let n = current.successors.cells[1]
        .iter()
        .flat_map(|phase| phase.iter())
        .map(|c| {
            // `acquired` is a byte invoice; this complete successful run needs
            // the event count for its per-selector charge.
            assert_eq!(c.refused, 0);
            assert_eq!(c.requested, c.acquired);
            c.attempts
        })
        .sum::<usize>();
    let m = current.successors.cells[1]
        .iter()
        .flat_map(|phase| phase.iter())
        .map(|c| c.primitive_banks)
        .sum::<usize>();
    let one = current.shapes.arity[1];
    assert!(one >= 64);
    let old_per = super::super::Canonicalizer::BORROWED_EXIT_WARM;
    let old_first = super::super::Canonicalizer::BORROWED_EXIT_PRIMITIVE;
    let extra_cold = lexical_successors::ONE_PASS_UNIT_ABI;
    let drop_bytes =
        current.shapes.unit_drops * 2 * size_of::<Rc<ordered_weights::OrderedWeights>>()
            + current.shapes.general_drops * 2 * size_of::<ordered_weights::OrderedWeights>();
    // The two genuine complete Frame types have equal width, and the identical
    // push/pop order keeps every cumulative replacement capacity invoice equal.
    let expected = one as i128
        * (old_per as i128 - inline_exit::RECEIVING as i128 - inline_exit::FIELDS as i128
            + size_of::<(Label, SharedWeighted)>() as i128)
        + old_first as i128
        - inline_exit::PRIMITIVES as i128
        - m as i128 * extra_cold as i128
        - n as i128 * inline_exit::SELECT as i128
        - drop_bytes as i128;
    assert_eq!(old.paid as i128 - current.paid as i128, expected);
    assert!(
        expected > 0 && current.paid < old.paid,
        "no adoption from gross Vec savings: {current:?}"
    );
    assert!(
        current.heap.allocations < old.heap.allocations,
        "actual removed singleton backing, not a fee-only change"
    );
    eprintln!("OPT889 complete old={old:?} current={current:?} N={n} M={m} actual_net={expected}; no application result inferred");
}
#[test]
fn every_small_work_cut_and_cycle_keep_six_fields_before_later_receiving() {
    let f = function();
    let (a, sink, root) = graph(&f, 2, 1, false);
    let full = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    let used = MAX_REPLAY_WORK_UNITS - full.remaining;
    for work in 0..=used {
        let old = run(true, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0);
        let current = run(false, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0);
        same(&old, &current);
    }
    let (a, sink, root) = graph(&f, 1, 1, true);
    let old = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    let current = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    same(&old, &current);
    assert!(current.result.is_err());
}
#[test]
fn actual_inline_owner_exact_short_zero_and_floor_refusals_release_every_owner() {
    let f = function();
    let (a, sink, root) = graph(&f, 4, 1, false);
    let full = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(matches!(full.result, Ok(Some(_))));
    let exact = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        full.paid,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(exact.result.is_ok() && !exact.refused);
    balanced(&exact);
    for limit in [0, full.paid - 1] {
        let short = run(
            false,
            &f,
            &a,
            sink,
            root,
            1,
            limit,
            MAX_REPLAY_WORK_UNITS,
            0,
        );
        assert!(short.result.is_err() && short.refused);
        balanced(&short);
        assert!(!short.shapes.complete);
    }
    let floor = run(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        64 * 1024 * 1024,
    );
    assert!(floor.result.is_err() && floor.refused);
    balanced(&floor);
}

#[test]
fn complete_immediate_hir_and_plan_caller_keeps_results_work_and_unchanged_cap() {
    use super::super::super::{
        before_inline_singleton, compare_with_index_storage, lexical_plan_tests,
    };
    fn invoke(
        old: bool,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        limit: usize,
    ) -> (
        Result<Option<bool>, Failure>,
        bool,
        usize,
        usize,
        usize,
        Metrics,
    ) {
        let ((result, remaining, spent, heap), refused, paid, _) =
            with_limit_usage_refusal(limit, || {
                let ((result, remaining, spent), heap) = measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = (|| {
                        crate::cleanup_plan::replay::reserve_program_skeleton_work(
                            program,
                            std::iter::once(function),
                            &mut budget,
                        )?;
                        if old {
                            before_inline_singleton::compare_with_index_storage(
                                program,
                                function,
                                &mut budget,
                                None,
                                None,
                            )
                        } else {
                            compare_with_index_storage(program, function, &mut budget, None, None)
                        }
                    })()
                    .map_err(failure);
                    (result, budget.remaining, budget.skeleton_spent)
                });
                (result, remaining, spent, heap)
            });
        (result, refused, paid, remaining, spent, heap)
    }
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let (program, function) = lexical_plan_tests::fixture(source);
        let old = invoke(true, &program, &function, 64 * 1024 * 1024);
        let current = invoke(false, &program, &function, 64 * 1024 * 1024);
        assert_eq!(old.0, Ok(Some(true)));
        assert_eq!(current.0, old.0);
        assert!(!old.1 && !current.1);
        assert_eq!((current.3, current.4), (old.3, old.4));
        for heap in [&old.5, &current.5] {
            assert!(!heap.invalid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
        }
        // A complete composition may pay more on a cold graph. This owner
        // checks real semantics and teardown without padding the old invoice.
        eprintln!(
            "OPT889 complete caller old={} current={} old_heap={:?} current_heap={:?}",
            old.2, current.2, old.5, current.5
        );
        let exact = invoke(false, &program, &function, current.2);
        assert_eq!(exact.0, current.0);
        assert!(!exact.1);
        for limit in [0, current.2 - 1] {
            let short = invoke(false, &program, &function, limit);
            assert!(short.0.is_err() && short.1);
            assert_eq!(short.5.live, 0);
            assert_eq!(short.5.allocated, short.5.freed);
        }
    }
}

#[test]
fn every_signature_state_and_cumulative_frame_capacity_matches_the_whole_old_owner() {
    let f = function();
    for length in [0, 1, 4, 64] {
        for width in [0, 1, 2, 4] {
            let (a, sink, root) = graph(&f, length, width, false);
            let (_, refused, _, _) = with_limit_usage_refusal(64 * 1024 * 1024, || {
                let mut x = ReplayBudget::new();
                let mut y = ReplayBudget::new();
                x.reserve_bytes(
                    &f,
                    super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * OLD_CALL,
                    "889 old complete owner",
                )
                .unwrap();
                y.reserve_bytes(
                    &f,
                    Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * NEW_CALL,
                    "889 current complete owner",
                )
                .unwrap();
                let mut old = super::super::Canonicalizer::new(&a, sink);
                let mut current = Canonicalizer::new(&a, sink);
                for _ in 0..2 {
                    x.reserve_array::<(usize, u64)>(&f, 1, "889 old root")
                        .unwrap();
                    y.reserve_array::<(usize, u64)>(&f, 1, "889 current root")
                        .unwrap();
                    assert_eq!(
                        old.id(&f, vec![(root, 1)], &mut x),
                        current.id(&f, vec![(root, 1)], &mut y)
                    );
                    assert!(old.ids.iter().eq(current.ids.iter()));
                    assert!(old.states.iter().eq(current.states.iter()));
                    assert_eq!(old.frames.capacity(), current.frames.capacity());
                    assert!(old.frames.is_empty() && current.frames.is_empty());
                    assert_eq!(
                        (x.remaining, x.skeleton_remaining, x.skeleton_spent),
                        (y.remaining, y.skeleton_remaining, y.skeleton_spent)
                    );
                }
            });
            assert!(!refused);
        }
    }
}

#[test]
fn distinct_equal_observation_owners_and_overflow_keep_full_order_and_six_fields() {
    let f = function();
    let mut a = Automaton::default();
    let mut setup = ReplayBudget::unobserved();
    let sink = a.node(&f, &mut setup).unwrap();
    let root = a.node(&f, &mut setup).unwrap();
    let observation = super::super::super::SkeletonObservation::Boolean {
        expression: crate::hir::ExpressionId::from_owned("889.equal.content".into()),
        value: true,
    };
    let first = Rc::new(observation.clone());
    let second = Rc::new(observation);
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(first, second);
    a.movement(&f, &mut setup, root, Label::Observation(first), sink)
        .unwrap();
    a.movement(&f, &mut setup, root, Label::Observation(second), sink)
        .unwrap();
    for weight in [0, 1, 2, u64::MAX] {
        for work in [0, 1, 31, MAX_REPLAY_WORK_UNITS] {
            let old = run(true, &f, &a, sink, root, weight, 64 * 1024 * 1024, work, 0);
            let current = run(false, &f, &a, sink, root, weight, 64 * 1024 * 1024, work, 0);
            same(&old, &current);
        }
    }
}

#[test]
fn inline_banks_cannot_authenticate_foreign_function_budget_or_nested_restored_invocation() {
    let f = function();
    let wrong = f.clone();
    let (a, sink, root) = graph(&f, 4, 1, false);
    for attack in 0..3 {
        with_limit_usage_refusal(1 << 28, || {
            let mut b = ReplayBudget::new();
            b.reserve_bytes(
                &f,
                Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                "884 actual warm constructor and callers",
            )
            .unwrap();
            let mut c = Canonicalizer::new(&a, sink);
            b.reserve_array::<(usize, u64)>(&f, 1, "884 warm root")
                .unwrap();
            assert!(c.id(&f, vec![(root, 1)], &mut b).unwrap().is_some());
            let flags = (c.successor_unit_paid, c.successor_general_paid);
            let observer = old_census::observe(&f, &b);
            b.reserve_bytes(&f, 2 * NEW_CALL, "884 actual two subsequent caller frames")
                .unwrap();
            b.reserve_array::<(usize, u64)>(&f, 2, "884 subsequent two roots")
                .unwrap();
            let foreign_root = vec![(root, 1)];
            let restored_root = vec![(root, 1)];
            if attack == 0 {
                let bytes = crate::bounded_output::active_remaining();
                let before = (b.remaining, b.skeleton_remaining);
                assert_eq!(
                    c.id(&wrong, foreign_root, &mut b).unwrap_err(),
                    Diagnostic::io("SPX-H006", String::new())
                );
                assert_eq!(bytes, crate::bounded_output::active_remaining());
                assert_eq!(before, (b.remaining, b.skeleton_remaining));
            } else if attack == 1 {
                let mut foreign = ReplayBudget::new();
                let bytes = crate::bounded_output::active_remaining();
                assert_eq!(
                    c.id(&f, foreign_root, &mut foreign).unwrap_err(),
                    Diagnostic::io("SPX-H006", String::new())
                );
                assert_eq!(bytes, crate::bounded_output::active_remaining());
            } else {
                let (_, child_refused, child_paid, child_first) =
                    with_limit_usage_refusal(1 << 28, || {
                        assert_eq!(
                            c.id(&f, foreign_root, &mut b).unwrap_err(),
                            Diagnostic::io("SPX-H006", String::new())
                        );
                    });
                assert!(!child_refused && child_first.is_none());
                assert_eq!(child_paid, 0);
            }
            assert_eq!(flags, (c.successor_unit_paid, c.successor_general_paid));
            assert_eq!(
                old_census::take().cells,
                [[[old_census::Cell::default(); 2]; 3]; 2]
            );
            let remaining = crate::bounded_output::active_remaining();
            assert_eq!(
                c.id(&f, restored_root, &mut b).unwrap_err(),
                Diagnostic::io("SPX-H006", String::new())
            );
            assert_eq!(remaining, crate::bounded_output::active_remaining());
            drop(observer);
        });
    }
}

#[test]
fn unwind_during_a_live_completed_owner_releases_all_rc_and_frame_backing() {
    // Initialize the thread's panic hook before observing compiler owners.
    // The runtime's first panic can retain thread-local backing independently
    // of either canonicalizer; every measured owner still must balance fully.
    let runtime = std::panic::catch_unwind(|| panic!("889 initialize thread panic hook"));
    assert!(runtime.is_err());
    drop(runtime);
    let f = function();
    let (a, sink, root) = graph(&f, 17, 1, false);
    for old in [true, false] {
        let ((caught, heap), refused, _, _) = with_limit_usage_refusal(64 * 1024 * 1024, || {
            measure(|| {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut b = ReplayBudget::new();
                    if old {
                        b.reserve_bytes(
                            &f,
                            super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + OLD_CALL,
                            "889 old unwind owner",
                        )
                        .unwrap();
                        let mut c = super::super::Canonicalizer::new(&a, sink);
                        b.reserve_array::<(usize, u64)>(&f, 1, "889 old unwind root")
                            .unwrap();
                        assert!(c.id(&f, vec![(root, 1)], &mut b).unwrap().is_some());
                        panic!("889 deliberate old live-owner unwind");
                    } else {
                        b.reserve_bytes(
                            &f,
                            Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                            "889 current unwind owner",
                        )
                        .unwrap();
                        let mut c = Canonicalizer::new(&a, sink);
                        b.reserve_array::<(usize, u64)>(&f, 1, "889 current unwind root")
                            .unwrap();
                        assert!(c.id(&f, vec![(root, 1)], &mut b).unwrap().is_some());
                        panic!("889 deliberate current live-owner unwind");
                    }
                }));
                let unwound = caught.is_err();
                drop(caught);
                unwound
            })
        });
        assert!(caught && !refused);
        assert!(!heap.invalid);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
    }
}

#[test]
fn cached_root_with_no_new_singleton_keeps_the_complete_additive_cold_bank() {
    let f = function();
    let (a, sink, root) = graph(&f, 4, 1, false);
    let mut charged = [0; 2];
    for old in [true, false] {
        let (cached, refused, _, _) = with_limit_usage_refusal(64 * 1024 * 1024, || {
            let mut b = ReplayBudget::new();
            if old {
                b.reserve_bytes(
                    &f,
                    super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * OLD_CALL,
                    "889 old cached owner",
                )
                .unwrap();
                let mut c = super::super::Canonicalizer::new(&a, sink);
                b.reserve_array::<(usize, u64)>(&f, 2, "889 old cached roots")
                    .unwrap();
                let first = c.id(&f, vec![(root, 1)], &mut b).unwrap();
                let root = vec![(root, 1)];
                let before = crate::bounded_output::active_remaining().unwrap();
                assert_eq!(c.id(&f, root, &mut b).unwrap(), first);
                before - crate::bounded_output::active_remaining().unwrap()
            } else {
                b.reserve_bytes(
                    &f,
                    Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * NEW_CALL,
                    "889 current cached owner",
                )
                .unwrap();
                let mut c = Canonicalizer::new(&a, sink);
                b.reserve_array::<(usize, u64)>(&f, 2, "889 current cached roots")
                    .unwrap();
                let first = c.id(&f, vec![(root, 1)], &mut b).unwrap();
                let root = vec![(root, 1)];
                let observer = shape_census::observe(&f, &b);
                let before = crate::bounded_output::active_remaining().unwrap();
                assert_eq!(c.id(&f, root, &mut b).unwrap(), first);
                let paid = before - crate::bounded_output::active_remaining().unwrap();
                let row = observer.finish(true);
                assert!(row.complete && !row.overflowed);
                assert_eq!(row.arity, [0; 3]);
                paid
            }
        });
        assert!(!refused);
        charged[usize::from(!old)] = cached;
    }
    assert_eq!(charged[1] - charged[0], inline_exit::PRIMITIVES);
    eprintln!(
        "OPT889 actual cached cold regression old={} current={} additive={}",
        charged[0],
        charged[1],
        inline_exit::PRIMITIVES
    );
}

#[test]
fn distinct_states_with_one_equal_language_exercise_real_signature_miss_then_hit() {
    let f = function();
    let mut a = Automaton::default();
    let mut setup = ReplayBudget::unobserved();
    let sink = a.node(&f, &mut setup).unwrap();
    let first = a.node(&f, &mut setup).unwrap();
    let second = a.node(&f, &mut setup).unwrap();
    let label = Label::End(super::super::super::SkeletonTerminal::Success);
    a.movement(&f, &mut setup, first, label.clone(), sink)
        .unwrap();
    a.movement(&f, &mut setup, second, label, sink).unwrap();
    assert_ne!(first, second);
    let (_, refused, _, _) = with_limit_usage_refusal(64 * 1024 * 1024, || {
        let mut x = ReplayBudget::new();
        let mut y = ReplayBudget::new();
        x.reserve_bytes(
            &f,
            super::super::Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * OLD_CALL,
            "889 signature old owner",
        )
        .unwrap();
        y.reserve_bytes(
            &f,
            Canonicalizer::CONSTRUCTOR_CONTROLS + 2 * NEW_CALL,
            "889 signature current owner",
        )
        .unwrap();
        let mut old = super::super::Canonicalizer::new(&a, sink);
        let mut current = Canonicalizer::new(&a, sink);
        let observer = shape_census::observe(&f, &y);
        let mut previous = None;
        for root in [first, second] {
            x.reserve_array::<(usize, u64)>(&f, 1, "889 signature old root")
                .unwrap();
            y.reserve_array::<(usize, u64)>(&f, 1, "889 signature current root")
                .unwrap();
            let expected = old.id(&f, vec![(root, 1)], &mut x).unwrap();
            let actual = current.id(&f, vec![(root, 1)], &mut y).unwrap();
            assert_eq!(actual, expected);
            assert!(actual.is_some());
            if let Some(prior) = previous {
                assert_eq!(actual, prior);
            }
            previous = Some(actual);
            assert!(old.ids.iter().eq(current.ids.iter()));
            assert!(old.states.iter().eq(current.states.iter()));
        }
        let row = observer.finish(true);
        assert!(row.complete && !row.overflowed);
        assert!(row.signature_hits > 0 && row.signature_misses > 0);
        eprintln!(
            "OPT889 actual distinct-state signature hits={} misses={}",
            row.signature_hits, row.signature_misses
        );
    });
    assert!(!refused);
}
