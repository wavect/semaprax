use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

struct Chunks;
impl fmt::Display for Chunks {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("é\0abc")?;
        output.write_str("0123456789abcdef")?;
        output.write_str("🦀suffix")
    }
}
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(
            "module encoded; @id(\"main\") fn main()->i64 {7}",
            "encoded.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn text(mode: usize, payer: &mut Payer<'_>) -> Result<String, GrowthFailure<Diagnostic>> {
    let mut output = acquire(payer)?;
    match mode {
        0 => capped(&mut output, format_args!("{}", Chunks), payer)?,
        1 => strict(&mut output, format_args!("{}", Chunks), payer)?,
        _ => raw(&mut output, format_args!("{}", Chunks), payer)?,
    }
    Ok(output.into_string())
}
fn run(
    limit: usize,
    work: usize,
    function: &ResolvedFunction,
    mode: usize,
    expected: Option<&str>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "encoded receiver setup")
                    .map_err(GrowthFailure::Payment)
                    .and_then(|()| {
                        budget.remaining = work;
                        text(mode, &mut super::super::tests::payer(function, &mut budget))
                    });
                let matched =
                    expected.is_none_or(|old| result.as_ref().is_ok_and(|current| current == old));
                let failed = result.is_err();
                let used = work.saturating_sub(budget.remaining);
                if failed {
                    assert!(matches!(&result, Err(GrowthFailure::Payment(_))));
                    let before = current_metrics();
                    let retry = text(mode, &mut super::super::tests::payer(function, &mut budget));
                    assert!(matches!(retry, Err(GrowthFailure::Payment(_))));
                    assert_eq!(current_metrics().allocated, before.allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used)
            })
        });
    (outcome, observed, refused, paid)
}
#[test]
fn real_encoded_constructor_work_exact_short_zero_and_receiving_drop_keep_all_utf8() {
    let program = fixture();
    let function = &program.functions[0];
    for mode in 0..3 {
        let expected = "é\0abc0123456789abcdef🦀suffix";
        let (outcome, observed, refused, paid) = run(
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            function,
            mode,
            Some(expected),
        );
        assert!(outcome.0 && !outcome.1 && !refused && !observed.invalid);
        assert!(observed.allocated <= paid);
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
        let (exact, observed, refused, used) = run(paid, outcome.2, function, mode, Some(expected));
        assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
        assert_eq!(used, paid);
        assert_eq!(observed.live, 0);
        for (bytes, work) in [
            (0, MAX_REPLAY_WORK_UNITS),
            (paid - 1, MAX_REPLAY_WORK_UNITS),
            (paid, 0),
            (paid, outcome.2 - 1),
        ] {
            let (short, observed, _, used) = run(bytes, work, function, mode, None);
            assert!(short.1 && !observed.invalid && observed.allocated <= used);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn complete_original_logical_frontier_suppression_strict_and_raw_modes_stay_equal() {
    // These callbacks observe request/work order, not physical payment. This
    // independent logical oracle makes no allocation-funding claim.
    for limit in [0, 1, 5, 8, 17, 64] {
        for floor in [0, 1, 6] {
            for mode in 0..3 {
                let old = crate::bounded_output::with_limit_usage_refusal(limit, || {
                    assert_eq!(
                        crate::bounded_output::set_active_floor(floor),
                        floor <= limit
                    );
                    let mut pay = |_| Ok::<_, ()>(());
                    let mut output = ClassifierString::acquire(&mut pay).unwrap();
                    match mode {
                        0 => output.capped(format_args!("{}", Chunks), &mut pay).unwrap(),
                        1 => output.strict(format_args!("{}", Chunks), &mut pay).unwrap(),
                        _ => output.raw(format_args!("{}", Chunks), &mut pay).unwrap(),
                    }
                    output.into_string()
                });
                let current = crate::bounded_output::with_limit_usage_refusal(limit, || {
                    assert_eq!(
                        crate::bounded_output::set_active_floor(floor),
                        floor <= limit
                    );
                    let mut context = (0usize, 0usize);
                    let mut pay = |bytes, context: &mut (usize, usize)| {
                        context.0 += bytes;
                        Ok::<_, ()>(())
                    };
                    let mut work = |units, context: &mut (usize, usize)| {
                        context.1 += units;
                        Ok::<_, ()>(())
                    };
                    let mut output =
                        ClassifierString::acquire_with_work(&mut context, &mut pay, &mut work)
                            .unwrap();
                    match mode {
                        0 => output
                            .capped_with_work(
                                format_args!("{}", Chunks),
                                &mut context,
                                &mut pay,
                                &mut work,
                            )
                            .unwrap(),
                        1 => output
                            .strict_with_work(
                                format_args!("{}", Chunks),
                                &mut context,
                                &mut pay,
                                &mut work,
                            )
                            .unwrap(),
                        _ => output
                            .raw_with_work(
                                format_args!("{}", Chunks),
                                &mut context,
                                &mut pay,
                                &mut work,
                            )
                            .unwrap(),
                    }
                    output.into_string()
                });
                assert_eq!(old, current);
            }
        }
    }
    struct Broken;
    impl fmt::Display for Broken {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            output.write_str("accepted")?;
            Err(fmt::Error)
        }
    }
    let old = crate::bounded_output::with_limit_usage_refusal(17, || {
        let mut pay = |_| Ok::<_, ()>(());
        let mut output = ClassifierString::acquire(&mut pay).unwrap();
        output.strict(format_args!("{}", Broken), &mut pay).unwrap();
        output.into_string()
    });
    let current = crate::bounded_output::with_limit_usage_refusal(17, || {
        let mut context = ();
        let mut pay = |_, _: &mut ()| Ok::<_, ()>(());
        let mut work = |_, _: &mut ()| Ok::<_, ()>(());
        let mut output =
            ClassifierString::acquire_with_work(&mut context, &mut pay, &mut work).unwrap();
        output
            .strict_with_work(
                format_args!("{}", Broken),
                &mut context,
                &mut pay,
                &mut work,
            )
            .unwrap();
        output.into_string()
    });
    assert_eq!(old, current);
}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    let current = include_str!("../../../../bounded_output/classifier_string.rs")
        .strip_suffix("\n#[cfg(test)]\nmod oracle_work;\n")
        .unwrap();
    let marker = "    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n";
    assert_eq!(current.matches(marker).count(), 1);
    let current = current.replace(marker, "");
    assert_eq!(
        current.strip_suffix("\nmod inventory_work;\n").unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}

#[path = "tests/constructors.rs"]
mod constructors;
