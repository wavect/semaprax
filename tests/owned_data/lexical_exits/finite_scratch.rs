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

    // A zero-sized allocation in either contract still consumes an event.
    let exact = source(16, 0, false);
    for contract in ["requires", "ensures"] {
        let hidden = exact.replace(
            "@id(\"app.main\") fn main()->i64 {",
            &format!(
                r#"@id("app.check") fn allocation_check() -> bool {{
    let hidden = bytes_zeroed(0usize);
    byte_len(bytes_as_slice(hidden)) == 0usize
}}
@id("app.main") fn main()->i64 {contract} allocation_check() {{"#
            ),
        );
        let diagnostics =
            semaprax::check(&hidden, "finite-scratch-contract-limit.spx").unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T267"
                && diagnostic.message.contains("33 sites")));
    }
}

#[test]
fn finite_scratch_refuses_return_allocation_outer_storage_and_recursive_helpers() {
    let outer = SOURCE
        .replace(
            " let mut count=0;",
            " let mut saved=vec_with_capacity<Bytes>(2usize); let mut count=0;",
        )
        .replace(
            "  if byte_len(bytes_as_slice(scratch))!=3usize {return 0;};",
            "  saved=vec_push<Bytes>(saved,scratch);",
        );
    let returned = SOURCE.replace(
        "{return 0;};",
        "{return {let hidden=bytes_zeroed(1usize); 0};};",
    );
    let recursive = SOURCE.replace(
        " return count;",
        " return if count==4 {count} else {main()};",
    );
    for source in [outer, returned, recursive] {
        let diagnostics = semaprax::check(&source, "finite-scratch-hostile.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T267"),
            "{diagnostics:?}"
        );
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

#[test]
fn finite_scratch_returns_the_current_owner_after_settling_its_suffix() {
    use super::interpret_status;
    use crate::owned_vec_bytes_runtime::{run_native, run_wasm_expected};

    let source = r#"module lexical.finite.returned;
@id("app.take") fn take() -> Bytes
    ensures byte_len(bytes_as_slice(result)) == 1usize {
    let first = vec_push<Bytes>(vec_with_capacity<Bytes>(2usize), bytes_zeroed(1usize));
    let values = vec_push<Bytes>(first, bytes_zeroed(2usize));
    for own item in vec_into_iter<Bytes>(values) {
        let scratch = bytes_zeroed(3usize);
        if byte_len(bytes_as_slice(scratch)) == 3usize { return item; };
        0
    }
    bytes_zeroed(4usize)
}
@id("app.main") fn main() -> i64 {
    let chosen = take();
    i64_from_usize(byte_len(bytes_as_slice(chosen)))
}"#;
    interpret(source, 1, false);
    run_native(source, "", 0, 1, "none");
    run_wasm_expected(source, 0, 3, "none", 1);

    let failed = source.replace("== 1usize", "== 2usize");
    interpret_status(&failed, 0, Some(("semaprax.contract.v1", 2)));
    run_native(&failed, "semaprax.contract.v1", 2, 0, "none");
    run_wasm_expected(&failed, 10, 3, "none", 0);
}

#[test]
fn frozen_native_owned_data_and_stream_profiles_refuse_finite_scratch() {
    use semaprax::{codegen, hir, project, wasm};

    let source = r#"module lexical.finite.profile;
@id("app.scratch") fn scratch() -> Bytes {
    let values = vec_push<Bytes>(vec_with_capacity<Bytes>(1usize), bytes_zeroed(1usize));
    for own item in vec_into_iter<Bytes>(values) {
        let scratch = bytes_zeroed(3usize);
        if byte_len(bytes_as_slice(scratch)) == 3usize { return item; };
        0
    }
    bytes_zeroed(0usize)
}
@id("app.main") fn main() -> i64 {
    let owner = scratch();
    i64_from_usize(byte_len(bytes_as_slice(owner)))
}
"#;
    let ast = semaprax::check(source, "finite-scratch-profile.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    codegen::emit_hir_c(&program).unwrap();
    let digest = format!("sha256:{}", "0".repeat(64));
    let subject = project::PublicApiSubject {
        project_schema: project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: &digest,
        workspace_revision: &digest,
        project_graph_digest: &digest,
    };
    let selected = ["app.scratch".to_owned()];
    let descriptor = project::derive_public_api_descriptor(&program, &selected, subject).unwrap();
    let native = codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    );
    let error = native.expect_err("frozen native provider must refuse");
    assert_eq!(error.code, "SPX-B103");
    assert!(error.message.contains("finite owned-iterator scratch"));

    let stream_source = source.replace(
        "module lexical.finite.profile;",
        "module lexical.finite.profile;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }",
    ) + r#"
@id("app.command") fn command() -> i64 uses { process.stdin.read } {
    let reader = stdin_stream_open();
    let owner = scratch();
    i64_from_usize(byte_len(bytes_as_slice(owner)))
}
"#;
    let without_scratch = stream_source
        .replace("        let scratch = bytes_zeroed(3usize);\n", "")
        .replace("byte_len(bytes_as_slice(scratch)) == 3usize", "true");
    let control = semaprax::check(&without_scratch, "finite-scratch-stream-control.spx").unwrap();
    codegen::emit_hir_c_with_stdin_stream_owned_data(
        &hir::resolve(&control).unwrap(),
        "app.command",
    )
    .expect("the stream profile still admits the ordinary nonallocating loop");
    let stream = semaprax::check(&stream_source, "finite-scratch-stream.spx").unwrap();
    let stream = hir::resolve(&stream).unwrap();
    let error =
        codegen::emit_hir_c_with_stdin_stream_owned_data(&stream, "app.command").unwrap_err();
    assert_eq!(error.code, "SPX-B103");
    assert!(error.message.contains("finite owned-iterator scratch"));
    let error =
        wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor).unwrap_err();
    assert_eq!(error.code, "SPX-W115");
}
