//! Complete original/current CFG owners; allocator scopes include result Drop.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn same_cfg(current: &Cfg<'_>, old: &Cfg<'_>) {
    // Destructure every field: adding a CFG field requires extending this owner.
    let Cfg {
        points,
        nodes,
        expressions,
        edges,
        adjacency,
    } = current;
    assert_eq!(points, &old.points);
    assert_eq!(edges, &old.edges);
    assert_eq!(expressions, &old.expressions);
    assert_eq!(
        nodes.complete_len_for_test(),
        old.nodes.complete_len_for_test()
    );
    for point in points {
        assert_eq!(nodes.get(point), old.nodes.get(point));
    }
    assert!(adjacency.complete_storage_matches_for_test(&old.adjacency));
    for (left, right) in expressions.iter().zip(&old.expressions) {
        assert!(std::ptr::eq(*left, *right));
    }
}

fn same_saved_cfg(current: &Cfg<'_>, old: &saved_planner::Cfg<'_>) {
    let saved_planner::Cfg {
        points,
        nodes,
        expressions,
        edges,
        successors,
        successor_edges,
        predecessors,
    } = old;
    assert_eq!(&current.points, points);
    assert_eq!(&current.edges, edges);
    assert_eq!(&current.expressions, expressions);
    assert_eq!(current.nodes.complete_len_for_test(), nodes.len());
    assert_eq!(successors.len(), points.len());
    assert_eq!(successor_edges.len(), points.len());
    assert_eq!(predecessors.len(), points.len());
    for (ordinal, point) in points.iter().enumerate() {
        assert_eq!(current.nodes.get(point), nodes.get(point));
        assert!(current
            .successors(ordinal)
            .eq(successors[ordinal].iter().copied()));
        assert!(current
            .successor_edges(ordinal)
            .eq(successor_edges[ordinal].iter().copied()));
        assert_eq!(
            current.predecessors(ordinal),
            predecessors[ordinal].as_slice()
        );
    }
    for (left, right) in current.expressions.iter().zip(expressions) {
        assert!(std::ptr::eq(*left, *right));
    }
}

fn fixture_source(width: usize, depth: usize) -> String {
    let mut source = String::from("module cfg.scratch; @id(\"cfg.main\") fn main()->i64 {");
    for index in 0..width {
        source.push_str(&format!("let value{index} = 1;"));
    }
    // Alternating leaf/binary widths exercises retained spare capacity.
    source.push('1');
    for _ in 0..depth {
        source.push_str("+1");
    }
    source.push('}');
    source
}

fn fixture(width: usize, depth: usize) -> ResolvedProgram {
    let source = fixture_source(width, depth);
    if depth != 128 {
        return crate::hir::resolve(&crate::check(&source, "cfg-scratch.spx").unwrap()).unwrap();
    }
    // Preserve the complete prior deep source as an admitted-parser refusal.
    // This CFG owner exercises HIR, so create its deep positive input from
    // separately resolved shallow binary expressions instead of widening P207.
    let diagnostics = crate::check(&source, "cfg-scratch.spx").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-P207");
    let mut shallow = String::from("module cfg.scratch; @id(\"cfg.main\") fn main()->i64 {");
    for index in 0..width {
        shallow.push_str(&format!("let value{index} = 1;"));
    }
    for index in 0..depth {
        shallow.push_str(&format!("let scratch{index} = 1+1;"));
    }
    shallow.push_str("1}");
    let mut program =
        crate::hir::resolve(&crate::check(&shallow, "cfg-scratch.spx").unwrap()).unwrap();
    let ResolvedExprKind::Block { statements, tail } = &mut program.functions[0].body.kind else {
        panic!("resolved fixture remains a block");
    };
    // Move existing resolved nodes, retaining every binary/right-leaf identity
    // and the seed identity. No cloned/forged identities or extra CFG statements.
    for statement in statements.split_off(width) {
        let ResolvedStatement::Let { value, .. } = statement else {
            panic!("resolved shallow fixture declaration");
        };
        let previous = std::mem::replace(tail, Box::new(value));
        let ResolvedExprKind::Binary { left, .. } = &mut tail.kind else {
            panic!("resolved shallow fixture binary");
        };
        *left = previous;
    }
    assert_eq!(statements.len(), width);
    let mut cursor = tail.as_ref();
    for _ in 0..depth {
        let ResolvedExprKind::Binary { left, right, .. } = &cursor.kind else {
            panic!("deep CFG fixture retains every binary frontier");
        };
        assert!(matches!(right.kind, ResolvedExprKind::Int(1)));
        cursor = left;
    }
    assert!(matches!(cursor.kind, ResolvedExprKind::Int(1)));
    program
}

fn observe(
    function: &ResolvedFunction,
    expected: &Cfg<'_>,
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
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    return false;
                }
            };
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            let result = if old {
                original::build(function, &mut work)
            } else {
                build(function, &mut work)
            };
            match result {
                Ok(cfg) => {
                    same_cfg(&cfg, expected);
                    drop(cfg);
                    assert_eq!(owner.first(), None);
                    drop(owner);
                    true
                }
                Err(error) => {
                    let first = owner.first().expect("positive CFG only refuses physically");
                    assert_eq!(owner.reserve(0), Err(first));
                    assert_eq!(owner.reserve(1), Err(first));
                    let before = current_metrics();
                    let before_work = work.used;
                    let retry = if old {
                        original::build(function, &mut work)
                    } else {
                        build(function, &mut work)
                    };
                    assert!(retry.is_err());
                    assert_eq!(owner.first(), Some(first));
                    assert_eq!(work.used, before_work);
                    assert_eq!(current_metrics().allocated, before.allocated);
                    drop(retry);
                    drop(error);
                    drop(owner);
                    false
                }
            }
        })
    })
}

#[test]
fn complete_cfg_child_scratch_reuse_pays_actual_acquisition_exact_short_sticky_and_drop() {
    for (width, depth) in [(17, 32), (129, 64), (257, 1), (1, 128)] {
        let program = fixture(width, depth);
        let function = &program.functions[0];
        let (expected, refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&owner);
            original::build(function, &mut work).unwrap()
        });
        assert!(!refused);
        let ((ok, current), refused, paid, first) = observe(function, &expected, false, usize::MAX);
        let ((old_ok, old), old_refused, old_paid, old_first) =
            observe(function, &expected, true, usize::MAX);
        assert!(ok && old_ok && !refused && !old_refused && first.is_none() && old_first.is_none());
        assert!(current.allocated < old.allocated && current.allocations < old.allocations);
        assert!(paid < old_paid);
        for (old_route, actual, debit) in [(false, current, paid), (true, old, old_paid)] {
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            assert!(actual.allocated <= debit && actual.highwater <= debit);
            let ((ok, exact), refused, used, first) =
                observe(function, &expected, old_route, debit);
            assert!(ok && !refused && first.is_none() && used == debit);
            assert_eq!(exact, actual);
            for limit in [0, debit - 1] {
                let ((ok, short), refused, used, first) =
                    observe(function, &expected, old_route, limit);
                assert!(!ok && refused && first.is_some() && used <= limit);
                assert!(!short.invalid && short.live == 0 && short.allocated == short.freed);
                assert!(short.allocated <= used && short.highwater <= used);
            }
        }
    }
}

#[test]
fn complete_cfg_fields_order_and_semantic_work_frontiers_match_both_originals() {
    for (width, depth) in [(0, 0), (1, 1), (4, 5), (17, 32), (129, 64), (257, 1)] {
        let program = fixture(width, depth);
        for function in &program.functions {
            let ((), refused, _) = with_limit_usage(usize::MAX, || {
                let owner = CfgReservation::acquire().unwrap();
                let mut new_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                new_work.allocation = Some(&owner);
                let current = build(function, &mut new_work).unwrap();
                let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                old_work.allocation = Some(&owner);
                let old = original::build(function, &mut old_work).unwrap();
                same_cfg(&current, &old);
                assert_eq!(new_work.used, old_work.used);
                let saved = saved_planner::build_cfg(
                    function,
                    &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1),
                )
                .unwrap();
                same_saved_cfg(&current, &saved);
            });
            assert!(!refused);
        }
    }
    for count in [1, 3, 17] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            current_work.allocation = Some(&owner);
            let current = build(function, &mut current_work).unwrap();
            let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            old_work.allocation = Some(&owner);
            let old = original::build(function, &mut old_work).unwrap();
            same_cfg(&current, &old);
            assert_eq!(current_work.used, old_work.used);
        });
        assert!(!refused);
    }
    let program = fixture(2, 3);
    let function = &program.functions[0];
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut complete = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        complete.allocation = Some(&owner);
        drop(original::build(function, &mut complete).unwrap());
        for limit in 0..=complete.used {
            let mut current_work = WorkCounter::new(limit);
            current_work.allocation = Some(&owner);
            let current = build(function, &mut current_work);
            let mut old_work = WorkCounter::new(limit);
            old_work.allocation = Some(&owner);
            let old = original::build(function, &mut old_work);
            match (current, old) {
                (Ok(current), Ok(old)) => same_cfg(&current, &old),
                (Err(current), Err(old)) => assert_eq!(current, old),
                _ => panic!("CFG semantic work verdict changed"),
            }
            assert_eq!(current_work.used, old_work.used);
            assert_eq!(owner.first(), None);
        }
    });
    assert!(!refused);
}

#[test]
fn complete_cfg_duplicate_and_point_edge_bound_diagnostics_keep_old_priority() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let base = &program.functions[index];
    let mut cases = Vec::new();
    for position in 0..3 {
        let mut function = base.clone();
        match position {
            0 => function.requires.insert(0, function.body.clone()),
            1 => {
                function.requires.push(function.body.clone());
                function.requires.push(function.body.clone());
            }
            _ => function.ensures.push(function.body.clone()),
        }
        cases.push((function, "CFG contains a duplicate expression identity"));
    }
    let mut point_over = base.clone();
    boundary_tests::add_padding(
        &mut point_over,
        "cfg.point.over",
        MAX_LOAN_ENDPOINTS_V1 / 2,
        0,
        0,
    );
    cases.push((point_over, "function exceeds 4,096 loan program points"));
    let mut edge_over = base.clone();
    boundary_tests::add_padding(&mut edge_over, "cfg.edge.over", 0, 0, 380);
    cases.push((edge_over, "function exceeds 4,096 loan CFG edges"));
    for (function, message) in &cases {
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            current_work.allocation = Some(&owner);
            let current = build(function, &mut current_work)
                .err()
                .expect("hostile CFG refused");
            let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            old_work.allocation = Some(&owner);
            let old = original::build(function, &mut old_work)
                .err()
                .expect("whole original refused");
            assert_eq!(current, old);
            assert_eq!(current.message, *message);
            assert_eq!(current_work.used, old_work.used);
            assert_eq!(owner.first(), None);
        });
        assert!(!refused);
    }
}

#[test]
fn every_small_cfg_acquisition_frontier_preserves_sticky_first_refusal_and_complete_drop() {
    let program = fixture(2, 3);
    let function = &program.functions[0];
    let (expected, refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(&owner);
        original::build(function, &mut work).unwrap()
    });
    assert!(!refused);
    let ((ok, _), refused, paid, first) = observe(function, &expected, false, usize::MAX);
    assert!(ok && !refused && first.is_none());
    // Advance only by the actual failed acquisition, testing every finite
    // reservation frontier without guessing amounts or rewriting receipts.
    let mut limit = 0;
    while limit < paid {
        let ((ok, actual), refused, used, first) = observe(function, &expected, false, limit);
        assert!(!ok && refused && first.is_some() && used <= limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(actual.allocated <= used && actual.highwater <= used);
        let next = used.checked_add(first.unwrap().requested).unwrap();
        assert!(next > limit && next <= paid);
        limit = next;
    }
    let ((ok, actual), refused, used, first) = observe(function, &expected, false, paid);
    assert!(ok && !refused && first.is_none() && used == paid);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}

#[test]
fn whole_original_cfg_legacy_plan_and_child_projection_sources_remain_exact() {
    let proof: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../docs/evidence/paid-cfg-child-scratch/exact-source-inverses.json"
    ))
    .unwrap();
    let sources = [
        (
            "src/loan_plan/paid_cfg.rs",
            ci_str!("../paid_cfg.rs"),
            ci_str!(
                "../../../docs/evidence/paid-cfg-child-scratch/src__loan_plan__paid_cfg.rs.before.txt"
            ),
        ),
        (
            "src/loan_plan/paid_plan_legacy.rs",
            ci_str!("../paid_plan_legacy.rs"),
            ci_str!(
                "../../../docs/evidence/paid-cfg-child-scratch/src__loan_plan__paid_plan_legacy.rs.before.txt"
            ),
        ),
        (
            "src/loan_plan/adjacency.rs",
            ci_str!("../adjacency.rs"),
            ci_str!(
                "../../../docs/evidence/paid-cfg-child-scratch/src__loan_plan__adjacency.rs.before.txt"
            ),
        ),
        (
            "src/loan_plan/cfg_directory.rs",
            ci_str!("../cfg_directory.rs"),
            ci_str!(
                "../../../docs/evidence/paid-cfg-child-scratch/src__loan_plan__cfg_directory.rs.before.txt"
            ),
        ),
    ];
    for (path, receiving, original) in sources {
        let row = proof
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == path)
            .unwrap();
        let mut restored = receiving.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let current = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(current).count(), 1);
            restored = restored.replacen(current, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original, "complete original {path}");
    }
    assert_eq!(ci_str!("original.rs"), sources[0].2);
    let inventory = ci_str!(
        "../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
    );
    for path in [
        "src/loan_plan/paid_cfg.rs",
        "src/loan_plan/paid_cfg/original.rs",
        "src/loan_plan/paid_cfg/tests.rs",
    ] {
        assert_eq!(
            inventory
                .matches(&format!("include_str!(\"../../../../../{path}\")"))
                .count(),
            1
        );
    }
    let legacy = ci_str!("../paid_plan_legacy.rs");
    assert_eq!(
        legacy
            .matches("use super::paid_cfg::build_original as build_cfg;\n")
            .count(),
        1
    );
    assert!(ci_str!("../paid_cfg.rs").contains("original::build(function, work)"));
}
