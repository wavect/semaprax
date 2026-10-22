//! Authored owning controls; no execution or application saving is implied.
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, with_limit_usage_refusal_site};
use crate::cleanup_plan::replay::MAX_REPLAY_WORK_UNITS;
use crate::test_allocation_observer::{measure, Metrics};
// These are genuine new TEST caller transports, paid equally for both real
// old/current method calls; they confer no authority and do not pad old tariffs.
const ID_CALL_CONTROLS: usize = 2 * size_of::<(
    &mut Canonicalizer<'static>,
    &ResolvedFunction,
    Weighted,
    &mut ReplayBudget,
)>() + 2 * size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * size_of::<Weighted>();
fn function() -> ResolvedFunction {
    let p=crate::check("module lexical884; @id(\"f\") fn f(a:bool)->i64{if a{1}else{2}} @id(\"main\") fn main()->i64{f(true)}","lexical884.spx").unwrap();
    crate::hir::resolve(&p).unwrap().functions.remove(0)
}
fn graph(
    f: &ResolvedFunction,
    length: usize,
    branches: usize,
    cycle: bool,
) -> (Automaton, usize, usize) {
    let mut a = Automaton::default();
    let mut b = ReplayBudget::unobserved();
    let sink = a.node(f, &mut b).unwrap();
    let mut next = sink;
    for index in 0..length {
        let node = a.node(f, &mut b).unwrap();
        for branch in 0..branches {
            let terminal = if branch % 2 == 0 {
                super::super::super::SkeletonTerminal::Success
            } else {
                super::super::super::SkeletonTerminal::Failure
            };
            let child = if branch >= 2 && branch != 4 {
                let alternate = a.node(f, &mut b).unwrap();
                a.movement(
                    f,
                    &mut b,
                    alternate,
                    Label::End(super::super::super::SkeletonTerminal::Failure),
                    next,
                )
                .unwrap();
                alternate
            } else {
                next
            };
            a.movement(f, &mut b, node, Label::End(terminal), child)
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
    span: Option<crate::ast::Span>,
    path: Option<([u8; 2048], usize)>,
    help: Option<([u8; 2048], usize)>,
}
fn text(s: &str) -> ([u8; 2048], usize) {
    assert!(s.len() <= 2048);
    let mut b = [0; 2048];
    b[..s.len()].copy_from_slice(s.as_bytes());
    (b, s.len())
}
fn failure(d: Diagnostic) -> Failure {
    Failure {
        code: d.code,
        severity: d.severity,
        message: text(&d.message),
        span: d.span,
        path: d.path.as_deref().map(text),
        help: d.help.as_deref().map(text),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    result: Result<Option<usize>, Failure>,
    ordinary: usize,
    skeleton: usize,
    spent: usize,
    sticky: Option<Failure>,
    report: census::Report,
}
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "The complete saved control retains its original expressions for independent parity checks. The independently compiled control retains its exact tuple layout and receiving types."
)]
fn observe(
    old: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    floor: usize,
    work: usize,
) -> (
    (Outcome, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
    Option<(&'static str, u32)>,
) {
    let (value, refused, paid, receipt, site) = with_limit_usage_refusal_site(limit, || {
        measure(|| {
            let mut b = ReplayBudget::with_skeleton_limit(work);
            b.remaining = work;
            let observer = census::observe(f, &b);
            let _phase = census::phase(true);
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = (|| {
                if old {
                    b.reserve_bytes(
                        f,
                        before_lexical_successors::Canonicalizer::CONSTRUCTOR_CONTROLS
                            + ID_CALL_CONTROLS,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = before_lexical_successors::Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "884 root input")?;
                    c.id(f, vec![(root, weight)], &mut b)
                } else {
                    b.reserve_bytes(
                        f,
                        Canonicalizer::CONSTRUCTOR_CONTROLS + ID_CALL_CONTROLS,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "884 root input")?;
                    c.id(f, vec![(root, weight)], &mut b)
                }
            })();
            let (result, sticky) = match result {
                Ok(value) => (Ok(value), None),
                Err(error) => {
                    let selected = failure(error);
                    let bytes = crate::bounded_output::active_remaining();
                    let work = (b.remaining, b.skeleton_remaining, b.skeleton_spent);
                    let allocated = crate::test_allocation_observer::current_metrics().allocated;
                    let ledger =
                        crate::bounded_output::classifier_string::ClassifierLedger::capture();
                    let first = ledger.first_refusal();
                    // A semantic cycle diagnostic does not poison a budget. Only
                    // a real work/physical refusal has the sticky empty retry.
                    if !b.refusal_spent && first.is_none() {
                        (Err(selected), None)
                    } else {
                        let retry = failure(b.reserve_bytes(f, 0, "884 sticky retry").unwrap_err());
                        assert_eq!(bytes, crate::bounded_output::active_remaining());
                        assert_eq!(first, ledger.first_refusal());
                        assert_eq!(work, (b.remaining, b.skeleton_remaining, b.skeleton_spent));
                        assert_eq!(
                            allocated,
                            crate::test_allocation_observer::current_metrics().allocated
                        );
                        assert_eq!(retry, failure(Diagnostic::io("SPX-H006", String::new())));
                        (Err(selected), Some(retry))
                    }
                }
            };
            let value = Outcome {
                result,
                ordinary: b.remaining,
                skeleton: b.skeleton_remaining,
                spent: b.skeleton_spent,
                sticky,
                report: census::take(),
            };
            drop(observer);
            drop(b);
            crate::bounded_output::clear_active_floor();
            value
        })
    });
    assert!(!value.1.invalid);
    assert_eq!(value.1.live, 0);
    assert_eq!(value.1.allocated, value.1.freed);
    assert!(value.1.allocated <= paid && value.1.highwater <= paid);
    (
        value,
        refused,
        paid,
        receipt,
        site.map(|p| (p.file(), p.line())),
    )
}
#[test]
fn complete_immediate_canonicalizers_keep_every_state_signature_work_cut_cycle_and_overflow() {
    let f = function();
    for length in [0, 1, 4, 17] {
        for branches in [0, 1, 2, 3, 7] {
            if branches >= 3 && length > 4 {
                continue;
            }
            for weight in [0, 1, 2, u64::MAX] {
                let (a, sink, root) = graph(&f, length, branches, false);
                for work in [0, 1, 7, 31, MAX_REPLAY_WORK_UNITS] {
                    // Real work refusals poison their own invocation; compare separate
                    // complete ledgers rather than let old's refusal poison current.
                    let old = observe(true, &f, &a, sink, root, weight, 1 << 29, 0, work);
                    let current = observe(false, &f, &a, sink, root, weight, 1 << 29, 0, work);
                    assert_eq!(
                        old.0 .0.result, current.0 .0.result,
                        "all six work failures and canonical IDs"
                    );
                    assert_eq!(
                        (old.0 .0.ordinary, old.0 .0.skeleton, old.0 .0.spent),
                        (
                            current.0 .0.ordinary,
                            current.0 .0.skeleton,
                            current.0 .0.spent
                        )
                    );
                }
                with_limit_usage_refusal(1 << 29, || {
                    let mut x = ReplayBudget::new();
                    let mut y = ReplayBudget::new();
                    x.reserve_bytes(
                        &f,
                        before_lexical_successors::Canonicalizer::CONSTRUCTOR_CONTROLS
                            + ID_CALL_CONTROLS,
                        "884 actual old constructor and method callers",
                    )
                    .unwrap();
                    y.reserve_bytes(
                        &f,
                        Canonicalizer::CONSTRUCTOR_CONTROLS + ID_CALL_CONTROLS,
                        "884 actual current constructor and method callers",
                    )
                    .unwrap();
                    let mut old = before_lexical_successors::Canonicalizer::new(&a, sink);
                    let mut current = Canonicalizer::new(&a, sink);
                    x.reserve_array::<(usize, u64)>(&f, 1, "884 old root")
                        .unwrap();
                    y.reserve_array::<(usize, u64)>(&f, 1, "884 current root")
                        .unwrap();
                    assert_eq!(
                        old.id(&f, vec![(root, weight)], &mut x),
                        current.id(&f, vec![(root, weight)], &mut y)
                    );
                    old.assert_same(&current);
                });
            }
        }
    }
    let (a, sink, root) = graph(&f, 1, 1, true);
    let old = observe(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        1 << 28,
        0,
        MAX_REPLAY_WORK_UNITS,
    );
    let current = observe(
        false,
        &f,
        &a,
        sink,
        root,
        1,
        1 << 28,
        0,
        MAX_REPLAY_WORK_UNITS,
    );
    assert_eq!(old.0 .0.result, current.0 .0.result);
    assert!(current.0 .0.result.is_err());
}
#[test]
fn actual_lexical_successor_reports_full_old_current_tuples_and_typed_break_even_without_credit() {
    let f = function();
    assert_eq!(ORIGINAL_RECEIVING, borrowed_labels::one_pass_receiving());
    for length in [0, 1, 4, 64] {
        for weight in [1, 2] {
            let (a, sink, root) = graph(&f, length, 2, false);
            let old = observe_matched(
                true,
                &f,
                &a,
                sink,
                root,
                weight,
                1 << 29,
                0,
                MAX_REPLAY_WORK_UNITS,
            );
            let current = observe_matched(
                false,
                &f,
                &a,
                sink,
                root,
                weight,
                1 << 29,
                0,
                MAX_REPLAY_WORK_UNITS,
            );
            assert_eq!(old.0 .0.result, current.0 .0.result);
            assert_eq!(
                (old.0 .0.ordinary, old.0 .0.skeleton, old.0 .0.spent),
                (
                    current.0 .0.ordinary,
                    current.0 .0.skeleton,
                    current.0 .0.spent
                )
            );
            assert!(!old.1 && !current.1);
            assert_eq!(old.0.1,current.0.1,"same real allocations, reallocations, peak and complete Drop; the candidate removes control frames");
            assert!(old.0 .0.report.complete && current.0 .0.report.complete);
            let old_cells = old.0 .0.report.cells[0][1];
            let cells = current.0 .0.report.cells[1][1];
            let n = cells.iter().map(|c| c.attempts).sum::<usize>();
            let m = cells.iter().map(|c| c.primitive_banks).sum::<usize>();
            assert_eq!(n, old_cells.iter().map(|c| c.attempts).sum::<usize>());
            assert!(m <= 2);
            let a = Canonicalizer::CONSTRUCTOR_CONTROLS
                .checked_sub(before_lexical_successors::Canonicalizer::CONSTRUCTOR_CONTROLS)
                .unwrap();
            let overhead = a + n * PER_USE_SELECTION;
            let gross = (n - m) * PRIMITIVE_FRAMES;
            assert_eq!(
                old.2.checked_add(overhead).unwrap(),
                current.2.checked_add(gross).unwrap(),
                "complete real bills reconcile without old padding/removed compound credit"
            );
            if gross > overhead {
                assert!(current.2 < old.2);
            }
            eprintln!("OPT884 length={length} weight={weight} N={n} M={m} P={PRIMITIVE_FRAMES} A={a} R={PER_USE_SELECTION} old/current paid={}/{} allocated={}/{} peak={}/{} work={}/{} census={:?}/{:?}",old.2,current.2,old.0.1.allocated,current.0.1.allocated,old.0.1.highwater,current.0.1.highwater,old.0.0.ordinary,current.0.0.ordinary,old.0.0.report,current.0.0.report);
        }
    }
}
fn primitive(
    f: &ResolvedFunction,
    requested: usize,
    remaining: usize,
    floor: usize,
) -> (Failure, Failure) {
    let ((first, sticky), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
        let mut b = ReplayBudget::new();
        b.prepare_builder(f).unwrap();
        let available = crate::bounded_output::active_remaining().unwrap();
        assert!(crate::bounded_output::reserve_active_required(
            available - remaining
        ));
        assert!(crate::bounded_output::set_active_floor(floor));
        let first = failure(
            b.reserve_bytes(f, requested, "factored one-pass label call receivers")
                .unwrap_err(),
        );
        let sticky = failure(b.reserve_bytes(f, 0, "884 sticky retry").unwrap_err());
        drop(b);
        crate::bounded_output::clear_active_floor();
        (first, sticky)
    });
    assert!(refused);
    let receipt = receipt.unwrap();
    assert_eq!(
        (receipt.requested, receipt.remaining, receipt.floor),
        (requested, remaining, floor)
    );
    (first, sticky)
}
#[test]
fn actual_cold_warm_unit_general_banks_exact_short_zero_floor_match_primitive_six_fields_and_drop()
{
    let f = function();
    for weight in [1, 2] {
        let (a, sink, root) = graph(&f, 4, 1, false);
        for requested in [COLD_RECEIVING, WARM_RECEIVING] {
            let mut limit = 0;
            let mut prefix = None;
            for _ in 0..10000 {
                let run = observe(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    limit,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                );
                let receipt = run.3.expect(
                    "finite staircase reaches real acquisition before successful completion",
                );
                let branch = usize::from(weight == 1);
                if receipt.requested == requested
                    && run.0 .0.report.cells[1][1][branch].refused == 1
                {
                    prefix = Some(run.2);
                    break;
                }
                limit = run.2.checked_add(receipt.requested).unwrap();
            }
            let prefix = prefix.expect("actual unit/general cold/warm bank must execute");
            let exact = prefix.checked_add(requested).unwrap();
            assert!(
                observe(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    exact,
                    0,
                    MAX_REPLAY_WORK_UNITS
                )
                .2 >= exact
            );
            for (limit, floor, remaining) in [
                (prefix, 0, 0),
                (exact - 1, 0, requested - 1),
                (exact, 1, requested),
            ] {
                let run = observe(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    limit,
                    floor,
                    MAX_REPLAY_WORK_UNITS,
                );
                assert!(run.1);
                assert_eq!(run.2, prefix);
                let receipt = run.3.unwrap();
                assert_eq!(
                    (receipt.requested, receipt.remaining, receipt.floor),
                    (requested, remaining, floor)
                );
                let expected = primitive(&f, requested, remaining, floor);
                assert_eq!(run.0 .0.result.as_ref().err(), Some(&expected.0));
                assert_eq!(run.0 .0.sticky.as_ref(), Some(&expected.1));
                let again = observe(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    limit,
                    floor,
                    MAX_REPLAY_WORK_UNITS,
                );
                assert_eq!(
                    (
                        &run.0 .0.result,
                        run.0 .0.ordinary,
                        run.0 .0.skeleton,
                        run.0 .0.spent,
                        &run.0 .0.sticky
                    ),
                    (
                        &again.0 .0.result,
                        again.0 .0.ordinary,
                        again.0 .0.skeleton,
                        again.0 .0.spent,
                        &again.0 .0.sticky
                    )
                );
                assert_eq!(
                    (run.0 .1, run.1, run.2, run.3, run.4),
                    (again.0 .1, again.1, again.2, again.3, again.4)
                );
                assert_eq!(run.0 .0.report.cells, again.0 .0.report.cells);
                assert!(run.0 .0.report.complete && again.0 .0.report.complete);
            }
        }
    }
}
#[test]
fn complete_original_sources_and_all_primitive_site_banks_remain_separate() {
    assert_eq!(
        PRIMITIVE_FRAMES,
        24 * ONE_PASS_UNIT_ABI + 5 * ONE_PASS_NODE_ABI
    );
    assert_eq!(COLD_RECEIVING - WARM_RECEIVING, PRIMITIVE_FRAMES);
    assert_eq!(
        COLD_RECEIVING,
        borrowed_labels::one_pass_receiving() + PER_USE_SELECTION
    );
    let before = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/opt884-lexical-successor-owner-20261017/canonical_shared.a6c.before.rs.txt"
    ));
    assert_eq!(before.matches("return borrowed_labels::one_pass_successors(function, budget, self.automaton, closed, accepted);").count(),2);
    let lexical = include_str!("../lexical_successors.rs");
    assert!(!lexical.contains("borrowed_labels::one_pass_successors("));
    assert!(!lexical.contains("successors_with_receiving("));
    assert_eq!(
        lexical.matches("$this.successor_unit_paid = true;").count(),
        1
    );
    assert_eq!(
        lexical
            .matches("$this.successor_general_paid = true;")
            .count(),
        1
    );
}

#[test]
fn warm_lexical_banks_cannot_authenticate_foreign_function_budget_or_nested_restored_invocation() {
    let f = function();
    let wrong = f.clone();
    let (a, sink, root) = graph(&f, 4, 1, false);
    for attack in 0..3 {
        with_limit_usage_refusal(1 << 28, || {
            let mut b = ReplayBudget::new();
            b.reserve_bytes(
                &f,
                Canonicalizer::CONSTRUCTOR_CONTROLS + ID_CALL_CONTROLS,
                "884 actual warm constructor and callers",
            )
            .unwrap();
            let mut c = Canonicalizer::new(&a, sink);
            b.reserve_array::<(usize, u64)>(&f, 1, "884 warm root")
                .unwrap();
            assert!(c.id(&f, vec![(root, 1)], &mut b).unwrap().is_some());
            let flags = (c.successor_unit_paid, c.successor_general_paid);
            let observer = census::observe(&f, &b);
            b.reserve_bytes(
                &f,
                2 * ID_CALL_CONTROLS,
                "884 actual two subsequent caller frames",
            )
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
            assert_eq!(census::take().cells, [[[census::Cell::default(); 2]; 3]; 2]);
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
fn whole_shipping_comparison_records_separate_hir_plan_banks_and_complete_old_work_and_drop() {
    use super::super::super::lexical_plan_tests;
    for source in [lexical_plan_tests::WHILE, lexical_plan_tests::GENERATED] {
        let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(source);
        fixture.with_linked_context(|program|{
            let functions=program.functions();let f=functions.iter().find(|f|f.id.as_str()=="probe").unwrap();
            let mut records=Vec::new();
            for old in [true,false] {
                let ((record,metrics),refused,paid,first)=with_limit_usage_refusal(1<<29,||measure(||{
                    let mut b=ReplayBudget::new();let observer=census::observe(f,&b);
                    let result=if old { before_lexical_successors::compare::compare_with_index_storage(program,f,&mut b,None,None) }
                        else { super::super::super::compare_with_index_storage(program,f,&mut b,None,None) };
                    assert_eq!(result,Ok(Some(true)));
                    let record=(b.remaining,b.skeleton_remaining,b.skeleton_spent,census::take());drop(observer);drop(b);record
                }));
                assert!(!refused && first.is_none());assert!(!metrics.invalid && metrics.live==0 && metrics.allocated==metrics.freed);
                assert!(metrics.allocated<=paid && metrics.highwater<=paid);assert!(record.3.complete);
                eprintln!("OPT884 whole production immediate={old} work={}/{}/{} paid={paid} physical={metrics:?} census={:?}",record.0,record.1,record.2,record.3);
                records.push(record);
            }
            assert_eq!((records[0].0,records[0].1,records[0].2),(records[1].0,records[1].1,records[1].2));
        });
    }
}

#[test]
fn equal_observation_contents_with_distinct_rc_keep_complete_immediate_grouping() {
    let f = function();
    let mut a = Automaton::default();
    let mut setup = ReplayBudget::unobserved();
    let sink = a.node(&f, &mut setup).unwrap();
    let root = a.node(&f, &mut setup).unwrap();
    let observation = super::super::super::SkeletonObservation::Boolean {
        expression: crate::hir::ExpressionId::from_owned("884.equal.content".into()),
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
    for weight in [1, 2, u64::MAX] {
        for work in [0, 1, 31, MAX_REPLAY_WORK_UNITS] {
            let old = observe(true, &f, &a, sink, root, weight, 1 << 28, 0, work);
            let current = observe(false, &f, &a, sink, root, weight, 1 << 28, 0, work);
            assert_eq!(old.0 .0.result, current.0 .0.result);
            assert_eq!(
                (old.0 .0.ordinary, old.0 .0.skeleton, old.0 .0.spent),
                (
                    current.0 .0.ordinary,
                    current.0 .0.skeleton,
                    current.0 .0.spent
                )
            );
        }
    }
}

// Complete original observer, selecting only the whole matched frame route.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "The complete saved control retains its original expressions for independent parity checks. The independently compiled control retains its exact tuple layout and receiving types."
)]
fn observe_matched(
    old: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    floor: usize,
    work: usize,
) -> (
    (Outcome, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
    Option<(&'static str, u32)>,
) {
    let (value, refused, paid, receipt, site) = with_limit_usage_refusal_site(limit, || {
        measure(|| {
            let mut b = ReplayBudget::with_skeleton_limit(work);
            b.remaining = work;
            let observer = census::observe(f, &b);
            let _phase = census::phase(true);
            assert!(crate::bounded_output::set_active_floor(floor));
            let result = (|| {
                if old {
                    b.reserve_bytes(
                        f,
                        before_lexical_successors::Canonicalizer::CONSTRUCTOR_CONTROLS
                            + ID_CALL_CONTROLS,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = before_lexical_successors::Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "884 root input")?;
                    c.id(f, vec![(root, weight)], &mut b)
                } else {
                    b.reserve_bytes(
                        f,
                        Canonicalizer::CONSTRUCTOR_CONTROLS + ID_CALL_CONTROLS,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "884 root input")?;
                    c.matched_884_id(f, vec![(root, weight)], &mut b)
                }
            })();
            let (result, sticky) = match result {
                Ok(value) => (Ok(value), None),
                Err(error) => {
                    let selected = failure(error);
                    let bytes = crate::bounded_output::active_remaining();
                    let work = (b.remaining, b.skeleton_remaining, b.skeleton_spent);
                    let allocated = crate::test_allocation_observer::current_metrics().allocated;
                    let ledger =
                        crate::bounded_output::classifier_string::ClassifierLedger::capture();
                    let first = ledger.first_refusal();
                    // A semantic cycle diagnostic does not poison a budget. Only
                    // a real work/physical refusal has the sticky empty retry.
                    if !b.refusal_spent && first.is_none() {
                        (Err(selected), None)
                    } else {
                        let retry = failure(b.reserve_bytes(f, 0, "884 sticky retry").unwrap_err());
                        assert_eq!(bytes, crate::bounded_output::active_remaining());
                        assert_eq!(first, ledger.first_refusal());
                        assert_eq!(work, (b.remaining, b.skeleton_remaining, b.skeleton_spent));
                        assert_eq!(
                            allocated,
                            crate::test_allocation_observer::current_metrics().allocated
                        );
                        assert_eq!(retry, failure(Diagnostic::io("SPX-H006", String::new())));
                        (Err(selected), Some(retry))
                    }
                }
            };
            let value = Outcome {
                result,
                ordinary: b.remaining,
                skeleton: b.skeleton_remaining,
                spent: b.skeleton_spent,
                sticky,
                report: census::take(),
            };
            drop(observer);
            drop(b);
            crate::bounded_output::clear_active_floor();
            value
        })
    });
    assert!(!value.1.invalid);
    assert_eq!(value.1.live, 0);
    assert_eq!(value.1.allocated, value.1.freed);
    assert!(value.1.allocated <= paid && value.1.highwater <= paid);
    (
        value,
        refused,
        paid,
        receipt,
        site.map(|p| (p.file(), p.line())),
    )
}
#[test]
fn composed_current_lexical_successor_adds_the_exact_later_arity_places_to_the_original_equation() {
    let f = function();
    assert_eq!(ORIGINAL_RECEIVING, borrowed_labels::one_pass_receiving());
    for length in [0, 1, 4, 64] {
        for weight in [1, 2] {
            let (a, sink, root) = graph(&f, length, 2, false);
            let old = observe(
                true,
                &f,
                &a,
                sink,
                root,
                weight,
                1 << 29,
                0,
                MAX_REPLAY_WORK_UNITS,
            );
            let current = observe(
                false,
                &f,
                &a,
                sink,
                root,
                weight,
                1 << 29,
                0,
                MAX_REPLAY_WORK_UNITS,
            );
            assert_eq!(old.0 .0.result, current.0 .0.result);
            assert_eq!(
                (old.0 .0.ordinary, old.0 .0.skeleton, old.0 .0.spent),
                (
                    current.0 .0.ordinary,
                    current.0 .0.skeleton,
                    current.0 .0.spent
                )
            );
            assert!(!old.1 && !current.1);
            assert_eq!(old.0.1,current.0.1,"same real allocations, reallocations, peak and complete Drop; the candidate removes control frames");
            assert!(old.0 .0.report.complete && current.0 .0.report.complete);
            let old_cells = old.0 .0.report.cells[0][1];
            let cells = current.0 .0.report.cells[1][1];
            let n = cells.iter().map(|c| c.attempts).sum::<usize>();
            let m = cells.iter().map(|c| c.primitive_banks).sum::<usize>();
            assert_eq!(n, old_cells.iter().map(|c| c.attempts).sum::<usize>());
            assert!(m <= 2);
            let root_is_general = root == sink || !a.nodes[root].moves().is_empty();
            assert!(root_is_general);
            let a = Canonicalizer::CONSTRUCTOR_CONTROLS
                .checked_sub(before_lexical_successors::Canonicalizer::CONSTRUCTOR_CONTROLS)
                .unwrap();
            // This fixture has one root and only arity0/2 signatures.
            // At length0 that root IS the distinguished sink, which the
            // cold-dead guard excludes even at unit weight. Thus every
            // fixture here enters one general frame-loop activation.
            let activations = usize::from(root_is_general);
            let later = activations * Canonicalizer::BORROWED_EXIT_LOCAL;
            assert_eq!(
                Canonicalizer::BORROWED_EXIT_LOCAL,
                2 * size_of::<usize>() + 4 * size_of::<bool>()
            );
            let overhead = a + n * PER_USE_SELECTION + later;
            let gross = (n - m) * PRIMITIVE_FRAMES;
            assert_eq!(
                old.2.checked_add(overhead).unwrap(),
                current.2.checked_add(gross).unwrap(),
                "complete real bills reconcile without old padding/removed compound credit"
            );
            if gross > overhead {
                assert!(current.2 < old.2);
            }
            eprintln!("OPT884 length={length} weight={weight} N={n} M={m} P={PRIMITIVE_FRAMES} A={a} R={PER_USE_SELECTION} old/current paid={}/{} allocated={}/{} peak={}/{} work={}/{} census={:?}/{:?}",old.2,current.2,old.0.1.allocated,current.0.1.allocated,old.0.1.highwater,current.0.1.highwater,old.0.0.ordinary,current.0.0.ordinary,old.0.0.report,current.0.0.report);
        }
    }
}
#[test]
fn matched_884_keeps_the_whole_original_equation_and_observer_with_only_route_glue() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/lexical-successor-matched-884-20261018/before/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs"));
    let current = include_str!("tests.rs");
    let marker = "#[test]\nfn actual_lexical_successor_reports_full_old_current_tuples_and_typed_break_even_without_credit()";
    let old = before[before.find(marker).unwrap()..]
        .split("\nfn primitive(")
        .next()
        .unwrap();
    let actual = current[current.find(marker).unwrap()..]
        .split("\nfn primitive(")
        .next()
        .unwrap();
    assert_eq!(
        actual,
        old.replace("let old = observe(", "let old = observe_matched(")
            .replace("let current = observe(", "let current = observe_matched(")
    );
    let root = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/lexical-successor-matched-884-20261018/historical-9debb-canonical_shared.rs.txt"));
    let start = root.find("    pub(super) fn id(").unwrap();
    let end = root[start..]
        .find("    #[cfg(test)]\n    fn successors(")
        .unwrap()
        + start;
    let expected = root[start..end]
        .trim_end()
        .replace(
            "pub(super) fn id(",
            "pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn matched_884_id(",
        )
        .replace(
            "self.id_with_receiving(",
            "self.before_borrowed_single_id_with_receiving(",
        );
    let frame = include_str!("frame_loop/before_borrowed_single.rs");
    let start = frame.find("    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn matched_884_id(").unwrap();
    assert_eq!(
        frame[start..].trim_end().strip_suffix("\n}").unwrap(),
        expected
    );
}

#[test]
fn whole_884_observer_keeps_the_old_type_id_and_only_routes_the_current_type_to_matched_frame() {
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/lexical-successor-matched-884-20261018/before/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs"));
    let start = before.find("fn observe(").unwrap();
    let end = before[start..].find("\n#[test]").unwrap() + start;
    let expected = before[start..end].replacen("fn observe(", "fn observe_matched(", 1);
    let split = expected.find("                } else {").unwrap();
    let expected = format!(
        "{}{}",
        &expected[..split],
        expected[split..].replacen(
            "c.id(f, vec![(root, weight)], &mut b)",
            "c.matched_884_id(f, vec![(root, weight)], &mut b)",
            1
        )
    );
    let current = include_str!("tests.rs");
    let start = current.find("fn observe_matched(").unwrap();
    let end = current[start..].find("\n#[test]").unwrap() + start;
    assert_eq!(
        &current[start..end],
        expected,
        "both complete original observer arms; old and current types remain independently compiled"
    );
}
