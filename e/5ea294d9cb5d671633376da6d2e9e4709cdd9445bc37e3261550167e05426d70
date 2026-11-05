use super::*;
use crate::cleanup::{FieldLiveness, LivenessFlagId};
use crate::test_allocation_observer::{measure, Metrics};

fn nested(depth: usize, width: usize) -> FieldLivenessShape {
    let id = DeclarationId::new("profile.saved.identity");
    let mut shape = FieldLivenessShape::Record {
        declaration: id.clone(),
        fields: (0..width)
            .map(|index| FieldLiveness {
                field: id.clone(),
                field_index: u32::try_from(index).unwrap(),
                shape: FieldLivenessShape::Leaf {
                    flag: LivenessFlagId(u32::try_from(index).unwrap()),
                    lifecycle: DeclarationId::new(BYTES_DROP_LIFECYCLE_ID),
                },
            })
            .collect(),
    };
    for _ in 1..depth {
        shape = FieldLivenessShape::Record {
            declaration: id.clone(),
            fields: vec![FieldLiveness {
                field: id.clone(),
                field_index: 0,
                shape,
            }],
        };
    }
    shape
}
fn run(
    limit: usize,
    function: &ResolvedFunction,
    shape: &FieldLivenessShape,
    expected: Option<&Result<CleanupShapeProfile, Diagnostic>>,
) -> ((bool, bool), Metrics, bool, usize) {
    let ((outcome, metrics), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "profile receiver setup")
                    .and_then(|()| {
                        let mut payer = super::super::tests::payer(function, &mut budget);
                        inspect(shape, &mut payer).and_then(|profile| profile.finish(&mut payer))
                    });
                let matched = expected.is_none_or(|old| &result == old);
                let failed = result.is_err();
                if crate::bounded_output::active_refused() {
                    let before = crate::test_allocation_observer::current_metrics();
                    let retry = budget.reserve_bytes(function, 0, "profile sticky receiver");
                    assert!(retry.is_err());
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics().allocated,
                        before.allocated
                    );
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed)
            })
        });
    (outcome, metrics, refused, paid)
}
#[test]
fn full_profile_receiver_preserves_canonical_shapes_limits_and_prepaid_drop() {
    for program in super::super::tests::fixtures() {
        for function in &program.functions {
            let inventory = crate::cleanup::build_inventory(&program, function).unwrap();
            let mut synthetic = vec![FieldLivenessShape::NoDrop];
            synthetic.extend([
                nested(2, 0),
                nested(2, 1),
                nested(2, 17),
                nested(65, 1),
                nested(2, 257),
                nested(2, 4097),
            ]);
            for shape in inventory
                .slots
                .iter()
                .map(|slot| &slot.shape)
                .chain(synthetic.iter())
            {
                let expected = crate::cleanup::cleanup_shape_profile(shape);
                let (outcome, observed, refused, paid) =
                    run(usize::MAX, function, shape, Some(&expected));
                assert_eq!(outcome, (true, expected.is_err()));
                assert!(!refused && !observed.invalid);
                assert!(observed.allocated <= paid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, observed.freed);
                let (exact, observed, refused, used) = run(paid, function, shape, Some(&expected));
                assert_eq!(exact, outcome);
                assert!(!refused && !observed.invalid);
                assert_eq!(used, paid);
                assert_eq!(observed.live, 0);
                for limit in [0, paid - 1] {
                    let (short, observed, refused, used) = run(limit, function, shape, None);
                    assert!(short.1 && refused && !observed.invalid);
                    assert!(observed.allocated <= used);
                    assert_eq!(observed.live, 0);
                    assert_eq!(observed.allocated, observed.freed);
                }
            }
        }
    }
}
#[test]
fn semantic_profile_limit_stays_inline_until_the_complete_final_boundary() {
    let program = super::super::tests::fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let shape = nested(65, 1);
    let expected = crate::cleanup::cleanup_shape_profile(&shape);
    let ((matched, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .charge(function, 0, "deferred profile receiver setup")
                    .unwrap();
                let result = {
                    let mut payer = super::super::tests::payer(function, &mut budget);
                    let inspected = inspect(&shape, &mut payer).unwrap();
                    assert_eq!(inspected.failure, Some(Failure::DepthLimit));
                    let before = crate::test_allocation_observer::current_metrics();
                    let copied = inspected;
                    assert_eq!(copied, inspected);
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics().allocated,
                        before.allocated
                    );
                    let result = inspected.finish(&mut payer);
                    assert!(
                        crate::test_allocation_observer::current_metrics().allocated
                            > before.allocated
                    );
                    result
                };
                let matched = result == expected;
                drop(result);
                drop(budget);
                matched
            })
        });
    assert!(matched && !refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn complete_original_cleanup_root_remains_byte_exact_with_paid_profile_separate() {
    let source = include_str!("../../../../cleanup.rs");
    let registration = "#[cfg(test)]\npub(crate) mod schema_summary;\n";
    assert_eq!(source.matches(registration).count(), 1);
    let projected = source.replacen(registration, "", 1);
    assert_eq!(
        projected,
        include_str!("../../../../../docs/evidence/schema-summary/cleanup.rs.before")
    );
}

#[test]
fn completed_profile_cannot_publish_after_same_budget_sticky_refusal() {
    let program = super::super::tests::fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let shape = nested(2, 17);
    let ((refused, observed), outer_refused, paid) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget
                    .charge(function, 0, "sticky profile receiver setup")
                    .unwrap();
                let refused = {
                    let mut payer = super::super::tests::payer(function, &mut budget);
                    let inspected = inspect(&shape, &mut payer).unwrap();
                    assert!(inspected.failure.is_none());
                    payer.budget.remaining = 0;
                    let first = payer
                        .budget
                        .charge(function, 1, "real subsequent receiver work");
                    assert!(first.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let retry = inspected.finish(&mut payer);
                    assert!(retry.is_err());
                    assert_eq!(
                        crate::test_allocation_observer::current_metrics().allocated,
                        before.allocated
                    );
                    drop(retry);
                    drop(first);
                    true
                };
                drop(budget);
                refused
            })
        });
    assert!(refused && !outer_refused && !observed.invalid);
    assert!(observed.allocated <= paid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}
