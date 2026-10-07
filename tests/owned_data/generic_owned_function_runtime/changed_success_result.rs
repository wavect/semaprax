//! Owned Result propagation reconstructs Err when the enclosing Ok type changes.
use super::*;

fn source(success: &str, mode: usize, manual: bool) -> String {
    let (computed, ok_value, inspect_ok) = match success {
        "bool" => (
            "byte_len(bytes_as_slice(payload)) == 0usize",
            "computed",
            "if value { 10 } else { 11 }",
        ),
        "i64" => (
            "if byte_len(bytes_as_slice(payload)) == 0usize { 20 } else { 22 }",
            "computed",
            "value",
        ),
        _ => unreachable!(),
    };
    let post = mode != 1 && mode != 2;
    let route = if manual { "manual" } else { "convert" };
    let entry = match mode {
        // Empty/nonempty success plus an Err whose zero divisor proves that
        // propagation skips every later expression.
        0 => format!(
            "consume({route}(make(0), 1)) + consume({route}(make(1), 1)) + consume({route}(make(2), 0))"
        ),
        // Postconditions run for both the ordinary and propagated-Err lanes.
        1 => format!("consume({route}(make(0), 1))"),
        2 => format!("consume({route}(make(2), 0))"),
        // The success payload is already extracted when this failure is selected.
        3 => format!("consume({route}(make(1), 0))"),
        _ => unreachable!(),
    };
    format!(
        r#"
module test.changed_success_result;
@id("changed.make") fn make(kind: i64) -> Result<Bytes, Bytes> {{
  let input = [4u8, 5u8];
  let error = [9u8];
  if kind == 0 {{ Result<Bytes, Bytes>::Ok {{ value: bytes_zeroed(0usize) }} }}
  else if kind == 1 {{ Result<Bytes, Bytes>::Ok {{ value: bytes_copy(array_as_slice(input)) }} }}
  else {{ Result<Bytes, Bytes>::Err {{ error: bytes_copy(array_as_slice(error)) }} }}
}}
@id("changed.convert")
fn convert(value: own Result<Bytes, Bytes>, divisor: i64) -> Result<{success}, Bytes>
  ensures {post}
{{
  let payload = value?;
  let unrelated_input = [7u8];
  let unrelated = bytes_copy(array_as_slice(unrelated_input));
  let _ = 1 / divisor;
  let computed = {computed};
  Result<{success}, Bytes>::Ok {{ value: {ok_value} }}
}}
@id("changed.manual")
fn manual(value: own Result<Bytes, Bytes>, divisor: i64) -> Result<{success}, Bytes>
  ensures {post}
{{
  match own value {{
    Result::Ok {{ value: payload }} => {{
      let unrelated_input = [7u8];
      let unrelated = bytes_copy(array_as_slice(unrelated_input));
      let _ = 1 / divisor;
      let computed = {computed};
      Result<{success}, Bytes>::Ok {{ value: {ok_value} }}
    }},
    Result::Err {{ error: error }} => Result<{success}, Bytes>::Err {{ error: error }},
  }}
}}
@id("changed.consume") fn consume(value: own Result<{success}, Bytes>) -> i64 {{
  match own value {{
    Result::Ok {{ value: value }} => {inspect_ok},
    Result::Err {{ error: payload }} => {{
      let view = bytes_as_slice(payload);
      match byte_get(view, 0usize) {{
        Option::Some {{ value: byte }} => if byte == 9u8 {{ 12 }} else {{ 0 }},
        Option::None {{}} => 0,
      }}
    }},
  }}
}}
@id("app.main") fn main() -> i64 {{ {entry} }}
"#
    )
}

fn expected(success: &str, mode: usize) -> Expected {
    match mode {
        0 => Expected::Value(if success == "bool" { 33 } else { 54 }),
        1 | 2 => Expected::Failure("semaprax.contract.v1", 2, "SEMAPRAX contract failure"),
        3 => Expected::Failure(
            "semaprax.arithmetic.v1",
            4,
            "SEMAPRAX checked arithmetic failure: invalid division",
        ),
        _ => unreachable!(),
    }
}

#[test]
fn changed_success_result_preserves_error_and_settles_every_lane_on_all_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(
            clang && node,
            "changed-success Result corpus requires clang and Node"
        );
    }
    for success in ["bool", "i64"] {
        for mode in 0..4 {
            let text = source(success, mode, false);
            let parsed = semaprax::check(&text, "changed-success-result.spx").unwrap();
            let resolved = hir::resolve(&parsed).unwrap();
            hir::validate(&resolved).unwrap();
            let expected = expected(success, mode);
            run_interpreter_source("changed-success-result", &text, expected);
            if clang {
                // The shared native harness executes O0/O2 four times each and
                // asserts that every tracked allocation is released.
                run_native(&parsed, expected);
            }
            if node {
                // A manual `match own` implementation is the independent
                // memory-copy baseline for the new typed reconstruction.
                run_wasm_source(&parsed, &source(success, mode, true), expected);
            }
        }
    }
}

#[test]
fn changed_success_result_settles_live_bytes_when_later_allocation_is_refused() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(
            clang && node,
            "changed-success allocation refusal requires clang and Node"
        );
    }
    if !clang || !node {
        return;
    }
    let text = r#"
module test.changed_success_refusal;
@id("changed.refusal.make") fn make() -> Result<Bytes, Bytes> {
  let input = [4u8, 5u8];
  Result<Bytes, Bytes>::Ok { value: bytes_copy(array_as_slice(input)) }
}
@id("changed.refusal.convert")
fn convert(value: own Result<Bytes, Bytes>) -> Result<bool, Bytes> {
  let payload = value?;
  let first = vec_with_capacity<i64>(1usize);
  let refused = vec_with_capacity<i64>(1usize);
  let empty = byte_len(bytes_as_slice(payload)) == 0usize;
  Result<bool, Bytes>::Ok { value: empty }
}
@id("changed.refusal.consume") fn consume(value: own Result<bool, Bytes>) -> i64 {
  match own value {
    Result::Ok { value: value } => if value { 1 } else { 2 },
    Result::Err { error: error } => 3,
  }
}
@id("app.main") fn main() -> i64 { consume(convert(make())) }
"#;
    // This owning harness refuses the second Vec allocation on native O0/O2
    // and raw Core Wasm, repeats four times, and checks both Vec and Bytes
    // owner inventories return to zero without replacing the Vec status.
    collections::run_backend_output_allocation_refusal(text);
}
