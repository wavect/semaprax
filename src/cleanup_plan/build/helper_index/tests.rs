use super::*;
const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "construction-index.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn construction_index_reuses_exact_sites_and_preserves_independent_plan_bytes() {
    for source in [
        SOURCE,
        r#"module ordinary_phase;
@id("main") fn main()->i64 {let mut values=vec_with_capacity<i64>(2usize);let mut bytes=bytes_zeroed(2usize);let mut i=0;
 while i<2 {values=vec_push<i64>(values,i);bytes=bytes_set(bytes,0usize,65u8);i=i+1;0} 0}
"#,
    ] {
        let program = program(source);

        let index = ModuleIndex::new(&program).unwrap();
        let actual = build_program_plans(&program).unwrap();
        for (function, plan) in program.functions.iter().zip(&actual) {
            let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
            let direct =
                lexical_control::build_plan_for_execution(&program, function, &execution).unwrap();
            assert_eq!(plan, &direct);
            assert_eq!(
                crate::cache_codec::encode(plan).unwrap(),
                crate::cache_codec::encode(&direct).unwrap()
            );
            let builder = PlanBuilder::with_index(&program, function, &index).unwrap();
            let independently =
                crate::loop_calls::byte_writer::checked_bindings(&program, function).unwrap();
            for (at, binding) in independently {
                let (found, overflowed, used) =
                    crate::bounded_output::with_limit_usage(0, || builder.renewal_binding(at));
                assert_eq!(found, Some(binding));
                assert!(!overflowed);
                assert_eq!(used, 0);
            }
            // Construction keeps this program's own backing, not a cloned function.
            if source == SOURCE {
                assert!(matches!(builder.byte_helpers, HelperSites::Indexed { .. }));
            } else {
                assert!(builder.ordinary_index.is_some());
                assert!(builder.byte_helpers.is_empty());
            }
        }
        crate::hir::validate(&program).unwrap();
        crate::cleanup_plan::validate_program(&program).unwrap();
        crate::cleanup_plan::replay::phase_measurement::replay(&program, false).unwrap();
        crate::cleanup_plan::replay::phase_measurement::replay(&program, true).unwrap();
    }
}

#[test]
fn construction_exact_one_short_and_zero_budget_leave_attached_plans_unchanged() {
    let mut program = program(SOURCE);
    let original = program
        .functions
        .iter()
        .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
        .collect::<Vec<_>>();
    let (plans, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || build_program_plans(&program));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert!(required > 0);
    let (plans, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || build_program_plans(&program));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (failure, overflowed, _) = crate::bounded_output::with_limit_usage(required - 1, || {
        attach_program_plans(&mut program)
    });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(
        original,
        program
            .functions
            .iter()
            .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    );
    let (failure, overflowed, _) =
        crate::bounded_output::with_limit_usage(0, || attach_program_plans(&mut program));
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
}
#[test]
fn construction_rejects_copied_function_foreign_program_and_reproves_changed_provider() {
    let program = program(SOURCE);
    let index = ModuleIndex::new(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let copy = function.clone();
    let error = PlanBuilder::with_index(&program, &copy, &index)
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-H006");
    let foreign = program.clone();
    let error = PlanBuilder::with_index(&foreign, function, &index)
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-H006");
    drop(index);
    let mut changed = program.clone();
    // Exact helper call remains in its body but the target inventory is missing.
    changed.functions.retain(|f| f.id.as_str() != "helper.leaf");
    let rebuilt = ModuleIndex::new(&changed).unwrap();
    let main = changed
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(
        !rebuilt.has_sites(main),
        "missing provider cannot retain helper authority"
    );
    assert!(
        crate::hir::validate(&changed).is_err(),
        "missing call target must fail independent validation"
    );
}
#[test]
fn irrelevant_and_allocating_legacy_helpers_keep_old_schemas_and_canonical_order() {
    let rejected = crate::check(
        "module old;@id(\"old\") fn old(input:own Bytes)->Bytes{let size=byte_len(bytes_as_slice(input));bytes_zeroed(1usize)} @id(\"main\") fn main()->i64{let mut output=bytes_zeroed(1usize);output=old(output);0}",
        "allocating-renewal.spx",
    ).unwrap_err();
    assert!(rejected.iter().any(|error| error.code == "SPX-U105"));
    for source in ["module plain;@id(\"main\") fn main()->i64{0}",
        "module old;@id(\"old\") fn old(input:own Bytes)->Bytes{let size=byte_len(bytes_as_slice(input));bytes_zeroed(1usize)} @id(\"main\") fn main()->i64{let mut output=bytes_zeroed(1usize);let next=old(output);i64_from_usize(byte_len(bytes_as_slice(next)))}"] {
        let program=program(source);
        let index=ModuleIndex::new(&program).unwrap();
        if source.starts_with("module plain") {assert!(index.is_empty());assert_eq!(index.owned_capacity_bytes(),7*std::mem::size_of::<usize>());}
        let plans=build_program_plans(&program).unwrap();
        for (function,plan) in program.functions.iter().zip(&plans) {
            assert_ne!(plan.schema,super::super::super::CLEANUP_PLAN_SCHEMA_V19);
            assert_eq!(plan,&function.cleanup_plan);
        }
    }
}

#[test]
fn old_attached_spare_backing_refuses_before_any_index_or_builder_starts() {
    let mut program = program(SOURCE);
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    main.cleanup_plan.blocks.reserve_exact(1024);
    let before = program
        .functions
        .iter()
        .map(|function| crate::cache_codec::encode(&function.cleanup_plan).unwrap())
        .collect::<Vec<_>>();
    let (overlap, overflowed, scratch) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            attached_capacity::program(&program)
        });
    let overlap = overlap.unwrap();
    assert!(!overflowed);
    assert!(overlap >= 1024 * std::mem::size_of::<crate::cleanup_plan::CleanupBlock>());
    CONSTRUCTION_STARTED.with(|started| started.set(false));
    let (failure, overflowed, used) =
        crate::bounded_output::with_limit_usage(scratch + overlap - 1, || {
            attach_program_plans(&mut program)
        });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(used, scratch);
    assert!(
        !CONSTRUCTION_STARTED.with(|started| started.get()),
        "old-plan physical refusal must precede index/staging/builder allocations"
    );
    assert_eq!(
        before,
        program
            .functions
            .iter()
            .map(|function| crate::cache_codec::encode(&function.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    );
}

#[test]
fn attached_owned_variant_spare_backing_rebuilds_at_exact_phase_reservation() {
    let mut program = program(
        r#"module overlap;
@id("maybe") fn maybe(input: borrow Slice<u8>, active: bool) -> Option<Bytes> {
    if active { Option<Bytes>::Some { value: bytes_copy(input) } }
    else { Option<Bytes>::None {} }
}
@id("identity") fn identity<T>(value: T) -> T { value }
@id("main") fn main() -> i64 { identity<i64>(1) }
"#,
    );
    assert!(!program.function_instances.is_empty());
    crate::hir::validate(&program).unwrap();
    let plan = &mut program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "maybe")
        .unwrap()
        .cleanup_plan;
    let before_capacity =
        crate::private_capacity_contract::cleanup_plan_owned_capacity(plan).unwrap();
    let shape = plan
        .slots
        .iter_mut()
        .find_map(|slot| match &mut slot.field_liveness_shape {
            crate::cleanup::FieldLivenessShape::Variant { cases, .. } => Some(cases),
            _ => None,
        })
        .expect("owned Option has an attached conditional inventory");
    let old_cases = shape.capacity();
    shape.reserve_exact(7);
    let case_delta =
        (shape.capacity() - old_cases) * std::mem::size_of::<crate::cleanup::VariantCaseLiveness>();
    let fields = &mut shape
        .iter_mut()
        .find(|case| !case.fields.is_empty())
        .unwrap()
        .fields;
    let old_fields = fields.capacity();
    fields.reserve_exact(9);
    let field_delta =
        (fields.capacity() - old_fields) * std::mem::size_of::<crate::cleanup::FieldLiveness>();
    assert_eq!(
        crate::private_capacity_contract::cleanup_plan_owned_capacity(plan).unwrap()
            - before_capacity,
        case_delta + field_delta
    );
    let bytes = |program: &ResolvedProgram| {
        program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .map(|function| crate::cache_codec::encode(&function.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    };
    let original = bytes(&program);
    let (plans, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || build_program_plans(&program));
    assert!(plans.is_ok());
    assert!(!overflowed);
    let (failure, overflowed, _) = crate::bounded_output::with_limit_usage(required - 1, || {
        attach_program_plans(&mut program)
    });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(bytes(&program), original);
    let (rebuilt, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || attach_program_plans(&mut program));
    rebuilt.unwrap();
    assert!(!overflowed);
    assert_eq!(used, required);
    assert_eq!(bytes(&program), original);
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
}

#[test]
fn prior_sticky_refusal_prevents_construction_and_publication() {
    let mut program = program(SOURCE);
    let original = program
        .functions
        .iter()
        .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
        .collect::<Vec<_>>();
    CONSTRUCTION_STARTED.with(|started| started.set(false));
    let (failure, overflowed, used, refusal) =
        crate::bounded_output::with_limit_usage_refusal(1, || {
            assert!(!crate::bounded_output::reserve_active_required(2));
            // Required work stays refused while an optional error trailer can
            // consume the remaining byte without clearing the first receipt.
            assert!(!crate::bounded_output::reserve_active_required(1));
            assert!(crate::bounded_output::reserve_active(1));
            attach_program_plans(&mut program)
        });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(used, 1);
    let refusal = refusal.unwrap();
    assert_eq!(refusal.requested, 2);
    assert_eq!(refusal.remaining, 1);
    assert!(!CONSTRUCTION_STARTED.with(|started| started.get()));
    assert_eq!(
        original,
        program
            .functions
            .iter()
            .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    );
}
