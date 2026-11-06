//! Actual scratch acquisition/Drop and complete independently compiled plans.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

type ObservedReuse = (
    ((bool, usize), Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
);

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
        1 => pending_funding::build(program, function, payer),
        2 => funded_before::build(program, function, payer),
        3 => funded_flat::build_before_dense(program, function, payer),
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
) -> ObservedReuse {
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
    let rows: serde_json::Value =
        serde_json::from_str(&crate::source_reflows_merge::receiver_fixture(
            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,
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
        let mut restored = crate::source_reflows_merge::receiver_fixture_entry(
            crate::source_reflows_merge::ReceiverFixture::PaidLiveReuse,
            rows[index]["source"].as_str().unwrap(),
            input,
        );
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
    let old_live = original_test_receiver(
        "src/loan_plan/paid_live/original.rs",
        include_str!("original.rs"),
    );
    let test_path = "#[path = \"tests.rs\"]\n";
    assert_eq!(old_live.matches(test_path).count(), 1);
    assert_eq!(old_live.replacen(test_path, "", 1), inputs[0].1);
    let before_plan = include_str!("../paid_plan/pre_live_reuse.rs");
    let plan_alias = "use super::super::paid_live::before_plan_live as paid_live;\n";
    assert_eq!(before_plan.matches(plan_alias).count(), 1);
    assert_eq!(before_plan.replacen(plan_alias, "", 1), inputs[1].1);
    let mut before_facade = original_test_receiver(
        "src/loan_plan/paid_live/funded_before.rs",
        include_str!("funded_before.rs"),
    );
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

// Only test-module lint/namespace glue separates these compiled receivers from
// their complete original bodies. Authenticate current bytes before inversion.
fn original_test_receiver(source: &str, compiled: &str) -> String {
    use sha2::{Digest, Sha256};
    let (length, current_hash, count, schema_import, old_length, old_hash) = match source {
        "src/loan_plan/paid_live/original.rs" => (
            2893,
            "53f1bf0c833c8bf88d391938c958e05b4210a78f7a26fd787cd7e06e0f27c210",
            1,
            false,
            2786,
            "9315163a396ddc3f3905dc8b56dd220ccbb6d852dd270d18f59a171308af4c26",
        ),
        "src/loan_plan/paid_live/funded_before.rs" => (
            3628,
            "9934c14303d48cf9f221065b9525628edc69919a65e8002e85444d03d7eee918",
            3,
            true,
            3264,
            "8acbf4f2aef5cb26524261c8ac27a9d0b6adce6334d43e11eb972fb88ef61401",
        ),
        _ => panic!("unknown complete receiver source"),
    };
    let mut original = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        source,
        current_hash,
        compiled,
    );
    assert_eq!(original.len(), length);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        current_hash
    );
    let lint = "#[allow(\n    clippy::duplicate_mod,\n    reason = \"Run the same controls against independent receivers.\"\n)]\n";
    assert_eq!(original.matches(lint).count(), count);
    original = original.replace(lint, "");
    if schema_import {
        let import = "use crate::loan_plan::LOAN_PLAN_SCHEMA_V3;\n";
        assert_eq!(original.matches(import).count(), 1);
        original = original.replacen(import, "", 1);
    }
    assert_eq!(original.len(), old_length);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        old_hash
    );
    original
}

#[test]
fn pending_reuse_funding_keeps_complete_immediate_facade() {
    use sha2::{Digest, Sha256};
    let original = include_str!("funded_before.rs");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "9934c14303d48cf9f221065b9525628edc69919a65e8002e85444d03d7eee918"
    );
    let mut expected = original.to_owned();
    for (before, after) in [
        (
            "use super::before_plan as paid_plan;",
            "use super::pending_plan as paid_plan;",
        ),
        (
            "use super::storage::{classify, PlannerError};",
            "use crate::loan_plan::storage::{classify, PlannerError};",
        ),
        ("use super::*;", "use crate::loan_plan::*;"),
    ] {
        assert_eq!(expected.matches(before).count(), 1);
        expected = expected.replacen(before, after, 1);
    }
    // The two shared harness registrations remain compiled on the original
    // receiver; this copy retains the complete funded body and work-limit seam.
    for registration in ["#[cfg(test)]\n#[path = \"../funded_flat/tests.rs\"]\n#[allow(\n    clippy::duplicate_mod,\n    reason = \"Run the same controls against independent receivers.\"\n)]\nmod tests;\n\n","#[cfg(test)]\n#[path = \"../funded_flat/directories_tests.rs\"]\n#[allow(\n    clippy::duplicate_mod,\n    reason = \"Run the same controls against independent receivers.\"\n)]\nmod directories_tests;\n\n"] {
        assert_eq!(expected.matches(registration).count(), 1);
        expected = expected.replacen(registration, "", 1);
    }
    let marker = "\n// BEGIN COMPLETE IMMEDIATE PENDING-REUSE FUNDING\n";
    let whole = include_str!("reuse_tests.rs");
    assert_eq!(whole.matches(marker).count(), 1);
    let source = whole.split_once(marker).unwrap().1;
    let source = source
        .split_once("\n// BEGIN COMPLETE IMMEDIATE POST-REUSE PLAN\n")
        .unwrap()
        .0;
    let source = source
        .strip_prefix("#[rustfmt::skip]\n#[path = \".\"]\nmod pending_funding {\n")
        .unwrap();
    let source = source.strip_suffix("}\n").unwrap();
    assert_eq!(
        source, expected,
        "whole original funded facade, only exact namespace binding"
    );
}

#[test]
fn pending_plan_keeps_complete_original_post_reuse_implementation() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/98db139e9b14abb2126a887f760e4db2bbbd095a9a2b89df924bef71e68732f3"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "98db139e9b14abb2126a887f760e4db2bbbd095a9a2b89df924bef71e68732f3"
    );
    let bridge = "\n#[cfg(test)]\n#[path = \"paid_plan/pre_live_reuse.rs\"]";
    assert_eq!(original.matches(bridge).count(), 1);
    let original = original.split_once(bridge).unwrap().0;
    assert_eq!(original.matches("use super::*;").count(), 1);
    let expected = original.replacen("use super::*;", "use crate::loan_plan::*;", 1);
    let marker = "\n// BEGIN COMPLETE IMMEDIATE POST-REUSE PLAN\n";
    let whole = include_str!("reuse_tests.rs");
    assert_eq!(whole.matches(marker).count(), 1);
    let source = whole.split_once(marker).unwrap().1;
    let source = source
        .strip_prefix("#[rustfmt::skip]\nmod pending_plan {\n")
        .unwrap();
    let source = source.strip_suffix("}\n").unwrap();
    assert_eq!(
        source, expected,
        "complete original post-reuse planner, only exact namespace binding"
    );
}

#[test]
fn retained_pre_dense_pipeline_keeps_all_fields_actual_payment_and_complete_drop() {
    for count in [1, 3, 17] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let expected = saved_planner::build_plan(&program, function).unwrap();
        let ((ok, observed), refused, paid, first) =
            observe_plan_route(&program, function, &expected, 3, usize::MAX);
        assert!(ok && !refused && first.is_none() && paid > 0);
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= paid && observed.highwater <= paid);
        let (exact, refused, used, first) =
            observe_plan_route(&program, function, &expected, 3, paid);
        assert_eq!(exact, (ok, observed));
        assert!(!refused && first.is_none() && used == paid);
        for limit in [0, paid - 1] {
            let ((ok, short), refused, used, first) =
                observe_plan_route(&program, function, &expected, 3, limit);
            assert!(!ok && refused && first.is_some() && used <= limit);
            assert!(!short.invalid && short.live == 0);
            assert_eq!(short.allocated, short.freed);
            assert!(short.allocated <= used && short.highwater <= used);
        }
    }
}

// BEGIN COMPLETE IMMEDIATE PENDING-REUSE FUNDING
#[rustfmt::skip]
#[path = "."]
mod pending_funding {
//! Private funding route for the authenticated canonical caller. Legacy
//! semantic diagnostics and upstream compiler preflight remain outside this
//! storage protocol; no whole-validator funding claim follows.
use super::pending_plan as paid_plan;
use crate::loan_plan::storage::{classify, PlannerError};
use crate::loan_plan::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::loan_plan::LOAN_PLAN_SCHEMA_V3;

pub(super) fn build(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
) -> Result<LoanPlan, PlannerError> {
    // Same immutable current subject, with no new source/profile authority.
    if !program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|current| std::ptr::eq(current, function))
    {
        return Err(PlannerError::Semantic(error(
            "funded loan planner requires the exact current function",
        )));
    }
    // Lexical preflight precedes the legacy guidance boundary. Its semantic
    // refusal must retain its original span/path/help, even for a work limit.
    let lexical = lexical_preflight(program, function, payer, &mut |program, region| {
        traversal::paid::inspect(program, region, payer).map(|proof| proof.is_some())
    })?;
    let result = (|| {
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        work.allocation = Some(payer);
        #[cfg(test)]
        work_boundary::apply(&mut work);
        if paid_candidate::inspect(program, function, payer, &mut work)? {
            paid_plan::build(program, function, &mut work)
        } else {
            let mut plan = LoanPlan::empty_v1();
            if lexical {
                plan.schema = LOAN_PLAN_SCHEMA_V3;
            }
            Ok(plan)
        }
    })();
    // Guidance is semantic rendering and may allocate. Never run it on a
    // physical marker or use its text to decide which receipt to publish.
    match classify(result, payer) {
        Err(PlannerError::Semantic(error)) => Err(PlannerError::Semantic(guidance::work_limit(
            error, function,
        ))),
        result => result,
    }
}

fn lexical_preflight(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
    inspect: &mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
) -> Result<bool, PlannerError> {
    classify(
        (|| {
            payer.reserve(0).map_err(|_| storage::marker())?;
            let lexical = crate::hir::lexical_exit::requires(function)?;
            if lexical {
                let owner =
                    crate::hir::lexical_exit::owner_with(program, function, &mut |bytes| {
                        payer.reserve(bytes).map_err(|_| storage::marker())
                    })?;
                crate::hir::lexical_exit::validate_function_with(
                    program, function, &owner, inspect,
                )?;
            }
            Ok(lexical)
        })(),
        payer,
    )
}

#[cfg(test)]
#[path = "../funded_flat/work_boundary.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod work_boundary;
}

// BEGIN COMPLETE IMMEDIATE POST-REUSE PLAN
#[rustfmt::skip]
mod pending_plan {
//! Complete unchanged loan ordering with private caller-paid inventories.
use crate::loan_plan::*;

pub(super) fn build(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<LoanPlan, Diagnostic> {
    let payer = work.allocation;
    let cfg = build_cfg(function, work)?;
    let mut aliases =
        cfg_directory::Map::<ValueId, Place>::new(payer.expect("private funded entry"))?;
    let mut bound =
        cfg_directory::Map::<ExpressionId, ValueId>::new(payer.expect("private funded entry"))?;
    let mut ownership = cfg_directory::Map::new(payer.expect("private funded entry"))?;
    for parameter in &function.params {
        cfg_directory::map_insert(
            &mut ownership,
            parameter.id.clone(),
            parameter.ownership,
            payer,
        )?;
    }
    for expression in &cfg.expressions {
        charge(work)?;
        native_view::inventory_aliases_paid(program, expression, &mut aliases, payer)?;
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                paid_patterns::inventory(&arm.pattern, &mut ownership, payer)?;
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    cfg_directory::map_insert(
                        &mut ownership,
                        binding.id.clone(),
                        binding.ownership,
                        payer,
                    )?;
                    if matches!(binding.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                        if let Some(place) = storage::expression_place(value, payer)? {
                            cfg_directory::map_insert(
                                &mut aliases,
                                binding.id.clone(),
                                place,
                                payer,
                            )?;
                            cfg_directory::map_insert(
                                &mut bound,
                                value.id.clone(),
                                binding.id.clone(),
                                payer,
                            )?;
                        }
                    }
                }
            }
        }
    }

    let mut drafts = Vec::<PaidDraft>::new();
    for expression in &cfg.expressions {
        charge(work)?;
        if let Some(draft) = traversal::paid::draft(program, expression, &cfg, work)? {
            storage::push(&mut drafts, draft, payer)?;
        }
        if let Some(draft) = traversal::paid::draft_bytes(expression, &cfg, &aliases, work)? {
            storage::push(&mut drafts, draft, payer)?;
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => paid_predicates::string_slot(expression)
                .map(|place| storage::clone_place(place, payer))
                .transpose()?,
            ResolvedExprKind::BorrowPlace { place, .. } => {
                Some(storage::clone_place(place, payer)?)
            }
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first()
                    .map(|source| storage::expression_place(source, payer))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::ByteRange { source, .. } => storage::expression_place(source, payer)?,
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args
                    .first()
                    .map(|source| storage::expression_place(source, payer))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(storage::clone_place(place, payer)?)
            }
            _ => None,
        };
        if let Some(place) = view {
            storage::push(
                &mut drafts,
                PaidDraft {
                    site: expression.id.clone(),
                    origin: origin::funded::resolve(
                        &aliases,
                        storage::clone_place(&place, payer)?,
                        work,
                    )?,
                    origin_span: expression.span,
                    parent_root: aliases
                        .contains_key(&place.root)
                        .then(|| place.root.clone()),
                    binding: bound.get(&expression.id).cloned(),
                    start: cfg.node(expression, LoanPointPhase::Before)?,
                    seeds: cfg_directory::Set::new(payer.expect("private funded entry"))?,
                    cause: if expression.ty == ResolvedType::Str {
                        LoanCause::StrView
                    } else {
                        LoanCause::SliceView
                    },
                },
                payer,
            )?;
        }
        for (index, argument) in
            call_arguments::borrowed_call_arguments_paid(program, expression, payer)?
        {
            let place = storage::expression_place(argument, payer)?
                .ok_or_else(|| error("borrowed call lacks an exact place origin"))?;
            storage::push(
                &mut drafts,
                PaidDraft {
                    site: expression.id.clone(),
                    origin: origin::funded::resolve(
                        &aliases,
                        storage::clone_place(&place, payer)?,
                        work,
                    )?,
                    origin_span: argument.span,
                    parent_root: aliases
                        .contains_key(&place.root)
                        .then(|| place.root.clone()),
                    binding: None,
                    start: cfg.node(expression, LoanPointPhase::Before)?,
                    seeds: cfg_directory::singleton_set(
                        cfg.node(expression, LoanPointPhase::After)?,
                        payer,
                    )?,
                    cause: LoanCause::BorrowedCall {
                        argument: u16::try_from(index)
                            .map_err(|_| error("borrowed argument index overflows"))?,
                    },
                },
                payer,
            )?;
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            arms,
        } = &expression.kind
        {
            let place = storage::expression_place(scrutinee, payer)?
                .ok_or_else(|| error("borrow match lacks an exact place origin"))?;
            let origin =
                origin::funded::resolve(&aliases, storage::clone_place(&place, payer)?, work)?;
            for (index, arm) in arms.iter().enumerate() {
                let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                storage::push(
                    &mut drafts,
                    PaidDraft {
                        site: expression.id.clone(),
                        origin: storage::clone_place(&origin, payer)?,
                        origin_span: scrutinee.span,
                        parent_root: aliases
                            .contains_key(&place.root)
                            .then(|| place.root.clone()),
                        binding: None,
                        start: cfg.node(entry, LoanPointPhase::Before)?,
                        seeds: cfg_directory::singleton_set(
                            cfg.node(&arm.value, LoanPointPhase::After)?,
                            payer,
                        )?,
                        cause: LoanCause::MatchBorrow {
                            arm: u16::try_from(index)
                                .map_err(|_| error("match arm index overflows"))?,
                        },
                    },
                    payer,
                )?;
            }
        }
    }

    // Independent HIR scope validation admits this exact result identity only
    // in ensures; it is held owned storage until contract settlement completes.
    drafts.retain(|draft| {
        ownership.get(&draft.origin.root) == Some(&OwnershipMode::Own)
            || provisional_result::root(function) == Some(&draft.origin.root)
    });
    // Borrow-only roots do not need an authenticated loan attachment. Do not
    // retain the transient CFG built to classify candidates: legacy HIR stays
    // heap-identical at this boundary and only real own-root loans carry the
    // endpoint/edge proof.
    if drafts.is_empty() {
        let mut plan = LoanPlan::empty_v1();
        if cfg
            .expressions
            .iter()
            .any(|e| matches!(e.kind, ResolvedExprKind::LexicalExit { .. }))
        {
            plan.schema = LOAN_PLAN_SCHEMA_V3;
        }
        return Ok(plan);
    }
    if drafts.len() > MAX_LOANS_PER_FUNCTION_V1 {
        return Err(error("function exceeds 256 shared loans"));
    }

    let mut binding_loans = cfg_directory::Map::new(payer.expect("private funded entry"))?;
    for (index, draft) in drafts.iter().enumerate() {
        if let Some(binding) = &draft.binding {
            cfg_directory::map_insert(
                &mut binding_loans,
                binding.clone(),
                LoanId(index as u16),
                payer,
            )?;
        }
    }
    for expression in &cfg.expressions {
        let ResolvedExprKind::Place(place) = &expression.kind else {
            continue;
        };
        if let Some(id) = binding_loans.get(&place.root) {
            cfg_directory::set_insert(
                &mut drafts[id.0 as usize].seeds,
                cfg.node(expression, LoanPointPhase::After)?,
                payer,
            )?;
        }
    }
    let match_parents = native_view::match_parents_funded(&cfg.expressions, &drafts, payer)?;
    let mut parents = storage::vector(drafts.len(), payer)?;
    for draft in &drafts {
        let parent = if let Some(root) = &draft.parent_root {
            match resolve_parent_paid(&aliases, &binding_loans, root.clone(), payer)? {
                Some(parent) => Some(parent),
                None => resolve_parent_paid(&aliases, &match_parents, root.clone(), payer)?,
            }
        } else {
            None
        };
        parents.push(parent);
    }
    #[cfg(not(test))]
    let reachable = work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?;
    #[cfg(test)]
    let reachable = if work.uncached_reachability {
        work::Reachability::uncached(&cfg, drafts.iter().map(|draft| draft.start), work)?
    } else {
        work::Reachability::build(&cfg, drafts.iter().map(|draft| draft.start), work)?
    };
    let mut live = storage::vector(drafts.len(), payer)?;
    let mut live_pending = Vec::new();
    for (index, draft) in drafts.iter().enumerate() {
        live.push(paid_live::live_nodes_with_pending(
            &cfg,
            LoanId(index as u16),
            draft.start,
            &draft.seeds,
            &reachable,
            work,
            &mut live_pending,
        )?);
    }
    for _ in 0..=drafts.len() {
        let mut changed = false;
        for child in (0..drafts.len()).rev() {
            let Some(parent) = parents[child] else {
                continue;
            };
            let parent = parent.0 as usize;
            let before = live[parent].len();
            let mut seeds = cfg_directory::clone_set(&drafts[parent].seeds, payer)?;
            for node in live[child].iter(payer.expect("private funded entry"))? {
                cfg_directory::set_insert(&mut seeds, *node, payer)?;
            }
            live[parent] = paid_live::live_nodes_with_pending(
                &cfg,
                LoanId(parent as u16),
                drafts[parent].start,
                &seeds,
                &reachable,
                work,
                &mut live_pending,
            )?;
            changed |= live[parent].len() != before;
        }
        if !changed {
            break;
        }
    }
    drop(live_pending);
    reject_cfg_overlaps_paid(program, &cfg, &aliases, &drafts, &live, work)?;

    let (mut edge_live, mut termination_edges) = paid_live::edge_liveness(&cfg, &live, work)?;
    // A contract/body root has no outgoing CFG edge. A synchronous call
    // ending at that root completes on its incoming After edge, rather than
    // needing a nonexistent successor. Only admit this formerly rejected
    // terminal shape; already admitted v1 plans retain identical bytes.
    if termination_edges.iter().any(Vec::is_empty) {
        let mut terminal_roots = cfg_directory::Set::new(payer.expect("private funded entry"))?;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            charge(work)?;
            cfg_directory::set_insert(
                &mut terminal_roots,
                cfg.node(root, LoanPointPhase::After)?,
                payer,
            )?;
        }
        for nodes in &mut live {
            for (index, point) in cfg.points.iter().enumerate() {
                charge(work)?;
                if point.phase == LoanPointPhase::After
                    && cfg.successor_edges(index).is_empty()
                    && terminal_roots.contains(&(index as u16))
                {
                    nodes.remove(&(index as u16), payer.expect("private funded entry"))?;
                }
            }
        }
        for (child, parent) in parents.iter().enumerate() {
            if let Some(parent) = parent {
                for node in live[child].iter(payer.expect("private funded entry"))? {
                    charge(work)?;
                    if !live[parent.0 as usize].contains(node) {
                        return Err(error("terminal loan child escapes its parent lifetime"));
                    }
                }
            }
        }
        // Recompute edge proof only for a previously rejected terminal shape.
        (edge_live, termination_edges) = paid_live::edge_liveness(&cfg, &live, work)?;
    }
    paid_materialize::materialize(
        cfg,
        drafts,
        parents,
        edge_live,
        termination_edges,
        payer.expect("private funded entry"),
    )
}

fn reject_cfg_overlaps_paid(
    program: &ResolvedProgram,
    cfg: &Cfg<'_>,
    aliases: &cfg_directory::Map<ValueId, Place>,
    drafts: &[PaidDraft],
    live: &[cfg_directory::Set<u16>],
    work: &mut WorkCounter,
) -> Result<(), Diagnostic> {
    let payer = work.allocation;
    let mut nonconsuming = cfg_directory::Set::new(payer.expect("private funded entry"))?;
    for expression in &cfg.expressions {
        for (_, argument) in
            call_arguments::borrowed_call_arguments_paid(program, expression, payer)?
        {
            cfg_directory::set_insert(&mut nonconsuming, argument.id.clone(), payer)?;
        }
        match &expression.kind {
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Borrow,
                scrutinee,
                ..
            } => {
                cfg_directory::set_insert(&mut nonconsuming, scrutinee.id.clone(), payer)?;
            }
            ResolvedExprKind::ByteRange { source, .. } => {
                cfg_directory::set_insert(&mut nonconsuming, source.id.clone(), payer)?;
            }
            _ => {}
        }
    }
    for expression in &cfg.expressions {
        charge(work)?;
        if let ResolvedExprKind::Place(place) = &expression.kind {
            if expression.ownership == OwnershipMode::Own && !nonconsuming.contains(&expression.id)
            {
                let place =
                    origin::funded::resolve(aliases, storage::clone_place(place, payer)?, work)?;
                reject_overlap_at(
                    cfg.node(expression, LoanPointPhase::Before)?,
                    &place,
                    drafts,
                    live,
                    expression.span,
                )?;
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field,
                    value,
                    span,
                } = statement
                {
                    let mut projections = Vec::new();
                    if let Some(field) = field {
                        storage::push(
                            &mut projections,
                            PlaceProjection::Field(field.clone()),
                            payer,
                        )?;
                    }
                    let place = origin::funded::resolve(
                        aliases,
                        Place {
                            root: binding.id.clone(),
                            projections,
                        },
                        work,
                    )?;
                    reject_overlap_at(
                        cfg.node(value, LoanPointPhase::After)?,
                        &place,
                        drafts,
                        live,
                        *span,
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn resolve_parent_paid(
    aliases: &cfg_directory::Map<ValueId, Place>,
    bindings: &cfg_directory::Map<ValueId, LoanId>,
    mut root: ValueId,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<Option<LoanId>, Diagnostic> {
    let mut seen = cfg_directory::Set::new(payer.expect("private funded entry"))?;
    while cfg_directory::set_insert(&mut seen, root.clone(), payer)? {
        if let Some(id) = bindings.get(&root) {
            return Ok(Some(*id));
        }
        let Some(alias) = aliases.get(&root) else {
            return Ok(None);
        };
        root = alias.root.clone();
    }
    Ok(None)
}
}
