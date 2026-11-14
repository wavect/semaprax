use super::interpret;
const SOURCE: &str = r#"module lexical.finite.scratch;
@id("app.main") fn main()->i64 ensures result==4 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let mut count=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(3usize);
  count=count+2;
  if byte_len(bytes_as_slice(item))==1usize {continue;};
  if byte_len(bytes_as_slice(scratch))!=3usize {return 0;};
  0
 }
 return count;
}
"#;
#[test]
fn finite_iteration_scratch_preserves_cumulative_allocation_settlement_and_reentry() {
    interpret(SOURCE, 4, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(SOURCE, 0, 4, "none", 4);
    let failure = SOURCE.replace("return count;", "return count-1;");
    interpret(&failure, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failure, 10, 4, "none", 0);
    interpret(SOURCE, 4, false);
}

#[test]
fn finite_scratch_runs_at_both_cumulative_limits_without_refunding() {
    use crate::owned_vec_bytes_runtime::{run_native, run_wasm_expected};

    let source = |rows: usize, first_bytes: usize, extra_empty: bool| {
        let mut text =
            String::from("module finite.scratch.boundary; @id(\"app.main\") fn main()->i64 {\n");
        if extra_empty {
            text.push_str("let extra=bytes_zeroed(0usize);\n");
        }
        let mut previous = format!("vec_with_capacity<Bytes>({rows}usize)");
        for row in 0..rows {
            let name = format!("row{row}");
            let input_bytes = if row == 0 { first_bytes } else { 0 };
            text.push_str(&format!(
                "let {name}=vec_push<Bytes>({previous},bytes_zeroed({input_bytes}usize));\n"
            ));
            previous = name;
        }
        text.push_str(&format!(
            r#"let values={previous};
let mut seen=0;
for own item in vec_into_iter<Bytes>(values) {{
    let scratch=bytes_zeroed(131072usize);
    if byte_len(bytes_as_slice(scratch))!=131072usize {{return -1;}};
    seen=seen+1;
    0
}}
return seen;
}}"#
        ));
        text
    };
    for (rows, result, events) in [(0, 0, 0), (16, 16, 32)] {
        let input = source(rows, 0, false);
        interpret(&input, result, false);
        run_native(&input, "", 0, result, "none");
        run_wasm_expected(&input, 0, events, "none", result);
    }

    // Exceed the event and payload caps independently by exactly one.
    for input in [source(16, 0, true), source(16, 1, false)] {
        let diagnostics = semaprax::check(&input, "finite-scratch-over-limit.spx").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T267"));
    }
}

#[test]
fn finite_scratch_return_settles_live_item_and_suffix_with_sticky_failure() {
    use super::interpret_status;
    use crate::owned_vec_bytes_runtime::{run_native, run_wasm_expected};

    // Return on the first item, with both the iteration scratch and the
    // untransferred second item still live. A zero divisor fails while
    // evaluating the return operand, before the false ensures can run.
    for (divisor, ensures, domain, code, wasm_status) in [
        (2, 4, "", 0, 0),
        (2, 5, "semaprax.contract.v1", 2, 10),
        (0, 5, "semaprax.arithmetic.v1", 4, 4),
    ] {
        let source = SOURCE
            .replace("ensures result==4", &format!("ensures result=={ensures}"))
            .replace(
                "if byte_len(bytes_as_slice(item))==1usize {continue;};",
                &format!(
                    "if byte_len(bytes_as_slice(item))==1usize {{let divisor={divisor}; return 8/divisor;}};"
                ),
            );
        interpret_status(&source, 4, (code != 0).then_some((domain, code)));
        run_native(&source, domain, code, 4, "none");
        run_wasm_expected(&source, wasm_status, 3, "none", 4);
    }
}
