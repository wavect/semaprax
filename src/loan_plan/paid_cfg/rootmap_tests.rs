//! Full prior/current CFG controls for removal of one redundant identity tree.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture(width: usize, depth: usize) -> ResolvedProgram {
    let mut source = String::from("module cfg.rootmap; @id(\"cfg.main\") fn main()->i64 {");
    for i in 0..width {
        source.push_str(&format!("let value{i}=1;"));
    }
    source.push('1');
    for _ in 0..depth {
        source.push_str("+1");
    }
    source.push('}');
    crate::hir::resolve(&crate::check(&source, "cfg-rootmap.spx").unwrap()).unwrap()
}
fn same_cfg(a: &Cfg<'_>, b: &Cfg<'_>) {
    let Cfg {
        points,
        nodes,
        expressions,
        edges,
        adjacency,
    } = a;
    assert_eq!(points, &b.points);
    assert_eq!(edges, &b.edges);
    assert_eq!(expressions, &b.expressions);
    assert_eq!(
        nodes.complete_len_for_test(),
        b.nodes.complete_len_for_test()
    );
    for point in points {
        assert_eq!(nodes.get(point), b.nodes.get(point));
    }
    assert!(adjacency.complete_storage_matches_for_test(&b.adjacency));
    for (left, right) in expressions.iter().zip(&b.expressions) {
        assert!(std::ptr::eq(*left, *right));
    }
}
fn run<'a>(
    f: &'a ResolvedFunction,
    work: &mut WorkCounter,
    old: bool,
) -> Result<Cfg<'a>, Diagnostic> {
    if old {
        seen_before::build(f, work)
    } else {
        build(f, work)
    }
}
#[test]
fn rootmap_identity_membership_preserves_complete_old_cfg_and_every_work_boundary() {
    for (width, depth) in [(0, 0), (17, 32), (129, 1)] {
        let program = fixture(width, depth);
        let function = &program.functions[0];
        let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            old_work.allocation = Some(&owner);
            let old = seen_before::build(function, &mut old_work).unwrap();
            let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            current_work.allocation = Some(&owner);
            let current = build(function, &mut current_work).unwrap();
            same_cfg(&current, &old);
            assert_eq!(current_work.used, old_work.used);
            for limit in 0..=old_work.used {
                let mut a = WorkCounter::new(limit);
                a.allocation = Some(&owner);
                let mut b = WorkCounter::new(limit);
                b.allocation = Some(&owner);
                let current = build(function, &mut a);
                let old = seen_before::build(function, &mut b);
                assert_eq!(a.used, b.used);
                match (current, old) {
                    (Ok(a), Ok(b)) => same_cfg(&a, &b),
                    (Err(a), Err(b)) => assert_eq!(a, b, "complete work refusal receipt"),
                    _ => panic!("complete old/current outcome differs at work limit {limit}"),
                }
            }
            assert_eq!(owner.first(), None);
        });
        assert!(!refused);
        assert_eq!(first, None);
    }
}
#[test]
fn rootmap_identity_membership_keeps_duplicate_identity_first_error_and_work_order() {
    let mut program = fixture(2, 3);
    let function = &mut program.functions[0];
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let ResolvedExprKind::Binary { left, right, .. } = &mut tail.kind else {
        panic!("binary")
    };
    right.id = left.id.clone();
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        for limit in [0, 1, 2, 3, 4, MAX_LOAN_PLAN_WORK_V1] {
            let mut a = WorkCounter::new(limit);
            a.allocation = Some(&owner);
            let mut b = WorkCounter::new(limit);
            b.allocation = Some(&owner);
            let current = build(function, &mut a)
                .err()
                .expect("duplicate or earlier work refusal");
            let old = seen_before::build(function, &mut b)
                .err()
                .expect("full old refusal");
            assert_eq!(
                current, old,
                "all diagnostic fields at competing work/identity errors"
            );
            assert_eq!(a.used, b.used);
            if limit == MAX_LOAN_PLAN_WORK_V1 {
                assert_eq!(
                    current.message,
                    "CFG contains a duplicate expression identity"
                );
            }
            assert_eq!(owner.first(), None);
        }
    });
    assert!(!refused);
    assert_eq!(first, None);
}
fn observe(
    function: &ResolvedFunction,
    old: bool,
    limit: usize,
) -> (
    (bool, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let owner = match CfgReservation::acquire() {
                Ok(owner) => owner,
                Err(_) => {
                    assert!(!reserve_active_required(0));
                    return false;
                }
            };
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            match run(function, &mut work, old) {
                Ok(cfg) => {
                    drop(cfg);
                    assert_eq!(owner.first(), None);
                    drop(owner);
                    true
                }
                Err(error) => {
                    let first = owner
                        .first()
                        .expect("positive fixture only physically refuses");
                    let at_stop = current_metrics().allocated;
                    assert_eq!(owner.reserve(0), Err(first));
                    assert_eq!(owner.reserve(1), Err(first));
                    let used = work.used;
                    let retry = run(function, &mut work, old);
                    assert!(retry.is_err());
                    assert_eq!(work.used, used);
                    assert_eq!(owner.first(), Some(first));
                    drop(retry);
                    drop(error);
                    drop(owner);
                    assert_eq!(
                        current_metrics().allocated,
                        at_stop,
                        "no allocation after selected error or during Drop"
                    );
                    false
                }
            }
        })
    })
}
/// Growth steps of a caller-paid Vec pushed one value at a time from capacity
/// 0: `storage::push` grows to `max(2 * capacity, 4)` on each full push.
fn growths(len: usize) -> usize {
    let (mut capacity, mut steps) = (0usize, 0usize);
    while capacity < len {
        capacity = (capacity * 2).max(4);
        steps += 1;
    }
    steps
}
#[test]
fn rootmap_identity_membership_removes_one_actual_box_per_expression_and_funds_drop() {
    for (width, depth) in [(0, 0), (17, 32), (129, 1)] {
        let program = fixture(width, depth);
        let function = &program.functions[0];
        let ((count, edges), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            let cfg = build(function, &mut work).unwrap();
            (cfg.expressions.len(), cfg.edges.len())
        });
        assert!(!refused);
        let ((old_ok, old), old_refused, old_paid, _) = observe(function, true, usize::MAX);
        let ((ok, current), refused, exact, first) = observe(function, false, usize::MAX);
        assert!(old_ok && ok && !old_refused && !refused);
        assert_eq!(first, None);
        // d70e4af22 keeps one shared discovery/phase-pair AVL node per
        // expression where the old builder held four (seen set, root map and
        // two point nodes), moves each expression's root into a Vec, and
        // collects edges in a Vec instead of one AVL node per unique edge plus
        // the exact drained edge Vec. Every Vec grows one value at a time from
        // capacity 0, one observed (re)allocation per growth step. These
        // straight-line fixtures push no duplicate edge, so the pushed edge
        // count equals the published unique count.
        assert_eq!(
            old.allocations - current.allocations,
            3 * count + edges + 1 - growths(count) - growths(edges),
            "removed AVL Boxes per expression and edge net of the new Vec growth"
        );
        assert!(old.allocated > current.allocated && old_paid > exact);
        for metrics in [old, current] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(current.allocated <= exact);
        assert!(old.allocated <= old_paid);
        for cap in [0, exact - 1, exact] {
            let ((ok, metrics), refused, used, first) = observe(function, false, cap);
            assert_eq!(ok, cap == exact);
            assert_eq!(refused, cap != exact);
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            if cap == exact {
                assert_eq!(used, exact);
                assert_eq!(first, None);
                assert!(metrics.allocated <= used);
            } else {
                assert!(first.is_some());
                if cap == 0 {
                    assert_eq!(metrics.allocated, 0);
                }
            }
        }
    }
}
