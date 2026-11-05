use super::*;
use crate::cleanup::{FieldLiveness, FieldLivenessShape, LivenessFlagId, BYTES_DROP_LIFECYCLE_ID};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
fn old(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<Summary, Diagnostic> {
    let inventory = crate::cleanup::build_inventory(program, function)?;
    let nested = inventory.slots.iter().try_fold(false, |nested, slot| {
        crate::cleanup::cleanup_shape_profile(&slot.shape)
            .map(|profile| nested || profile.has_nested_owned_bytes)
    })?;
    Ok(Summary {
        has_nested_owned_bytes: nested,
        has_variant: inventory.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2,
        has_iterator: inventory
            .flags
            .iter()
            .any(|flag| flag.lifecycle.as_str() == crate::cleanup::ITER_DROP_LIFECYCLE_ID),
        storage_count: inventory.slots.len(),
        flag_count: inventory.flags.len(),
    })
}
fn run(
    bytes: usize,
    work: usize,
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expected: &Result<Summary, Diagnostic>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, metrics), refused, paid) =
        crate::bounded_output::with_limit_usage(bytes, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "full streaming receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        collect_with_payer(
                            program,
                            function,
                            &mut super::super::tests::payer(function, &mut budget),
                        )
                    });
                let matched = &result == expected;
                let physical = budget.refusal_spent || crate::bounded_output::active_refused();
                let used = work.saturating_sub(budget.remaining);
                if physical {
                    let before = current_metrics();
                    let retry = collect_with_payer(
                        program,
                        function,
                        &mut super::super::tests::payer(function, &mut budget),
                    );
                    assert!(retry.is_err());
                    assert_eq!(before.allocated, current_metrics().allocated);
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, physical, used)
            })
        });
    (outcome, metrics, refused, paid)
}
fn control(program: &ResolvedProgram, function: &ResolvedFunction) {
    // The independently compiled original constructs and receives its COMPLETE
    // inventory outside the new payer observation; no new Drop serves as oracle.
    let expected = old(program, function);
    let (ample, metrics, refused, paid) = run(
        usize::MAX,
        MAX_REPLAY_WORK_UNITS,
        program,
        function,
        &expected,
    );
    assert!(ample.0 && !ample.1 && !refused && !metrics.invalid);
    assert!(metrics.allocated <= paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let (exact, metrics, refused, used) = run(paid, ample.2, program, function, &expected);
    assert!(exact.0 && !exact.1 && !refused && !metrics.invalid);
    assert_eq!(used, paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    for (bytes, work) in [
        (0, MAX_REPLAY_WORK_UNITS),
        (paid.saturating_sub(1), MAX_REPLAY_WORK_UNITS),
        (paid, 0),
        (paid, ample.2.saturating_sub(1)),
    ] {
        if bytes == paid && work == ample.2 {
            continue;
        }
        let (short, metrics, _, used) = run(bytes, work, program, function, &expected);
        assert!(short.1 && !metrics.invalid);
        assert!(metrics.allocated <= used);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}
fn parsed(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "paid-summary.spx").unwrap()).unwrap()
}
#[test]
fn full_original_inventory_summary_preserves_complete_function_receiver_and_paid_drop() {
    for program in super::super::tests::fixtures() {
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            control(&program, function);
        }
    }
    let program = parsed(include_str!(
        "../../../../../tests/fixtures/owned-nested-outcome.spx"
    ));
    for function in &program.functions {
        control(&program, function);
    }
}
#[test]
fn full_generic_zero_one_seventeen_arguments_missing_arity_and_late_error_keep_original_diagnostics(
) {
    let program = parsed(
        r#"module summary.generic;
@id("summary.g") record G<T> { @id("summary.g.x") x:T, }
@id("summary.take") fn take(value:own G<Bytes>)->i64 {0}
@id("summary.main") fn main()->i64 {0}
"#,
    );
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "summary.take")
        .unwrap();
    assert!(old(&program, function).is_ok());
    control(&program, function);
    for width in [0, 1, 17] {
        let mut changed = function.clone();
        changed.return_type = ResolvedType::Nominal {
            declaration: DeclarationId::new("summary.g"),
            arguments: vec![ResolvedType::Bytes; width],
        };
        control(&program, &changed);
    }
    let mut changed = function.clone();
    changed.return_type = ResolvedType::Nominal {
        declaration: DeclarationId::new("missing.λ\0.suffix"),
        arguments: vec![],
    };
    assert!(old(&program, &changed).is_err());
    control(&program, &changed);
}
#[test]
fn authenticated_summary_ignores_attached_inventory_and_plan_but_keeps_full_expression_walk() {
    for program in super::super::tests::fixtures() {
        for function in &program.functions {
            let expected = old(&program, function);
            let mut detached = function.clone();
            detached.cleanup = crate::cleanup::CleanupInventory::unresolved();
            detached.cleanup_plan = crate::cleanup_plan::CleanupPlan::unresolved();
            assert_eq!(old(&program, &detached), expected);
            control(&program, &detached);
        }
    }
}
#[test]
fn complete_old_inventory_and_generic_shape_sources_remain_independent_byte_exact() {
    assert_eq!(
        include_str!("../../../../cleanup/inventory_builder.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/inventory_builder.rs.before")
    );
    assert_eq!(
        include_str!("../../../../cleanup/generic_record.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/generic_record.rs.before")
    );
}

fn inspect_stream(
    shape: &crate::cleanup::FieldLivenessShape,
    depth: usize,
    tracker: &mut profile::Tracker,
    payer: &mut Payer<'_>,
) -> Result<(), Diagnostic> {
    metadata::step(payer, 1)?;
    match shape {
        crate::cleanup::FieldLivenessShape::NoDrop => {}
        crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
            tracker.leaf(lifecycle.as_str(), depth, payer)?
        }
        crate::cleanup::FieldLivenessShape::Record { fields, .. } => {
            let next = tracker.record(depth, fields.len(), payer)?;
            for field in fields {
                inspect_stream(&field.shape, next, tracker, payer)?;
            }
        }
        crate::cleanup::FieldLivenessShape::Variant { cases, .. } => {
            tracker.variant_shape(cases, payer)?;
            for case in cases {
                for field in &case.fields {
                    inspect_stream(&field.shape, depth, tracker, payer)?;
                }
            }
        }
    }
    Ok(())
}
#[test]
fn streaming_profile_counts_every_variant_field_nodrop_and_preserves_nested_byte_limits() {
    for program in super::super::tests::fixtures() {
        for function in &program.functions {
            let inventory = crate::cleanup::build_inventory(&program, function).unwrap();
            let extra = [
                nested(2, 0),
                nested(2, 1),
                nested(65, 1),
                nested(2, 256),
                nested(2, 257),
                nested(2, 4097),
            ];
            for shape in inventory
                .slots
                .iter()
                .map(|slot| &slot.shape)
                .chain(extra.iter())
            {
                let expected = crate::cleanup::cleanup_shape_profile(shape);
                let (actual, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        let mut budget = ReplayBudget::new();
                        budget
                            .charge(function, 0, "streaming profile oracle")
                            .unwrap();
                        let mut payer = super::super::tests::payer(function, &mut budget);
                        let mut tracker = profile::Tracker::default();
                        inspect_stream(shape, 0, &mut tracker, &mut payer)
                            .and_then(|()| tracker.finish(&mut payer).map(|(profile, _)| profile))
                    });
                assert!(!refused);
                assert_eq!(actual, expected);
            }
        }
    }
}

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
