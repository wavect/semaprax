//! Genuine selected session constructor, receiving and final Drop; old BTree control stays independent.
use super::*;
use crate::cleanup_plan::replay::expression_facts::Lookup;
#[test]
fn paid_arena_actual_session_exact_short_zero_work_and_drop_keep_first_receipt() {
    let program = tiny();
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    let (full, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut selected = session(&context, function, usize::MAX);
        let facts = expression_facts::paid_arena(function, &mut selected.budget).unwrap();
        assert_eq!(facts.len(), 2);
        for (key, _) in facts.entries_paid(function, &mut selected.budget).unwrap() {
            assert!(facts
                .fact(function, key, &mut selected.budget)
                .unwrap()
                .is_some());
        }
        let charged = selected.usage().1;
        drop(facts);
        drop(selected.finish_paid().ok().expect("paid arena finish"));
        charged
    });
    assert!(!refused);
    for limit in [full - 1, full] {
        let ((), metrics) = measure(|| {
            let mut selected = session(&context, function, limit);
            let pointer = selected.budget.refusal_text.as_ref().unwrap().as_ptr();
            // Match the complete measured success path, including paid lookup
            // receiving carriers, at both the exact and one-byte-short limit.
            let result = (|| {
                let facts = expression_facts::paid_arena(function, &mut selected.budget)?;
                assert_eq!(facts.len(), 2);
                for (key, _) in facts.entries_paid(function, &mut selected.budget)? {
                    assert!(facts.fact(function, key, &mut selected.budget)?.is_some());
                }
                Ok(facts)
            })();
            match result {
                Ok(facts) => {
                    assert_eq!(limit, full);
                    assert_eq!(selected.usage().1, full);
                    assert_eq!(facts.len(), 2);
                    drop(facts);
                    drop(selected.finish_paid().ok().expect("complete paid owner"));
                }
                Err(expression_facts::errors::EngineError::Owned(error)) => {
                    assert_eq!(limit, full - 1);
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.message.as_ptr(), pointer);
                    assert!(selected.budget.refusal_spent);
                    assert!(selected
                        .budget
                        .physical
                        .as_deref_mut()
                        .unwrap()
                        .reserve(0)
                        .is_err());
                    let before = current_metrics();
                    assert!(expression_facts::paid_arena(function, &mut selected.budget).is_err());
                    let after = current_metrics();
                    assert_eq!(after.allocations, before.allocations);
                    assert_eq!(after.allocated, before.allocated);
                    drop(error);
                    drop(selected);
                }
                _ => panic!("physical refusal must move prepaid diagnostic"),
            }
        });
        assert!(!metrics.invalid);
        assert!(metrics.allocated <= full);
        assert!(metrics.highwater <= full);
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
    }
    let ((), metrics) = measure(|| {
        let mut selected = session(&context, function, usize::MAX);
        let pointer = selected.budget.refusal_text.as_ref().unwrap().as_ptr();
        selected.budget.remaining = 0;
        let error = expression_facts::paid_arena(function, &mut selected.budget)
            .err()
            .unwrap();
        let expression_facts::errors::EngineError::Owned(error) = error else {
            panic!("work refusal owns prepaid text")
        };
        assert_eq!(error.message.as_ptr(), pointer);
        assert!(selected.budget.refusal_spent);
        assert!(selected
            .budget
            .physical
            .as_deref_mut()
            .unwrap()
            .reserve(0)
            .is_err());
        drop(error);
        drop(selected);
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
