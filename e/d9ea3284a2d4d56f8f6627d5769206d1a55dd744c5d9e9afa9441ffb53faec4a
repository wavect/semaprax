use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal_site};
use crate::test_allocation_observer::measure;
use std::mem::size_of;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module cursor.calls;
@id("cursor.add") fn add(left: i64, right: i64) -> i64 { left + right }
@id("cursor.main") fn main() -> i64 requires true ensures result >= 0 {
    let value = add(1, 2);
    if value > 0 { add(value, 3) } else { add(0, 4) }
}"#,
            "structural-call-cursor.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

fn prepared(function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    budget.payer = AllocationPayer::ActiveBuilder;
    budget.prepare_builder(function).unwrap();
    budget
}

fn check_lookup(function: &ResolvedFunction) {
    let mut ordinary = ReplayBudget::unobserved();
    let mut old_budget = ReplayBudget::unobserved();
    let mut cursor = CallCursor::new::<Diagnostic>(function, &mut ordinary).unwrap();
    let mut old = before::CallCursor::new(function, &mut old_budget).unwrap();
    // Probe every authored node, not only calls or the first successful lookup.
    // Reusing both owners after early returns also exercises stale stack slots.
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        let actual = cursor
            .find::<Diagnostic>(function, &node.id, &mut ordinary)
            .unwrap();
        let reference = before::lookup(&mut old, function, &node.id, &mut old_budget).unwrap();
        assert!(std::ptr::eq(actual, reference));
        assert!(std::ptr::eq(actual, node));
        pending.extend(ChildCursor::new(node));
    }
    let missing = ExpressionId::from_owned("cursor.absent".into());
    assert_eq!(
        cursor
            .find::<Diagnostic>(function, &missing, &mut ordinary)
            .unwrap_err(),
        before::lookup(&mut old, function, &missing, &mut old_budget).unwrap_err()
    );
}

#[test]
fn every_lookup_and_missing_error_match_the_complete_original_owner() {
    for function in &program().functions {
        check_lookup(function);
    }
}

#[test]
fn raw_cursor_retains_the_original_514_depth_boundary() {
    let program = program();
    let template = &program.functions[0];
    let mut function = template.clone();
    function.requires.clear();
    function.ensures.clear();
    let ResolvedExprKind::Block { tail, .. } = &template.body.kind else {
        panic!("source block");
    };
    let mut root = tail.as_ref().clone();
    root.id = ExpressionId::from_owned("cursor.bottom".into());
    // The chosen bottom is itself binary: search its root, not its children.
    let bottom = root.id.clone();
    for depth in 1..=515 {
        if depth >= 513 {
            function.body = root.clone();
            let mut actual_budget = ReplayBudget::unobserved();
            let mut old_budget = ReplayBudget::unobserved();
            let mut actual = CallCursor::new::<Diagnostic>(&function, &mut actual_budget).unwrap();
            let mut old = before::CallCursor::new(&function, &mut old_budget).unwrap();
            let result = actual.find::<Diagnostic>(&function, &bottom, &mut actual_budget);
            let reference = before::lookup(&mut old, &function, &bottom, &mut old_budget);
            assert_eq!(result.is_ok(), depth <= 514);
            match (result, reference) {
                (Ok(actual), Ok(reference)) => assert!(std::ptr::eq(actual, reference)),
                (Err(actual), Err(reference)) => assert_eq!(actual, reference),
                _ => panic!("original raw depth verdict changed"),
            }
        }
        let mut layer = template.body.clone();
        layer.id = ExpressionId::from_owned(format!("cursor.depth.{}", depth + 1));
        let ResolvedExprKind::Block { tail, .. } = &mut layer.kind else {
            unreachable!();
        };
        **tail = root;
        root = layer;
    }
}

const TARGETS: [usize; 10] = [2, 4, 8, 16, 32, 64, 128, 256, 512, 514];
fn price(target: usize) -> usize {
    2 * size_of::<Vec<Slot<'_>>>() + target * size_of::<Slot<'_>>()
}

fn check_growth(function: &ResolvedFunction) {
    let mut budget = prepared(function);
    let expected = size_of::<CallCursor<'_>>() + TARGETS.into_iter().map(price).sum::<usize>();
    let (((), observed), refused, used) = with_limit_usage(expected, || {
        measure(|| {
            let mut cursor = CallCursor::new::<Diagnostic>(function, &mut budget).unwrap();
            for target in TARGETS {
                cursor
                    .grow::<Diagnostic>(function, target, &mut budget)
                    .unwrap();
                assert_eq!(cursor.frames.len(), target);
                assert_eq!(cursor.frames.capacity(), target);
                assert!(cursor.frames.iter().all(Option::is_none));
            }
            cursor
                .grow::<Diagnostic>(function, 514, &mut budget)
                .unwrap();
            drop(cursor);
        })
    });
    assert!(!refused && !observed.invalid);
    assert_eq!(used, expected);
    assert_eq!(observed.allocated, 1536 * size_of::<Slot<'_>>());
    assert_eq!(observed.highwater, 1026 * size_of::<Slot<'_>>());
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.live, 0);
}

fn check_short(function: &ResolvedFunction) {
    for refused_target in TARGETS {
        let mut budget = prepared(function);
        let earlier = TARGETS.into_iter().take_while(|n| *n < refused_target);
        let before = size_of::<CallCursor<'_>>() + earlier.map(price).sum::<usize>();
        let cap = before + price(refused_target) - 1;
        let (_, refused, used, receipt, _) = with_limit_usage_refusal_site(cap, || {
            let mut cursor = CallCursor::new::<Diagnostic>(function, &mut budget).unwrap();
            for target in TARGETS.into_iter().take_while(|n| *n < refused_target) {
                cursor
                    .grow::<Diagnostic>(function, target, &mut budget)
                    .unwrap();
            }
            let old_capacity = cursor.frames.capacity();
            assert!(cursor
                .grow::<Diagnostic>(function, refused_target, &mut budget)
                .is_err());
            assert_eq!(cursor.frames.capacity(), old_capacity);
            // Spare backing cannot reopen a refused payer, even for reuse.
            assert!(cursor.grow::<Diagnostic>(function, 0, &mut budget).is_err());
        });
        assert!(refused);
        assert_eq!(used, before + 2 * size_of::<Vec<Slot<'_>>>());
        let receipt = receipt.unwrap();
        assert_eq!(receipt.requested, refused_target * size_of::<Slot<'_>>());
        assert_eq!(receipt.remaining + 1, receipt.requested);
    }
}

fn check_shallow(function: &ResolvedFunction) {
    let mut new_budget = prepared(function);
    let (_, refused, new_bill) = with_limit_usage(usize::MAX, || {
        let mut cursor = CallCursor::new::<Diagnostic>(function, &mut new_budget).unwrap();
        let node = cursor
            .find::<Diagnostic>(function, &function.body.id, &mut new_budget)
            .unwrap();
        assert!(std::ptr::eq(node, &function.body));
    });
    assert!(!refused);
    let mut old_budget = prepared(function);
    let (_, refused, old_bill) = with_limit_usage(usize::MAX, || {
        let mut cursor = before::CallCursor::new(function, &mut old_budget).unwrap();
        before::lookup(&mut cursor, function, &function.body.id, &mut old_budget).unwrap();
    });
    assert!(!refused);
    assert_eq!(old_bill, 514 * size_of::<Slot<'_>>());
    assert_eq!(new_bill, size_of::<CallCursor<'_>>() + price(2));
    assert!(new_bill < old_bill);
}

fn check_zero_and_work(function: &ResolvedFunction) {
    let mut budget = prepared(function);
    let (_, refused, used, receipt, _) = with_limit_usage_refusal_site(0, || {
        assert!(CallCursor::new::<Diagnostic>(function, &mut budget).is_err());
    });
    assert!(refused);
    assert_eq!(used, 0);
    assert_eq!(receipt.unwrap().requested, size_of::<CallCursor<'_>>());

    let mut budget = prepared(function);
    let mut cursor = CallCursor::new::<Diagnostic>(function, &mut budget).unwrap();
    budget.remaining = 0;
    let ((result, observed), refused, used) = with_limit_usage(usize::MAX, || {
        measure(|| cursor.grow::<Diagnostic>(function, 1, &mut budget))
    });
    assert!(result.is_err());
    assert!(!refused);
    assert_eq!(used, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(cursor.frames.capacity(), 0);
}

#[test]
fn actual_growth_backing_overlap_drop_and_every_short_frontier_are_paid() {
    let program = program();
    let function = &program.functions[0];
    check_growth(function);
    check_short(function);
    check_shallow(function);
    check_zero_and_work(function);
}

#[test]
fn complete_original_lookup_and_unchanged_receivers_remain_byte_bound() {
    let old = include_str!(
        "../../../../../../docs/evidence/structural-call-cursor-v1/calls.before.rs.txt"
    );
    let current = crate::source_reflows_merge::main27424_input(
        "src/cleanup_plan/replay/structural_control/calls.rs",
        include_str!("../../calls.rs"),
    );
    let owner_start = old.find("pub(super) struct CallCursor").unwrap();
    let owner_end = old.find("\nenum Signature").unwrap();
    let owner = &old[owner_start..owner_end];
    let independently_compiled = include_str!("../cursor_before.rs");
    assert!(independently_compiled.contains(owner));
    assert_eq!(
        current.replace("mod cursor;\npub(super) use cursor::CallCursor;\n", owner),
        old
    );
    let new = crate::source_reflows_merge::main27424_input(
        "src/cleanup_plan/replay/structural_control/calls/cursor.rs",
        include_str!("../cursor.rs"),
    );
    let new_find = &new[new.find("    pub(super) fn find(").unwrap()
        ..new.find("\n}\n\n#[cfg(test)]").unwrap() + 1];
    let old_find = &owner[owner.find("    fn find(").unwrap()..owner.len() - 2];
    assert_eq!(
        new_find
            .replace("pub(super) fn find", "fn find")
            .replace("        self.grow(function, 1, budget)?;\n", "")
            .replace("if depth == DEPTH", "if depth == self.frames.len()")
            .replace(
                "                    self.grow(function, depth + 1, budget)?;\n",
                ""
            ),
        old_find
    );
}

fn selected_policy_controls<T>() -> usize {
    use crate::cleanup_plan::replay::structural_control::call_receiving::Permit;
    type Engine = expression_facts::errors::EngineError;
    size_of::<(
        Permit<'static, Engine>,
        Kind,
        DeclarationId,
        semantic_refusal::Atom,
        semantic_refusal::Receipt,
        Engine,
        Option<Engine>,
        Result<T, Engine>,
        Result<(), Engine>,
        Result<Permit<'static, Engine>, Engine>,
        &'static Permit<'static, Engine>,
    )>()
}
fn selected_budget(function: &ResolvedFunction, extra: usize) -> ReplayBudget {
    let setup = size_of::<PhysicalReservation>()
        + 514 * size_of::<Option<(&ResolvedType, usize)>>()
        + function.id.as_str().len()
        + 512;
    let mut budget = ReplayBudget::with_physical(function, setup.saturating_add(extra)).unwrap();
    budget.remaining = usize::MAX;
    budget
}
#[test]
fn selected_cursor_pays_every_growth_and_reuse_permit_and_retains_whole_old_owner() {
    type Engine = expression_facts::errors::EngineError;
    let program = program();
    let function = &program.functions[0];
    let exact = selected_policy_controls::<CallCursor<'_>>()
        + size_of::<CallCursor<'_>>()
        + selected_policy_controls::<()>()
        + price(2);
    for available in [0, exact - 1, exact] {
        let (accepted, metrics) = measure(|| {
            let mut budget = selected_budget(function, available);
            let Ok(mut cursor) = CallCursor::new::<Engine>(function, &mut budget) else {
                return false;
            };
            cursor.grow::<Engine>(function, 1, &mut budget).is_ok()
        });
        assert_eq!(accepted, available == exact);
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    let mut budget = selected_budget(function, usize::MAX);
    let mut cursor = CallCursor::new::<Engine>(function, &mut budget).unwrap();
    cursor.grow::<Engine>(function, 1, &mut budget).unwrap();
    let before = budget.physical.as_deref().unwrap().charged();
    let work = budget.remaining;
    cursor.grow::<Engine>(function, 0, &mut budget).unwrap();
    assert_eq!(
        budget.physical.as_deref().unwrap().charged() - before,
        selected_policy_controls::<()>()
    );
    assert_eq!(work - budget.remaining, 11);
    drop(cursor);
    drop(budget);

    for remaining in [
        0,
        selected_policy_controls::<()>() - 1,
        selected_policy_controls::<()>(),
    ] {
        let (accepted, metrics) = measure(|| {
            let mut budget = selected_budget(function, exact + remaining);
            let mut cursor = CallCursor::new::<Engine>(function, &mut budget).unwrap();
            cursor.grow::<Engine>(function, 1, &mut budget).unwrap();
            let before = budget.physical.as_deref().unwrap().charged();
            let answer = cursor.grow::<Engine>(function, 0, &mut budget);
            if remaining == selected_policy_controls::<()>() {
                assert!(answer.is_ok());
                assert_eq!(
                    budget.physical.as_deref().unwrap().charged() - before,
                    remaining
                );
                true
            } else {
                assert!(answer.is_err());
                let first = budget
                    .physical
                    .as_deref_mut()
                    .unwrap()
                    .reserve(0)
                    .unwrap_err();
                let charged = budget.physical.as_deref().unwrap().charged();
                assert!(cursor.grow::<Engine>(function, 0, &mut budget).is_err());
                assert_eq!(
                    budget
                        .physical
                        .as_deref_mut()
                        .unwrap()
                        .reserve(0)
                        .unwrap_err(),
                    first
                );
                assert_eq!(budget.physical.as_deref().unwrap().charged(), charged);
                false
            }
        });
        assert_eq!(accepted, remaining == selected_policy_controls::<()>());
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }

    let missing = ExpressionId::from_owned("absent:é".into());
    let mut current_budget = selected_budget(function, usize::MAX);
    let mut old_budget = selected_budget(function, usize::MAX);
    let mut current = CallCursor::new::<Engine>(function, &mut current_budget).unwrap();
    let mut old = before_selected::CallCursor::new::<Engine>(function, &mut old_budget).unwrap();
    let detail = |error| match error {
        Engine::Structural(receipt) => receipt.legacy(function),
        Engine::Owned(diagnostic) => diagnostic,
        _ => panic!("structural cursor diagnostic"),
    };
    let current_error = detail(
        current
            .find::<Engine>(function, &missing, &mut current_budget)
            .unwrap_err(),
    );
    let old_error = detail(
        old.find::<Engine>(function, &missing, &mut old_budget)
            .unwrap_err(),
    );
    assert_eq!(current_error.code, old_error.code);
    assert_eq!(current_error.severity, old_error.severity);
    assert_eq!(current_error.message, old_error.message);
    assert_eq!(current_error.path, old_error.path);
    assert_eq!(current_error.span, old_error.span);
    assert_eq!(current_error.help, old_error.help);
    drop(current);
    drop(old);
    drop(current_budget);
    drop(old_budget);

    for lookups in [0, 1, 4] {
        for old in [true, false] {
            let ((bill, work), metrics) = measure(|| {
                let mut budget = selected_budget(function, usize::MAX);
                if old {
                    let mut cursor =
                        before_selected::CallCursor::new::<Engine>(function, &mut budget).unwrap();
                    for _ in 0..lookups {
                        assert!(std::ptr::eq(
                            cursor
                                .find::<Engine>(function, &function.body.id, &mut budget)
                                .unwrap(),
                            &function.body
                        ));
                    }
                    drop(cursor);
                } else {
                    let mut cursor = CallCursor::new::<Engine>(function, &mut budget).unwrap();
                    for _ in 0..lookups {
                        assert!(std::ptr::eq(
                            cursor
                                .find::<Engine>(function, &function.body.id, &mut budget)
                                .unwrap(),
                            &function.body
                        ));
                    }
                    drop(cursor);
                }
                (
                    budget.physical.as_deref().unwrap().charged(),
                    usize::MAX - budget.remaining,
                )
            });
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!("MAIN27424 selected lookups={lookups} complete_old677={old} charged={bill} work={work} metrics={metrics:?}");
        }
    }
}
