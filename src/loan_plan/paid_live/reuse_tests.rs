//! Actual scratch acquisition/Drop and complete independently compiled plans.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn same_plan(actual: &LoanPlan, expected: &LoanPlan) {
    let LoanPlan {
        schema,
        loans,
        endpoints,
        edges,
    } = actual;
    assert_eq!(schema, &expected.schema);
    assert_eq!(loans, &expected.loans);
    assert_eq!(endpoints, &expected.endpoints);
    assert_eq!(edges, &expected.edges);
    assert_eq!(actual, expected);
}

fn observe_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &LoanPlan,
    old: bool,
    limit: usize,
) -> (
    (bool, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    observe_plan_route(program, function, expected, if old { 2 } else { 0 }, limit)
}

// 0: current dense; 1: complete pre-dense pending-reuse receiver; 2: original.
fn observe_plan_route(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &LoanPlan,
    route: u8,
    limit: usize,
) -> (
    (bool, Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let payer = match CfgReservation::acquire() {
                Ok(payer) => payer,
                Err(_) => return false,
            };
            let result = build_route(program, function, &payer, route);
            match result {
                Ok(plan) => {
                    same_plan(&plan, expected);
                    drop(plan);
                    assert_eq!(payer.first(), None);
                    drop(payer);
                    true
                }
                Err(storage::PlannerError::Physical(first)) => {
                    assert_eq!(payer.first(), Some(first));
                    assert_eq!(payer.reserve(0), Err(first));
                    assert_eq!(payer.reserve(1), Err(first));
                    let before = current_metrics();
                    let retry = build_route(program, function, &payer, route);
                    assert!(
                        matches!(retry, Err(storage::PlannerError::Physical(repeated)) if repeated==first)
                    );
                    assert_eq!(payer.first(), Some(first));
                    assert_eq!(current_metrics().allocated, before.allocated);
                    drop(payer);
                    false
                }
                Err(storage::PlannerError::Semantic(error)) => {
                    panic!("positive fixture: {error:?}")
                }
            }
        })
    })
}

fn build_route(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
    route: u8,
) -> Result<LoanPlan, storage::PlannerError> {
    match route {
        0 => funded_flat::build(program, function, payer),
        1 => funded_flat::build_before_dense(program, function, payer),
        2 => funded_before::build(program, function, payer),
        _ => unreachable!("test route"),
    }
}

#[test]
fn complete_original_and_reused_liveness_planners_keep_all_fields_actual_payment_and_drop() {
    for count in [1, 3, 17] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let expected = saved_planner::build_plan(&program, function).unwrap();
        // Existing boundary corpus has one view and one byte_len call loan
        // per row, so even count1 genuinely executes two liveness visits.
        assert_eq!(expected.loans.len(), count * 2);
        let mut observations = Vec::new();
        for route in [0, 1, 2] {
            let ((ok, actual), refused, paid, first) =
                observe_plan_route(&program, function, &expected, route, usize::MAX);
            assert!(ok && !refused && first.is_none());
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            assert!(actual.allocated <= paid && actual.highwater <= paid);
            let ((ok, exact), refused, used, first) =
                observe_plan_route(&program, function, &expected, route, paid);
            assert!(ok && !refused && first.is_none() && used == paid);
            assert_eq!(exact, actual);
            for limit in [0, paid - 1] {
                let ((ok, short), refused, used, first) =
                    observe_plan_route(&program, function, &expected, route, limit);
                assert!(!ok && refused && first.is_some() && used <= limit);
                assert!(!short.invalid && short.live == 0 && short.allocated == short.freed);
                assert!(short.allocated <= used && short.highwater <= used);
            }
            observations.push((actual, paid));
        }
        let (current, paid) = observations[0];
        let (before_dense, before_dense_paid) = observations[1];
        let (old, old_paid) = observations[2];
        eprintln!("liveness count={count} dense={current:?}/{paid} pre_dense={before_dense:?}/{before_dense_paid} original={old:?}/{old_paid}");
        assert!(current.allocated < old.allocated && current.allocations < old.allocations);
        // The original strict pending-vector reuse contract stays independent
        // of the later dense receiving/carrier costs, including sparse inputs.
        assert!(
            before_dense.allocated < old.allocated && before_dense.allocations < old.allocations
        );
        assert!(before_dense_paid < old_paid);
        // Dense setup may move the debit frontier on small inputs. Require a
        // gain on the existing larger corpus; actual runtime must qualify it.
        if count == 17 {
            assert!(paid < old_paid);
        }
    }
}

fn fixture(width: usize, depth: usize) -> ResolvedProgram {
    let mut source = String::from("module live.scratch; @id(\"live.main\") fn main()->i64 {");
    for index in 0..width {
        source.push_str(&format!("let value{index}=1;"));
    }
    // Resolve shallow independent nodes, then move them into the deep HIR
    // input outside observations. P207/source admission is never widened.
    for index in 0..depth {
        source.push_str(&format!("let node{index}=1+1;"));
    }
    source.push_str("1}");
    let mut program =
        crate::hir::resolve(&crate::check(&source, "live-scratch.spx").unwrap()).unwrap();
    let ResolvedExprKind::Block { statements, tail } = &mut program.functions[0].body.kind else {
        panic!("fixture block")
    };
    for statement in statements.split_off(width) {
        let ResolvedStatement::Let { value, .. } = statement else {
            panic!("shallow binary")
        };
        let previous = std::mem::replace(tail, Box::new(value));
        let ResolvedExprKind::Binary { left, .. } = &mut tail.kind else {
            panic!("binary")
        };
        *left = previous;
    }
    program
}

fn visit(
    cfg: &Cfg<'_>,
    seeds: &cfg_directory::Set<u16>,
    reachable: &work::Reachability,
    expected: &[u16],
    repeats: usize,
    old: bool,
    limit: usize,
) -> (
    ((bool, usize), Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let payer = match CfgReservation::acquire() {
                Ok(payer) => payer,
                Err(_) => return (false, 0),
            };
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&payer);
            let mut pending = Vec::new();
            for _ in 0..repeats {
                let result = if old {
                    original::live_nodes(cfg, LoanId(0), 0, seeds, reachable, &mut work)
                } else {
                    live_nodes_with_pending(
                        cfg,
                        LoanId(0),
                        0,
                        seeds,
                        reachable,
                        &mut work,
                        &mut pending,
                    )
                };
                let result = result.and_then(|live| live.into_vec(&payer));
                match result {
                    Ok(values) => {
                        assert!(pending.is_empty());
                        assert_eq!(values, expected);
                        drop(values);
                    }
                    Err(error) => {
                        let first = payer
                            .first()
                            .expect("only physical refusal in positive visit");
                        assert_eq!(payer.reserve(0), Err(first));
                        assert_eq!(payer.reserve(1), Err(first));
                        let before = current_metrics();
                        let before_work = work.used;
                        let retry = if old {
                            original::live_nodes(cfg, LoanId(0), 0, seeds, reachable, &mut work)
                        } else {
                            live_nodes_with_pending(
                                cfg,
                                LoanId(0),
                                0,
                                seeds,
                                reachable,
                                &mut work,
                                &mut pending,
                            )
                        };
                        assert!(retry.is_err());
                        assert_eq!(payer.first(), Some(first));
                        assert_eq!(work.used, before_work);
                        assert_eq!(current_metrics().allocated, before.allocated);
                        drop(retry);
                        drop(error);
                        drop(pending);
                        drop(payer);
                        return (false, before_work);
                    }
                }
            }
            let used_work = work.used;
            drop(pending);
            drop(payer);
            (true, used_work)
        })
    })
}

#[test]
fn repeated_wide_deep_lifo_visits_charge_only_real_scratch_and_keep_exact_short_sticky_drop() {
    for (width, depth) in [(0, 0), (1, 1), (17, 32), (129, 64), (257, 128)] {
        let program = fixture(width, depth);
        let function = &program.functions[0];
        let ((cfg, seeds, reachable, expected), refused, _) = with_limit_usage(usize::MAX, || {
            let payer = CfgReservation::acquire().unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&payer);
            let cfg = build_cfg(function, &mut work).unwrap();
            let reachable = work::Reachability::build(&cfg, [0].into_iter(), &mut work).unwrap();
            let mut seeds = cfg_directory::Set::new(&payer).unwrap();
            for index in 0..cfg.points.len() {
                cfg_directory::set_insert(&mut seeds, index as u16, Some(&payer)).unwrap();
            }
            let live =
                original::live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut work).unwrap();
            let expected = live.into_vec(&payer).unwrap();
            (cfg, seeds, reachable, expected)
        });
        assert!(!refused);
        let repeats = 4;
        let mut observations = Vec::new();
        for old in [false, true] {
            let (((ok, semantic), actual), refused, paid, first) = visit(
                &cfg,
                &seeds,
                &reachable,
                &expected,
                repeats,
                old,
                usize::MAX,
            );
            assert!(ok && !refused && first.is_none());
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            assert!(actual.allocated <= paid && actual.highwater <= paid);
            let (((ok, exact_work), exact), refused, used, first) =
                visit(&cfg, &seeds, &reachable, &expected, repeats, old, paid);
            assert!(ok && !refused && first.is_none() && used == paid);
            assert_eq!(semantic, exact_work);
            assert_eq!(actual, exact);
            for limit in [0, paid - 1] {
                let (((ok, _), actual), refused, used, first) =
                    visit(&cfg, &seeds, &reachable, &expected, repeats, old, limit);
                assert!(!ok && refused && first.is_some() && used <= limit);
                assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
                assert!(actual.allocated <= used && actual.highwater <= used);
            }
            observations.push((semantic, actual, paid));
        }
        let (work, current, paid) = observations[0];
        let (old_work, old, old_paid) = observations[1];
        assert_eq!(work, old_work);
        assert!(current.allocated < old.allocated && current.allocations < old.allocations);
        assert!(paid < old_paid);
    }
}

#[test]
fn all_liveness_semantic_work_frontiers_keep_first_diagnostic_and_full_original_rows() {
    let program = fixture(4, 5);
    let function = &program.functions[0];
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let payer = CfgReservation::acquire().unwrap();
        let mut setup = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        setup.allocation = Some(&payer);
        let cfg = build_cfg(function, &mut setup).unwrap();
        let reachable = work::Reachability::build(&cfg, [0].into_iter(), &mut setup).unwrap();
        let mut seeds = cfg_directory::Set::new(&payer).unwrap();
        for index in 0..cfg.points.len() {
            cfg_directory::set_insert(&mut seeds, index as u16, Some(&payer)).unwrap();
        }
        let mut full = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        full.allocation = Some(&payer);
        let expected =
            original::live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut full).unwrap();
        let expected = expected.into_vec(&payer).unwrap();
        for limit in 0..=full.used {
            let mut old = WorkCounter::new(limit);
            old.allocation = Some(&payer);
            let mut current = WorkCounter::new(limit);
            current.allocation = Some(&payer);
            let before = original::live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut old);
            let mut pending = Vec::new();
            let actual = live_nodes_with_pending(
                &cfg,
                LoanId(0),
                0,
                &seeds,
                &reachable,
                &mut current,
                &mut pending,
            );
            assert_eq!(old.used, current.used);
            match (before, actual) {
                (Ok(before), Ok(actual)) => {
                    assert!(pending.is_empty());
                    assert_eq!(before.into_vec(&payer).unwrap(), expected);
                    assert_eq!(actual.into_vec(&payer).unwrap(), expected);
                }
                (Err(before), Err(actual)) => assert_eq!(before, actual),
                _ => panic!("semantic work frontier diverged"),
            }
            drop(pending);
            assert_eq!(payer.first(), None);
        }
    });
    assert!(!refused);
}

#[test]
fn complete_reborrow_and_owned_nested_fixture_plans_keep_original_fields_and_current_member_refusal(
) {
    for source in [
        include_str!("../../../tests/language/while_loops/record_borrow_renewal.spx"),
        include_str!("../../../tests/fixtures/nested-collection-records.spx"),
        include_str!("../../../tests/fixtures/nested-collection-string-record.spx"),
        include_str!("../../../tests/fixtures/owned-nested-outcome.spx"),
    ] {
        let program =
            crate::hir::resolve(&crate::check(source, "complete-live-fixture.spx").unwrap())
                .unwrap();
        for function in &program.functions {
            let expected = saved_planner::build_plan(&program, function).unwrap();
            for old in [false, true] {
                let ((ok, actual), refused, paid, first) =
                    observe_plan(&program, function, &expected, old, usize::MAX);
                assert!(ok && !refused && first.is_none());
                assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
                assert!(actual.allocated <= paid && actual.highwater <= paid);
            }
            let detached = function.clone();
            let ((), refused, _) = with_limit_usage(usize::MAX, || {
                let payer = CfgReservation::acquire().unwrap();
                let old = funded_before::build(&program, &detached, &payer);
                let current = funded_flat::build(&program, &detached, &payer);
                match (old, current) {
                    (
                        Err(storage::PlannerError::Semantic(old)),
                        Err(storage::PlannerError::Semantic(current)),
                    ) => assert_eq!(old, current),
                    _ => panic!("detached current-function admission changed"),
                }
                assert_eq!(payer.first(), None);
            });
            assert!(!refused);
        }
    }
}

#[test]
fn every_small_reused_pending_acquisition_frontier_keeps_the_first_receipt_through_drop() {
    let program = fixture(1, 1);
    let function = &program.functions[0];
    let ((cfg, seeds, reachable, expected), refused, _) = with_limit_usage(usize::MAX, || {
        let payer = CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(&payer);
        let cfg = build_cfg(function, &mut work).unwrap();
        let reachable = work::Reachability::build(&cfg, [0].into_iter(), &mut work).unwrap();
        let mut seeds = cfg_directory::Set::new(&payer).unwrap();
        for index in 0..cfg.points.len() {
            cfg_directory::set_insert(&mut seeds, index as u16, Some(&payer)).unwrap();
        }
        let expected = original::live_nodes(&cfg, LoanId(0), 0, &seeds, &reachable, &mut work)
            .unwrap()
            .into_vec(&payer)
            .unwrap();
        (cfg, seeds, reachable, expected)
    });
    assert!(!refused);
    for old in [false, true] {
        let (((ok, _), _), refused, paid, first) =
            visit(&cfg, &seeds, &reachable, &expected, 4, old, usize::MAX);
        assert!(ok && !refused && first.is_none());
        let mut limit = 0;
        while limit < paid {
            let (((ok, _), actual), refused, used, first) =
                visit(&cfg, &seeds, &reachable, &expected, 4, old, limit);
            assert!(!ok && refused && used <= limit);
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            assert!(actual.allocated <= used && actual.highwater <= used);
            let first = first.unwrap();
            let next = used.checked_add(first.requested).unwrap();
            assert!(next > limit && next <= paid);
            limit = next;
        }
        assert_eq!(limit, paid);
    }
}

#[test]
fn complete_original_helpers_planner_facade_and_all_source_inventory_hooks_remain_exact() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/paid-live-pending-reuse-3c351/whole-source-inverses.json"
    ))
    .unwrap();
    let inputs = [
        (
            include_str!("../paid_live.rs"),
            include_str!(
                "../../../docs/evidence/paid-live-pending-reuse-3c351/paid_live.rs.before.txt"
            ),
        ),
        (
            include_str!("../paid_plan.rs"),
            include_str!(
                "../../../docs/evidence/paid-live-pending-reuse-3c351/paid_plan.rs.before.txt"
            ),
        ),
        (
            include_str!(
                "../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"
            ),
            include_str!(
                "../../../docs/evidence/paid-live-pending-reuse-3c351/interop_source_inventory.rs.before.txt"
            ),
        ),
    ];
    for (index, (input, original)) in inputs.into_iter().enumerate() {
        let mut restored = input.to_owned();
        for hook in rows[index]["hooks"].as_array().unwrap() {
            let after = hook["after"].as_str().unwrap();
            let before = hook["before"].as_str().unwrap();
            assert_eq!(
                restored.matches(after).count(),
                hook["count"].as_u64().unwrap() as usize
            );
            restored = restored.replace(after, before);
        }
        assert_eq!(restored, original);
    }
    let old_live = include_str!("original.rs");
    let test_path = "#[path = \"tests.rs\"]\n";
    assert_eq!(old_live.matches(test_path).count(), 1);
    assert_eq!(old_live.replacen(test_path, "", 1), inputs[0].1);
    let before_plan = include_str!("../paid_plan/pre_live_reuse.rs");
    let plan_alias = "use super::super::paid_live::before_plan_live as paid_live;\n";
    assert_eq!(before_plan.matches(plan_alias).count(), 1);
    assert_eq!(before_plan.replacen(plan_alias, "", 1), inputs[1].1);
    let mut before_facade = include_str!("funded_before.rs").to_owned();
    let alias = "use super::before_plan as paid_plan;\n";
    assert_eq!(before_facade.matches(alias).count(), 1);
    before_facade = before_facade.replacen(alias, "", 1);
    for name in ["tests", "directories_tests", "work_boundary"] {
        let path = format!("#[path = \"../funded_flat/{name}.rs\"]\n");
        assert_eq!(before_facade.matches(&path).count(), 1);
        before_facade = before_facade.replacen(&path, "", 1);
    }
    assert_eq!(
        before_facade,
        include_str!(
            "../../../docs/evidence/paid-live-pending-reuse-3c351/funded_flat.rs.before.txt"
        )
    );
    let originals = concat!(
        include_str!("original.rs"),
        include_str!("tests.rs"),
        include_str!("../paid_plan/pre_live_reuse.rs"),
        include_str!("funded_before.rs"),
        include_str!("../funded_flat/tests.rs"),
        include_str!("../funded_flat/directories_tests.rs"),
        include_str!("../funded_flat/work_boundary.rs")
    );
    assert!(originals.contains(
        "paid_seed_live_and_edge_receivers_equal_old_full_rows_and_checked_work_frontiers"
    ));
    assert!(originals
        .contains("complete_projected_and_lexical_planner_uses_one_paid_owner_through_drop"));
    assert!(originals.contains(
        "complete_old_btree_receiver_constructor_drop_exact_short_and_original_applications"
    ));
}
