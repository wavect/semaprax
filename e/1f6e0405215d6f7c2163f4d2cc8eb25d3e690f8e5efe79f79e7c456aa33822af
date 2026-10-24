//! Actual compiler-type controls; no replacement IR or layout definitions.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
fn fixture() -> ResolvedFunction {
    let source="module paired.controls; @id(\"probe\") fn probe()->i64 { 0 } @id(\"main\") fn main()->i64 {0}";
    let parsed = crate::check(source, "paired-controls.spx").unwrap();
    crate::hir::resolve(&parsed)
        .unwrap()
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap()
        .clone()
}
fn labels() -> [Label; 3] {
    [
        Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned("prefix.é\0tail".to_owned()),
            value: true,
        })),
        Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned("prefix.é\0tail".to_owned()),
            value: false,
        })),
        Label::End(SkeletonTerminal::Failure),
    ]
}
fn branching(f: &ResolvedFunction, duplicate: bool, mismatch: bool) -> BuiltComparison {
    let mut budget = ReplayBudget::new();
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let labels = labels();
    let b = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, b, labels[1].clone(), sink)
        .unwrap();
    let c = a.node(f, &mut budget).unwrap();
    a.movement(
        f,
        &mut budget,
        c,
        if duplicate {
            labels[1].clone()
        } else {
            labels[2].clone()
        },
        sink,
    )
    .unwrap();
    let left = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, left, labels[0].clone(), b)
        .unwrap();
    a.movement(f, &mut budget, left, labels[0].clone(), c)
        .unwrap();
    let joined = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, joined, labels[1].clone(), sink)
        .unwrap();
    if !mismatch {
        a.movement(
            f,
            &mut budget,
            joined,
            if duplicate {
                labels[1].clone()
            } else {
                labels[2].clone()
            },
            sink,
        )
        .unwrap();
    }
    let right = a.node(f, &mut budget).unwrap();
    a.movement(f, &mut budget, right, labels[0].clone(), joined)
        .unwrap();
    BuiltComparison {
        automaton: a,
        sink,
        hir_root: left,
        plan_root: right,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Outcome {
    kind: u8,
    remaining: usize,
    error: Option<&'static str>,
}
fn at_limit(
    f: &ResolvedFunction,
    b: &BuiltComparison,
    limit: usize,
    work: Option<usize>,
) -> (
    (Outcome, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    crate::bounded_output::with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            if let Some(work) = work {
                budget.remaining = work;
            }
            let result = budget
                .reserve_bytes(f, entry_controls(), "paired control entry")
                .and_then(|_| positive(f, b, &mut budget));
            let kind = match &result {
                Ok(Some(true)) => 1,
                Ok(None) => 0,
                Ok(Some(false)) => panic!("positive-only proof invented a mismatch"),
                Err(d) => {
                    assert_eq!(d.code, "SPX-H006");
                    assert_eq!(d.severity, crate::diagnostic::Severity::Error);
                    assert!(d.path.is_none() && d.span.is_none() && d.help.is_none());
                    2
                }
            };
            let out = Outcome {
                kind,
                remaining: budget.remaining,
                error: result.as_ref().err().map(|d| d.code),
            };
            drop(result);
            drop(budget);
            out
        })
    })
}
fn old_equal(f: &ResolvedFunction, b: &BuiltComparison) -> Option<bool> {
    let mut canonical =
        canonical_shared::inline_singleton::Canonicalizer::new(&b.automaton, b.sink);
    let mut budget = ReplayBudget::new();
    let left = canonical
        .id_for_comparison(f, vec![(b.hir_root, 1)], &mut budget)
        .unwrap()?;
    let right = canonical
        .id_for_comparison(f, vec![(b.plan_root, 1)], &mut budget)
        .unwrap()?;
    Some(left == right)
}
fn balanced(m: Metrics) {
    assert!(!m.invalid, "actual allocator observation invalid: {m:?}");
    assert_eq!(m.live, 0);
    assert_eq!(m.allocated, m.freed);
}
fn semantic_and_all_cuts(f: &ResolvedFunction, b: &BuiltComparison, expected: u8) {
    let ((out, m), refused, charged, _) = at_limit(f, b, 1 << 24, None);
    assert_eq!(out.kind, expected);
    assert!(!refused);
    balanced(m);
    assert!(
        m.allocated <= charged,
        "every requested allocation is paid: allocated={} charged={charged}",
        m.allocated
    );
    let ((exact, m), refused, paid, _) = at_limit(f, b, charged, None);
    assert_eq!(exact, out);
    assert_eq!(paid, charged);
    assert!(!refused);
    balanced(m);
    for limit in [0, 1, entry_controls().saturating_sub(1), charged - 1] {
        let ((out, m), refused, paid, receipt) = at_limit(f, b, limit, None);
        assert_eq!(out.kind, 2);
        assert!(refused);
        assert!(paid <= limit);
        assert!(receipt.is_some());
        assert!(m.allocated <= paid);
        balanced(m);
    }
    // Exhaust every integral byte cut through the actual composition, including
    // current bank, first backing, geometric growth and final publication.
    for limit in 0..charged {
        let ((out, m), refused, paid, receipt) = at_limit(f, b, limit, None);
        assert_eq!(out.kind, 2, "cut{limit}/{charged}");
        assert!(refused);
        assert!(paid <= limit);
        assert!(receipt.is_some());
        assert!(m.allocated <= paid);
        balanced(m);
    }
    if expected == 1 {
        let used = MAX_REPLAY_WORK_UNITS - out.remaining;
        let ((exact, m), _, _, _) = at_limit(f, b, 1 << 24, Some(used));
        assert_eq!(exact.kind, 1);
        assert_eq!(exact.remaining, 0);
        balanced(m);
        let ((short, m), _, _, _) = at_limit(f, b, 1 << 24, Some(used - 1));
        assert_eq!(short.kind, 2);
        balanced(m);
    }
    eprintln!(
        "PAIRED_CONTROL complete bound={charged} allocated={} work={}",
        m.allocated,
        MAX_REPLAY_WORK_UNITS - out.remaining
    );
}
fn scalar_cases(f: &ResolvedFunction) {
    for duplicate in [false, true] {
        let b = branching(f, duplicate, false);
        assert_eq!(old_equal(f, &b), Some(true));
        semantic_and_all_cuts(f, &b, 1);
    }
    let b = branching(f, false, true);
    assert_eq!(old_equal(f, &b), Some(false));
    semantic_and_all_cuts(f, &b, 0);
}
fn hostile_cases(f: &ResolvedFunction) {
    for case in 0..5 {
        let mut b = branching(f, false, false);
        match case {
            0 => b.hir_root = usize::MAX,
            1 => b.automaton.nodes[b.hir_root] = Node::Epsilon(usize::MAX),
            2 => b.automaton.nodes[b.hir_root] = Node::Epsilon(b.hir_root),
            3 => b.automaton.nodes[b.sink] = Node::Epsilon(b.hir_root),
            _ => {
                b.automaton.nodes.resize_with(MAX + 1, Node::default);
            }
        }
        let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
        assert_eq!(out.kind, 0);
        assert!(!refused);
        balanced(m);
    }
    let b = branching(f, false, false);
    let ((out, m), _, _, _) = at_limit(f, &b, 1 << 24, Some(0));
    assert_eq!(out.kind, 2);
    assert_eq!(out.remaining, 0);
    balanced(m);
    // Sticky physical refusal is checked on every root and warm frame use.
    crate::bounded_output::with_limit(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(f, entry_controls(), "paired sticky entry")
            .unwrap();
        let mut ledger =
            crate::bounded_output::classifier_string::ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        let bank = FrameBank {
            function: std::ptr::from_ref(f),
            budget: std::ptr::from_ref(&budget),
            ledger,
        };
        let mut w = Work {
            function: f,
            budget: &mut budget,
            bank,
        };
        w.frame(Frame::Compare).unwrap();
        assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
        assert!(w.frame(Frame::Compare).is_err());
        assert!(w.frame(Frame::Compare).is_err());
    });
    // A held invocation cannot spend the foreign ledger or revive on restore.
    crate::bounded_output::with_limit(1 << 20, || {
        let mut budget = ReplayBudget::new();
        budget
            .reserve_bytes(f, entry_controls(), "paired foreign entry")
            .unwrap();
        let mut ledger =
            crate::bounded_output::classifier_string::ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        let bank = FrameBank {
            function: std::ptr::from_ref(f),
            budget: std::ptr::from_ref(&budget),
            ledger,
        };
        let mut w = Work {
            function: f,
            budget: &mut budget,
            bank,
        };
        let (_, refused, charged) =
            crate::bounded_output::with_limit_usage(1 << 20, || assert!(w.bytes(1).is_err()));
        assert!(!refused);
        assert_eq!(charged, 0);
        assert!(w.bytes(0).is_err());
    });
}
fn overflow_and_width(f: &ResolvedFunction) {
    let mut budget = ReplayBudget::new();
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let mut root = sink;
    for _ in 0..64 {
        let next = a.node(f, &mut budget).unwrap();
        a.epsilon(f, &mut budget, next, root).unwrap();
        a.epsilon(f, &mut budget, next, root).unwrap();
        root = next;
    }
    let b = BuiltComparison {
        automaton: a,
        sink,
        hir_root: root,
        plan_root: root,
    };
    let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
    assert_eq!(out.kind, 0);
    assert!(!refused);
    balanced(m);
    let mut a = Automaton::default();
    let sink = a.node(f, &mut budget).unwrap();
    let left = a.node(f, &mut budget).unwrap();
    let right = a.node(f, &mut budget).unwrap();
    for i in 0usize..96 {
        let label = Label::Observation(Rc::new(SkeletonObservation::Boolean {
            expression: ExpressionId::from_owned(format!("wide.\0.é.{i}")),
            value: i.is_multiple_of(2),
        }));
        a.movement(f, &mut budget, left, label.clone(), sink)
            .unwrap();
        a.movement(f, &mut budget, right, label, sink).unwrap();
    }
    let b = BuiltComparison {
        automaton: a,
        sink,
        hir_root: left,
        plan_root: right,
    };
    assert_eq!(old_equal(f, &b), Some(true));
    let ((out, m), refused, _, _) = at_limit(f, &b, 1 << 24, None);
    assert_eq!(out.kind, 1);
    assert!(!refused);
    balanced(m);
}
#[test]
fn paired_complete_legacy_semantics_and_every_byte_work_cut() {
    scalar_cases(&fixture());
}
#[test]
fn paired_malformed_cycles_work_and_stale_foreign_sticky_receivers_fail_closed() {
    hostile_cases(&fixture());
}
#[test]
fn paired_duplicate_overflow_and_wide_utf8_nul_labels_keep_complete_order() {
    overflow_and_width(&fixture());
}
