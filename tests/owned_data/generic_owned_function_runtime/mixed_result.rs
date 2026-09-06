//! GEN-06A mixed owned Result propagation and exact conditional settlement.
use super::*;

fn source(error: &str, mode: usize, bypass: bool) -> String {
    let error_value = match error {
        "i64" => "7",
        "i32" => "7i32",
        "u8" => "7u8",
        "usize" => "7usize",
        "char" => "'x'",
        "f32" => "1.5f32",
        "f64" => "1.5f64",
        "bool" => "true",
        "Bytes" => "bytes_copy(array_as_slice(input))",
        _ => unreachable!(),
    };
    let inspect_error = match error {
        "i64" => "error == 7",
        "i32" => "error == 7i32",
        "u8" => "error == 7u8",
        "usize" => "error == 7usize",
        "char" => "error == 'x'",
        "f32" => "error == 1.5f32",
        "f64" => "error == 1.5f64",
        "bool" => "error",
        "Bytes" => "byte_len(bytes_as_slice(error)) == 1usize",
        _ => unreachable!(),
    };
    let post = mode != 1 && mode != 2;
    let call = |ok: bool, divisor: i64, allowed: bool| {
        if bypass {
            format!("consume(make({ok}))")
        } else {
            format!("consume(outer<{error}>(make({ok}), {divisor}, {allowed}))")
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
module test.generic_mixed_result;
@id("mixed.propagate")
fn propagate<E>(value: own Result<Bytes, E>, divisor: i64, allowed: bool) -> Result<Bytes, E>
  requires allowed
  ensures {post}
{{
  let payload = value?;
  let _ = 1 / divisor;
  Result<Bytes, E>::Ok {{ value: payload }}
}}
@id("mixed.middle")
fn middle<E>(value: own Result<Bytes, E>, divisor: i64, allowed: bool) -> Result<Bytes, E> {{
  propagate<E>(value, divisor, allowed)
}}
@id("mixed.outer")
fn outer<E>(value: own Result<Bytes, E>, divisor: i64, allowed: bool) -> Result<Bytes, E> {{
  middle<E>(value, divisor, allowed)
}}
@id("mixed.make") fn make(ok: bool) -> Result<Bytes, {error}> {{
  let input = [9u8];
  if ok {{ Result<Bytes, {error}>::Ok {{ value: bytes_copy(array_as_slice(input)) }} }}
  else {{ Result<Bytes, {error}>::Err {{ error: {error_value} }} }}
}}
@id("mixed.consume") fn consume(value: own Result<Bytes, {error}>) -> i64 {{
  match own value {{
    Result::Ok {{ value: payload }} => if byte_len(bytes_as_slice(payload)) == 1usize {{ 1 }} else {{ 0 }},
    Result::Err {{ error: error }} => if {inspect_error} {{ 1 }} else {{ 0 }},
  }}
}}
@id("app.main") fn main() -> i64 {{ {entry} }}
"#
    )
}

#[test]
fn generic_mixed_result_success_residual_and_failures_settle_on_all_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node, "mixed Result corpus requires clang and Node");
    }
    for error in [
        "i64", "i32", "u8", "usize", "char", "f32", "f64", "bool", "Bytes",
    ] {
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
fn generic_mixed_result_empty_error_flags_and_residual_transfers_fail_closed() {
    use semaprax::cleanup_plan::CleanupTransition;
    for error in [
        "i64", "i32", "u8", "usize", "char", "f32", "f64", "bool", "Bytes",
    ] {
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
        assert_eq!(cases[0].live_flags.len(), 1);
        assert_eq!(cases[1].live_flags.len(), usize::from(error == "Bytes"));
        let mut wrong_flags = program.clone();
        let cases = &mut wrong_flags.function_instances[index]
            .function
            .cleanup
            .entry_state
            .conditional_owned_parameters[0]
            .cases;
        if error == "Bytes" {
            cases[1].live_flags.clear();
        } else {
            let ok_flag = cases[0].live_flags[0];
            cases[1].live_flags.push(ok_flag);
        }
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
