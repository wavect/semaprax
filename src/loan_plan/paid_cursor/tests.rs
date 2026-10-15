//! Full old/current discovery, planner, first failure and actual paid backing.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
#[path = "funded_before.rs"]
mod funded_before;
fn scalar(wide: bool) -> ResolvedProgram {
    let mut source = String::from("module cursor; @id(\"cursor.main\") fn main()->i64 {");
    if wide {
        for i in 0..80 {
            source.push_str(&format!("let v{i}={i};"));
        }
    }
    source.push_str("0}");
    crate::hir::resolve(&crate::check(&source, "cursor.spx").unwrap()).unwrap()
}
fn old_order(function: &ResolvedFunction) -> Vec<&ResolvedExpr> {
    let roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some(expression) = pending.pop() {
        result.push(expression);
        push_children_old(expression, &mut pending);
    }
    result
}
fn push_children_old<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    paid_children::push_children(expression, pending, None).unwrap();
}
#[test]
fn full_old_discovery_order_and_count_are_kept_across_inline_spill_and_root_reset() {
    for wide in [false, true] {
        let p = scalar(wide);
        let mut function = p.functions[0].clone();
        function.requires.push(function.body.clone());
        function.ensures.push(function.body.clone());
        let old = old_order(&function);
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut cursor = Cursor::new(&function, &owner);
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            for pass in 1..=3 {
                for expected in &old {
                    assert!(std::ptr::eq(
                        cursor.next(&mut work).unwrap().unwrap(),
                        *expected
                    ));
                }
                assert!(cursor.next(&mut work).unwrap().is_none());
                assert_eq!(work.used, pass * old.len());
                cursor.reset().unwrap();
            }
        });
        assert!(!refused);
    }
}
#[test]
fn cursor_spill_is_prepaid_exact_short_sticky_and_reused_through_drop() {
    let p = scalar(true);
    let function = &p.functions[0];
    let old = old_order(function);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let mut cursor = Cursor::new(function, &owner);
                let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
                let result = (|| {
                    for expected in &old {
                        assert!(std::ptr::eq(cursor.next(&mut work)?.unwrap(), *expected));
                    }
                    assert!(cursor.next(&mut work)?.is_none());
                    cursor.reset()?;
                    let before = current_metrics();
                    for expected in &old {
                        assert!(std::ptr::eq(cursor.next(&mut work)?.unwrap(), *expected));
                    }
                    assert!(cursor.next(&mut work)?.is_none());
                    assert_eq!(current_metrics(), before);
                    Ok::<_, Diagnostic>(())
                })();
                let success = result.is_ok();
                if let Err(error) = result {
                    assert_eq!(error, storage::marker());
                    let first = owner.first().unwrap();
                    let before = current_metrics();
                    assert_eq!(owner.reserve(0), Err(first));
                    assert!(cursor.next(&mut work).is_err());
                    assert!(cursor.reset().is_err());
                    assert_eq!(current_metrics(), before);
                    drop(error);
                }
                drop(cursor);
                drop(owner);
                success
            })
        })
    };
    let ((ok, full), refused, exact, first) = run(usize::MAX);
    assert!(ok && !refused && first.is_none() && !full.invalid);
    assert!(full.allocated <= exact && full.highwater <= exact);
    assert_eq!(full.allocated, full.freed);
    assert_eq!(full.live, 0);
    let ((ok, again), refused, used, first) = run(exact);
    assert!(ok && !refused && first.is_none());
    assert_eq!(used, exact);
    assert_eq!(again, full);
    let ((ok, short), refused, used, first) = run(exact - 1);
    assert!(!ok && refused && first.is_some() && !short.invalid);
    assert!(used < exact);
    assert!(short.allocated <= used && short.highwater <= used);
    assert_eq!(short.allocated, short.freed);
    assert_eq!(short.live, 0);
}
#[test]
fn every_cursor_pass_spends_the_same_existing_work_allowance() {
    let p = scalar(true);
    let f = &p.functions[0];
    let count = old_order(f).len();
    let ((), refused, _) = with_limit_usage(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let mut cursor = Cursor::new(f, &owner);
        let mut work = WorkCounter::new(2 * count);
        for _ in 0..2 {
            while cursor.next(&mut work).unwrap().is_some() {}
            cursor.reset().unwrap();
        }
        assert_eq!(work.used, 2 * count);
        let error = cursor.next(&mut work).unwrap_err();
        assert_eq!(work.used, 2 * count + 1);
        assert_eq!(
            error,
            super::super::error("loan analysis exceeds 1,000,000 checked work")
        );
        assert!(cursor.next(&mut work).is_err());
        assert!(cursor.reset().is_err());
    });
    assert!(!refused);
}
#[test]
fn complete_saved_planner_and_candidate_match_all_plans_and_unchanged_plan_work() {
    for count in [1, 3] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let f = &program.functions[index];
        let frozen = saved_planner::build_plan(&program, f).unwrap();
        let discovery_count = old_order(f).len();
        let ((), refused, _) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let old_order = old_order(f);
            let mut cursor = Cursor::new(f, &owner);
            let mut discovery_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            for expected in &old_order {
                assert!(std::ptr::eq(
                    cursor.next(&mut discovery_work).unwrap().unwrap(),
                    *expected
                ));
            }
            assert!(cursor.next(&mut discovery_work).unwrap().is_none());
            assert_eq!(discovery_work.used, old_order.len());
            drop(cursor);
            let old = funded_before::build(&program, f, &owner)
                .unwrap_or_else(|_| panic!("old full planner refused"));
            let current = funded_flat::build(&program, f, &owner)
                .unwrap_or_else(|_| panic!("current full planner refused"));
            assert_eq!(old, frozen);
            assert_eq!(current, frozen);
            assert!(paid_candidate_before::inspect(&program, f, &owner).unwrap());
            let mut current_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            current_work.allocation = Some(&owner);
            assert!(paid_candidate::inspect(&program, f, &owner, &mut current_work).unwrap());
            let candidate_work = current_work.used;
            assert!(candidate_work >= 2 * discovery_count);
            let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            old_work.allocation = Some(&owner);
            let old_plan = paid_plan::build(&program, f, &mut old_work).unwrap();
            let current_plan = paid_plan::build(&program, f, &mut current_work).unwrap();
            assert_eq!(old_plan, frozen);
            assert_eq!(current_plan, frozen);
            assert_eq!(current_work.used - candidate_work, old_work.used);
        });
        assert!(!refused);
    }
}
#[test]
fn full_old_current_planner_backing_includes_receiver_drop_and_no_expression_materialization() {
    for wide in [false, true] {
        let program = scalar(wide);
        let function = &program.functions[0];
        let run = |old| {
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let owner = CfgReservation::acquire().unwrap();
                    let result = if old {
                        funded_before::build(&program, function, &owner)
                    } else {
                        funded_flat::build(&program, function, &owner)
                    };
                    let plan = result.unwrap_or_else(|_| panic!("scalar full planner refused"));
                    assert!(plan.loans.is_empty());
                    drop(plan);
                    drop(owner);
                })
            })
        };
        let (((), old), refused, old_paid, first) = run(true);
        assert!(!refused && first.is_none() && !old.invalid);
        assert!(old.allocated <= old_paid && old.highwater <= old_paid);
        assert_eq!(old.allocated, old.freed);
        assert_eq!(old.live, 0);
        let (((), current), refused, paid, first) = run(false);
        assert!(!refused && first.is_none() && !current.invalid);
        assert!(current.allocated <= paid && current.highwater <= paid);
        assert_eq!(current.allocated, current.freed);
        assert_eq!(current.live, 0);
        assert!(current.allocated < old.allocated && paid < old_paid);
    }
}
#[test]
fn late_spill_preflight_precedes_early_alias_cycle_and_keeps_full_old_diagnostics() {
    let (mut program, index) = boundary_tests::fixture_with_uses(1, false);
    let late = scalar(true).functions[0].body.clone();
    let function = &mut program.functions[index];
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block");
    };
    let ResolvedStatement::Let { binding, value, .. } = &mut statements[1] else {
        panic!("view");
    };
    value.kind = ResolvedExprKind::Place(Place {
        root: binding.id.clone(),
        projections: vec![],
    });
    statements.push(ResolvedStatement::CopyDiscard {
        value: late,
        span: Span::default(),
    });
    let function = &program.functions[index];
    let compare = |limit| {
        with_limit_usage_refusal(limit, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            let error = paid_candidate::inspect(&program, function, &owner, &mut work).unwrap_err();
            let first = owner.first();
            drop(owner);
            (error, first)
        })
    };
    let ((current, current_first), refused, _, _) = compare(usize::MAX);
    assert!(!refused && current_first.is_none());
    let ((old, old_first), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let result = paid_candidate_before::inspect(&program, function, &owner).unwrap_err();
        let first = owner.first();
        drop(owner);
        (result, first)
    });
    assert!(!refused && old_first.is_none());
    assert_eq!(current, old);
    assert_eq!(
        current.message,
        "shared-loan alias provenance contains a cycle"
    );
    // Literal acquisition only: the later wide child must refuse before any alias pass.
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    let ((current, first), refused, _, receipt) = compare(setup);
    assert!(refused && first.is_some());
    assert_eq!(
        receipt.unwrap().requested,
        4 * std::mem::size_of::<&ResolvedExpr>()
    );
    assert_eq!(current, storage::marker());
    let ((old, first), refused, _, receipt) = with_limit_usage_refusal(setup, || {
        let owner = CfgReservation::acquire().unwrap();
        let result = paid_candidate_before::inspect(&program, function, &owner).unwrap_err();
        let first = owner.first();
        drop(owner);
        (result, first)
    });
    assert!(refused && first.is_some() && receipt.is_some());
    assert_eq!(current, old);
}

#[test]
fn complete_old_candidate_and_funded_receivers_are_exact_and_current_hooks_are_scoped() {
    use sha2::{Digest, Sha256};
    let old = include_str!("../../../docs/evidence/paid-loan-cursors/paid_candidate_before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "2171767962fc58bd699165f3023910712146871fadc6b2bc6077a02d7374bbbe"
    );
    assert_eq!(include_str!("../paid_candidate_before.rs"), old);
    let mut projected = paid_candidate::before_inventory_source(
        "src/loan_plan/paid_candidate.rs",
        include_str!("../paid_candidate.rs"),
    );
    for (current, original) in [
        ("    work: &mut WorkCounter<'_>,\n", ""),
        ("    // Complete discovery still precedes every alias/ownership diagnostic.\n    // Retain its actual paid spill and replay the immutable borrowed traversal.\n    let mut discovery = paid_cursor::Cursor::new(function, payer);\n    while discovery.next(work)?.is_some() {}\n    discovery.reset()?;\n\n", "    let mut roots = Vec::new();\n    storage::extend(\n        &mut roots,\n        function\n            .requires\n            .iter()\n            .chain(std::iter::once(&function.body))\n            .chain(function.ensures.iter()),\n        Some(payer),\n    )?;\n    let mut expressions = Vec::new();\n    let mut pending = Vec::new();\n    storage::extend(&mut pending, roots.iter().rev().copied(), Some(payer))?;\n    while let Some(expression) = pending.pop() {\n        storage::push(&mut expressions, expression, Some(payer))?;\n        paid_children::push_children(expression, &mut pending, Some(payer))?;\n    }\n\n"),
        ("    discovery.reset()?;\n    while let Some(expression) = discovery.next(work)? {", "    for expression in expressions {"),
        ("    while let Some(expression) = discovery.next(work)? {", "    for expression in &expressions {"),
    ] { assert_eq!(projected.matches(current).count(), 1); projected = projected.replacen(current, original, 1); }
    assert_eq!(projected, old);
    let funded = include_str!("../../../docs/evidence/paid-loan-cursors/funded_flat_before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(funded.as_bytes()))
        ),
        "50d842bbc31c8d811fdf2135528e959f066f459c8dcec0f0b5d1d231026297fb"
    );
    let current_hook = "        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);\n        work.allocation = Some(payer);\n        if paid_candidate::inspect(program, function, payer, &mut work)? {\n";
    let original_hook = "        if paid_candidate::inspect(program, function, payer)? {\n            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);\n            work.allocation = Some(payer);\n";
    let mut current = paid_candidate::before_inventory_source(
        "src/loan_plan/funded_flat.rs",
        include_str!("../funded_flat.rs"),
    );
    for test_hook in [
        "        #[cfg(test)]\n        work_boundary::apply(&mut work);\n",
        "\n#[cfg(test)]\nmod work_boundary;\n",
    ] {
        assert_eq!(current.matches(test_hook).count(), 1);
        current = current.replacen(test_hook, "", 1);
    }
    assert_eq!(current.matches(current_hook).count(), 1);
    assert_eq!(current.replacen(current_hook, original_hook, 1), funded);
    let children =
        include_str!("../../../docs/evidence/paid-loan-cursors/paid_children_before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(children.as_bytes()))
        ),
        "2d4dbf70b4e0555e3a95bf9a1abe19990d0239d5f34ffe5cdc140f68aa5be141"
    );
    assert_eq!(include_str!("../paid_children.rs"), children);
    let expected = funded
        .strip_suffix("#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\nmod directories_tests;\n")
        .unwrap()
        .strip_suffix("\n")
        .unwrap()
        .replacen(
            "paid_candidate::inspect",
            "paid_candidate_before::inspect",
            1,
        );
    assert_eq!(include_str!("funded_before.rs"), expected);
}
