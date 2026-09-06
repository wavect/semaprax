//! GEN-06B Copy-success owned-error Result propagation and exact conditional settlement.
use super::*;

fn source(success: &str, mode: usize, bypass: bool) -> String {
    let success_value = match success {
        "i64" => "7",
        "i32" => "7i32",
        "u8" => "7u8",
        "usize" => "7usize",
        "char" => "'x'",
        "f32" => "1.5f32",
        "f64" => "1.5f64",
        "bool" => "true",
        _ => unreachable!(),
    };
    let inspect_success = match success {
        "i64" => "payload == 7",
        "i32" => "payload == 7i32",
        "u8" => "payload == 7u8",
        "usize" => "payload == 7usize",
        "char" => "payload == 'x'",
        "f32" => "payload == 1.5f32",
        "f64" => "payload == 1.5f64",
        "bool" => "payload",
        _ => unreachable!(),
    };
    let post = mode != 1 && mode != 2;
    let call = |ok: bool, divisor: i64, allowed: bool| {
        if bypass {
            format!("consume(make({ok}))")
        } else {
            format!("consume(outer<{success}>(make({ok}), {divisor}, {allowed}))")
        }
    };
    let entry = match mode {
        0 => format!("{} + {}", call(true, 1, true), call(false, 1, true)),
        1 => call(true, 1, true),
        2 => call(false, 1, true),
        3 => call(true, 0, true),
        4 => call(false, 1, false),
        _ => unreachable!(),
    };
    format!(
        r#"
module test.generic_copy_success_result;
@id("mixed.propagate")
fn propagate<T>(value: own Result<T, Bytes>, divisor: i64, allowed: bool) -> Result<T, Bytes>
  requires allowed
  ensures {post}
{{
  let payload = value?;
  let _ = 1 / divisor;
  Result<T, Bytes>::Ok {{ value: payload }}
}}
@id("mixed.middle")
fn middle<T>(value: own Result<T, Bytes>, divisor: i64, allowed: bool) -> Result<T, Bytes> {{
  propagate<T>(value, divisor, allowed)
}}
@id("mixed.outer")
fn outer<T>(value: own Result<T, Bytes>, divisor: i64, allowed: bool) -> Result<T, Bytes> {{
  middle<T>(value, divisor, allowed)
}}
@id("mixed.make") fn make(ok: bool) -> Result<{success}, Bytes> {{
  let input = [9u8];
  if ok {{ Result<{success}, Bytes>::Ok {{ value: {success_value} }} }}
  else {{ Result<{success}, Bytes>::Err {{ error: bytes_copy(array_as_slice(input)) }} }}
}}
@id("mixed.consume") fn consume(value: own Result<{success}, Bytes>) -> i64 {{
  match own value {{
    Result::Ok {{ value: payload }} => if {inspect_success} {{ 1 }} else {{ 0 }},
    Result::Err {{ error: error }} => if byte_len(bytes_as_slice(error)) == 1usize {{ 1 }} else {{ 0 }},
  }}
}}
@id("app.main") fn main() -> i64 {{ {entry} }}
"#
    )
}

#[test]
fn generic_copy_success_result_success_residual_and_failures_settle_on_all_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node, "mixed Result corpus requires clang and Node");
    }
    for error in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        for mode in 0..5 {
            let text = source(error, mode, false);
            let parsed = semaprax::check(&text, "generic-mixed-result.spx").unwrap();
            let program = hir::resolve(&parsed).unwrap();
            hir::validate(&program).unwrap();
            assert_eq!(program.function_instances.len(), 3);
            for instance in &program.function_instances {
                assert_eq!(
                    instance.function.cleanup_plan.schema,
                    "semaprax.cleanup-plan.v6"
                );
            }
            let expected = match mode {
                0 => Expected::Value(2),
                1 | 2 => Expected::Failure("semaprax.contract.v1", 2, "SEMAPRAX contract failure"),
                3 => Expected::Failure(
                    "semaprax.arithmetic.v1",
                    4,
                    "SEMAPRAX checked arithmetic failure: invalid division",
                ),
                4 => Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure"),
                _ => unreachable!(),
            };
            run_interpreter_source("mixed-result", &text, expected);
            if clang {
                run_native(&parsed, expected);
            }
            if node {
                run_wasm_source(&parsed, &source(error, mode, true), expected);
            }
        }
    }
}

#[test]
fn generic_copy_success_result_empty_success_flags_and_residual_transfers_fail_closed() {
    use semaprax::cleanup_plan::CleanupTransition;
    for error in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        let parsed = semaprax::check(&source(error, 0, false), "mixed-result-hostile.spx").unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let index = program
            .function_instances
            .iter()
            .position(|i| i.template.as_str() == "mixed.propagate")
            .unwrap();
        let cases = &program.function_instances[index]
            .function
            .cleanup
            .entry_state
            .conditional_owned_parameters[0]
            .cases;
        assert_eq!(cases[0].live_flags.len(), 0);
        assert_eq!(cases[1].live_flags.len(), 1);
        let mut wrong_flags = program.clone();
        let cases = &mut wrong_flags.function_instances[index]
            .function
            .cleanup
            .entry_state
            .conditional_owned_parameters[0]
            .cases;
        let err_flag = cases[1].live_flags[0];
        cases[0].live_flags.push(err_flag);
        let mut missing_transfer = program.clone();
        let blocks = &mut missing_transfer.function_instances[index]
            .function
            .cleanup_plan
            .blocks;
        let block = blocks
            .iter_mut()
            .find(|b| {
                b.transitions
                    .iter()
                    .any(|t| matches!(t, CleanupTransition::TransferVariant { .. }))
            })
            .unwrap();
        let transfer = block
            .transitions
            .iter()
            .position(|t| matches!(t, CleanupTransition::TransferVariant { .. }))
            .unwrap();
        block.transitions.remove(transfer);
        for hostile in [wrong_flags, missing_transfer] {
            assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
            assert_eq!(
                interpreter::evaluate_resolved_owned_data(&hostile, "mixed.make", &[], 10_000)
                    .unwrap_err()[0]
                    .code,
                "SPX-H006",
            );
            assert_eq!(codegen::emit_hir_c(&hostile).unwrap_err().code, "SPX-H006");
            assert_eq!(
                wasm::emit_resolved_module(&hostile).unwrap_err().code,
                "SPX-H006"
            );
        }
    }
}
