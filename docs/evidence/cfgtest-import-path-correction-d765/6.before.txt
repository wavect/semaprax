//! Real wide authored discovery and the complete original allocation fixture.
use super::*;
use crate::test_allocation_observer::current_metrics;

pub(super) fn wide_source(source: &str, name: &str) -> String {
    assert!(!source.contains("cursor_value_"));
    let declaration = format!("@id(\"{name}\")");
    assert_eq!(source.matches(&declaration).count(), 1);
    let start = source.find(&declaration).unwrap();
    let body = start + source[start..].find('{').unwrap() + 1;
    let mut wide = source[..body].to_owned();
    // Every next scalar consumes its predecessor. The final Copy discard
    // receives the chain before the complete original body resumes. These
    // are real authored bindings, not allocations added to the observer.
    wide.push_str("let cursor_value_0=7;\n");
    for index in 1..80 {
        wide.push_str(&format!(
            "let cursor_value_{index}=cursor_value_{}+1;\n",
            index - 1
        ));
    }
    wide.push_str("cursor_value_79;\n");
    wide.push_str(&source[body..]);
    wide
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
        paid_children::push_children(expression, &mut pending, None).unwrap();
    }
    result
}
pub(super) fn assert_real_discovery_spill(function: &ResolvedFunction) {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("wide authored block")
    };
    assert!(statements.len() >= 81);
    let expected = old_order(function);
    let (((), metrics), refused, paid) = with_limit_usage(usize::MAX, || {
        measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let before = current_metrics();
            let mut cursor = paid_cursor::Cursor::new(function, &owner);
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            let mut reused = None;
            for pass in 1..=3 {
                for expression in &expected {
                    assert!(std::ptr::eq(
                        cursor.next(&mut work).unwrap().unwrap(),
                        *expression
                    ));
                }
                assert!(cursor.next(&mut work).unwrap().is_none());
                assert_eq!(work.used, pass * expected.len());
                cursor.reset().unwrap();
                if pass == 1 {
                    assert!(current_metrics().allocations > before.allocations);
                    assert!(current_metrics().allocated > before.allocated);
                }
                if let Some(before) = reused {
                    assert_eq!(
                        current_metrics(),
                        before,
                        "retained spill reuse allocates nothing"
                    );
                } else {
                    reused = Some(current_metrics());
                }
            }
            drop(cursor);
            drop(owner);
        })
    });
    assert!(!refused && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}

#[test]
fn complete_original_allocation_driver_and_receiver_stay_byte_exact() {
    let original = include_str!(
        "../../../docs/evidence/funded-planner-fact-owner/original-funded-flat-tests.txt"
    );
    assert_eq!(include_str!("tests_before.rs"), original);
    let before =
        include_str!("../../../docs/evidence/funded-planner-fact-owner/original-funded-before.txt");
    assert_eq!(include_str!("../paid_cursor/funded_before.rs"), before);
    assert_eq!(
        include_str!("fixture_before.rs"),
        format!("{before}\n#[path = \"tests_before.rs\"]\nmod tests;\n")
    );
    let old = "        let ast = crate::check(source, \"paid-planner.spx\").unwrap();";
    let new = "        let source = fixture_receiving::wide_source(source, name);\n        let ast = crate::check(&source, \"paid-planner.spx\").unwrap();";
    assert_eq!(original.matches(old).count(), 1);
    let call = "        assert_paid_owner_through_drop(&program, function, &old);";
    let projected = original.replacen(old, new, 1).replacen(
        call,
        &format!("        fixture_receiving::assert_real_discovery_spill(function);\n{call}"),
        1,
    );
    assert_eq!(include_str!("tests.rs"), format!("{projected}\n#[path = \"fixture_before.rs\"]\nmod fixture_before;\n#[path = \"fixture_receiving.rs\"]\nmod fixture_receiving;\n"));
    assert_eq!(
        original
            .matches("assert!(observed.allocations > 8);")
            .count(),
        1
    );
    assert_eq!(
        include_str!("tests.rs")
            .matches("assert!(observed.allocations > 8);")
            .count(),
        1
    );
}
