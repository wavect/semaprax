use super::*;
use std::cell::Cell;
thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(original: bool, run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(original)));
    run()
}
fn function() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn unit_weight_and_every_fallback_keep_full_prechange_ids_signatures_work_and_errors() {
    let function = function();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let mut old = Canonicalizer::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut old_budget = ReplayBudget::new();
    let mut budget = ReplayBudget::new();
    let mut inputs = Vec::new();
    for root in &roots[..6] {
        for weight in [0, 1, 2, u64::MAX] {
            inputs.push(vec![(*root, weight)]);
        }
    }
    inputs.extend([
        Vec::new(),
        vec![(roots[0], 1), (roots[1], 1)],
        vec![(roots[0], 1), (roots[0], 1)],
        vec![(roots[3], u64::MAX)],
    ]);
    for input in inputs {
        let expected = with_original(true, || old.id(&function, input.clone(), &mut old_budget));
        let actual = current.id(&function, input, &mut budget);
        assert_eq!(actual, expected);
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        assert_eq!(current.ids, old.ids);
        assert_eq!(current.states, old.states);
    }
    let expected = with_original(true, || {
        old.id(&function, vec![(roots[6], 1)], &mut old_budget)
    });
    let actual = current.id(&function, vec![(roots[6], 1)], &mut budget);
    assert!(expected.is_err());
    assert_eq!(actual, expected);
    assert_eq!(budget.remaining, old_budget.remaining);
}

#[test]
fn whole_receiver_spends_less_backing_and_keeps_exact_short_and_zero_limits() {
    let function = function();
    let (automaton, sink, roots) = super::tests::automaton(&function);
    let observe = |original, limit| {
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            crate::test_allocation_observer::measure(|| {
                with_original(original, || {
                    let mut budget = ReplayBudget::new();
                    let mut canonical = Canonicalizer::new(&automaton, sink);
                    let result = (|| {
                        for root in &roots[..6] {
                            budget.reserve_array::<(usize, u64)>(
                                &function,
                                1,
                                "unit-weight test root",
                            )?;
                            assert!(canonical
                                .id(&function, vec![(*root, 1)], &mut budget)?
                                .is_some());
                        }
                        Ok::<_, Diagnostic>(())
                    })();
                    let success = result.is_ok();
                    drop(result);
                    drop(canonical);
                    drop(budget);
                    success
                })
            })
        })
    };
    let ((success, old_allocation), refused, old_paid, first) = observe(true, usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!old_allocation.invalid && old_allocation.live == 0);
    assert_eq!(old_allocation.allocated, old_allocation.freed);
    assert!(old_allocation.allocated <= old_paid);
    let ((success, allocation), refused, exact, first) = observe(false, usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    assert!(exact < old_paid);
    assert!(allocation.allocated < old_allocation.allocated);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = observe(false, limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        assert_eq!(success, limit == exact);
        if success {
            assert!(!refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn complete_previous_successor_producer_and_whole_current_module_remain_bound() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/unit-weight-closure-v1/canonical_shared.before.rs.txt"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "a3ce7638daaf633d17e121f51028875b733d77e8c38464699c83700588003ca6"
    );
    let method_start = original.find("    fn successors(").unwrap();
    let method_end = method_start
        + original[method_start..]
            .find("\n    pub(super) fn id(")
            .unwrap();
    let compiled = include_str!("before_unit.rs");
    let start = compiled
        .find("    pub(super) fn successors_before_unit(")
        .unwrap();
    let restored = compiled[start..]
        .strip_suffix("\n}\n")
        .unwrap()
        .trim_end_matches('\n')
        .replacen("pub(super) fn successors_before_unit(", "fn successors(", 1);
    assert_eq!(
        restored,
        original[method_start..method_end].trim_end_matches('\n')
    );
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/unit-weight-closure-v1/inverse.json"
    )))
    .unwrap();
    let mut restored = include_str!("../canonical_shared.rs").to_owned();
    for change in inverse.as_array().unwrap().iter().rev() {
        let start = change[0].as_u64().unwrap() as usize;
        let new = change[1].as_str().unwrap();
        let previous = change[2].as_str().unwrap();
        assert_eq!(restored.get(start..start + new.len()), Some(new));
        restored.replace_range(start..start + new.len(), previous);
    }
    assert_eq!(restored, original);
}
