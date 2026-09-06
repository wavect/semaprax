//! GEN-05B: freeze selection from semantic operations, not attached metadata.
use super::*;

fn program(ty: &str) -> ResolvedProgram {
    let scalar_ty = if ty == "bool" { "bool" } else { "i64" };
    let source = format!(
        r#"
module test.generic_cleanup_selection;
@id("pair") record Pair<T, U> {{
  @id("pair.payload") payload: T,
  @id("pair.marker") marker: U,
}}
@id("box") record Box<T> {{ @id("box.value") value: T, }}
@id("scalar") fn scalar<T>(value: T) -> T {{ value }}
@id("flat") fn flat<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ value }}
@id("matched") fn matched<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{
  match own value {{
    Pair {{ payload: payload, marker: marker }} =>
      Pair<Bytes, T> {{ payload: payload, marker: marker }},
  }}
}}
@id("nested") fn nested<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> {{ value }}
@id("invoke-scalar") fn invoke_scalar(value: {scalar_ty}) -> {scalar_ty} {{ scalar<{scalar_ty}>(value) }}
@id("invoke-flat") fn invoke_flat(value: own Pair<Bytes, {ty}>) -> Pair<Bytes, {ty}> {{ flat<{ty}>(value) }}
@id("invoke-matched") fn invoke_matched(value: own Pair<Bytes, {ty}>) -> Pair<Bytes, {ty}> {{ matched<{ty}>(value) }}
@id("invoke-nested") fn invoke_nested(value: own Box<Pair<Bytes, {ty}>>) -> Box<Pair<Bytes, {ty}>> {{ nested<{ty}>(value) }}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
    );
    let parsed = crate::check(&source, "generic-cleanup-selection.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}

#[test]
fn generic_instance_schema_selection_freezes_operation_sensitive_v2_v5_v7() {
    for ty in ["bool", "i64", "i32", "u8", "usize", "char", "f32", "f64"] {
        let program = program(ty);
        assert_eq!(program.function_instances.len(), 4);
        for instance in &program.function_instances {
            let expected = match instance.template.as_str() {
                "scalar" | "flat" => CLEANUP_PLAN_SCHEMA_V2,
                "matched" => CLEANUP_PLAN_SCHEMA_V5,
                "nested" => CLEANUP_PLAN_SCHEMA_V7,
                other => panic!("unexpected template {other}"),
            };
            assert_eq!(
                selected_schema(&program, &instance.function).unwrap(),
                expected
            );
            assert_eq!(instance.function.cleanup_plan.schema, expected);
            validate_structure(&program, &instance.function).unwrap();

            // Both attachments can be replaced together without changing the
            // independently selected semantic contract.
            let mut detached = instance.function.clone();
            detached.cleanup = crate::cleanup::CleanupInventory::unresolved();
            detached.cleanup_plan = super::super::CleanupPlan::unresolved();
            assert_eq!(selected_schema(&program, &detached).unwrap(), expected);
        }
    }
}

#[test]
fn generic_instance_schema_substitution_rejects_before_semantic_execution() {
    let program = program("bool");
    for (index, instance) in program.function_instances.iter().enumerate() {
        for schema in [
            CLEANUP_PLAN_SCHEMA_V2,
            CLEANUP_PLAN_SCHEMA_V5,
            CLEANUP_PLAN_SCHEMA_V7,
        ] {
            if schema == instance.function.cleanup_plan.schema {
                continue;
            }
            let mut hostile = program.clone();
            hostile.function_instances[index]
                .function
                .cleanup_plan
                .schema = schema;
            let function = &hostile.function_instances[index].function;
            let diagnostic = validate_structure(&hostile, function).unwrap_err();
            assert_eq!(diagnostic.code, "SPX-H006");
            assert!(diagnostic.message.contains("HIR-derived"));
            assert_eq!(crate::hir::validate(&hostile).unwrap_err().code, "SPX-H006");
            assert_eq!(
                crate::codegen::emit_hir_c(&hostile).unwrap_err().code,
                "SPX-H006"
            );
            assert_eq!(
                crate::wasm::emit_resolved_module(&hostile)
                    .unwrap_err()
                    .code,
                "SPX-H006"
            );
            let diagnostics =
                crate::interpreter::evaluate_resolved_zero_arg_i64(&hostile, "app.main", 10_000)
                    .unwrap_err();
            assert_eq!(diagnostics[0].code, "SPX-H006");
        }
    }
}
