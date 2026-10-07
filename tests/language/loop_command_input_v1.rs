//! Immutable argument and borrowed-text input in bounded loop bodies.

use std::path::Path;
use std::process::Command;

use semaprax::{format, graph, hir, parse, verify};

use super::owned_string_loops_v1::support::Fixture;

const SOURCE: &str = r#"
module test.loop_command_input;
permit { fs.read, process.args.read }

@id("loop.read_text")
fn read_text(path: borrow str) -> string
    uses { fs.read }
{
    file_read_text(path)
}

@id("loop.argument")
fn argument(index: usize) -> i64
    uses { process.args.read }
{
    let argument = arg_utf8(index);
    if str_is_empty(argument) { 0 } else { 1 }
}

@id("loop.read_all")
fn read_all() -> i64
    uses { fs.read, process.args.read }
{
    let mut index = 0usize;
    let mut total = 0usize;
    let mut arguments = 0;
    while index < args_len() {
        let kept = "iteration";
        let path = arg_utf8(index);
        let text = read_text(path);
        let view = string_as_str(text);
        let bytes = str_as_bytes(view);
        total = total + byte_len(bytes);
        arguments = arguments + argument(index);
        index = index + 1usize;
        0
    }
    if total == 8usize && arguments == 2 { 0 } else { 2 }
}

@id("loop.argument_failure")
fn argument_failure() -> i64
    uses { process.args.read }
{
    let mut index = 0usize;
    while index <= args_len() {
        let kept = "live-before-lookup";
        let argument = arg_utf8(index);
        let count = str_len_bytes(argument);
        index = index + 1usize;
        0
    }
    0
}

@id("app.main")
fn main() -> i64
    uses { fs.read, process.args.read }
{
    read_all()
}
"#;

#[test]
fn loop_input_round_trips_and_replays_named_views() {
    let program = parse(SOURCE, Path::new("loop-command-input.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("loop-command-input-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn loop_input_executes_with_explicit_arguments_and_files() {
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("one.txt", "abc");
    fixture.write("two.txt", "12345");
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&fixture.root)
        .args(["run", "source.spx", "--", "one.txt", "two.txt"])
        .output()
        .unwrap();
    if cfg!(windows) {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("file access denied"));
        fixture.cleanup();
        return;
    }
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    fixture.cleanup();
}

#[test]
fn core_wasm_refuses_loop_file_helpers_with_its_stable_profile_diagnostic() {
    let program = parse(SOURCE, Path::new("loop-command-input-wasm.spx")).unwrap();
    let error = semaprax::wasm::internal_strings::emit_module(
        &program,
        &["loop.read_all".to_owned()],
        semaprax::wasm::internal_strings::InternalStringOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W111");
    assert_eq!(
        error.message,
        "standalone String internal signature is outside the closed profile"
    );
}

#[test]
fn native_loop_input_settles_on_success_and_late_read_failure() {
    if cfg!(windows) || Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("loop-command-input-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c_with_source_command(&program).unwrap();
    let mut probe = format!(
        "#define _DARWIN_C_SOURCE 1\n#define _POSIX_C_SOURCE 200809L\n{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\n(void)spx_source_command_usage_v1; (void)spx_source_command_flush_v1;\nREQUIRE(fixture_binary_stdout());\nstruct spx_language_command_input_v1 input={{0}};\ninput.argument_count=2;\ninput.arguments[0]=(spx_str_v1){{.data=(const uint8_t*)\"one.txt\",.len=7}};\ninput.arguments[1]=(spx_str_v1){{.data=(const uint8_t*)\"two.txt\",.len=7}};\nREQUIRE(spx_language_command_input_is_valid_v1(&input));\nstruct spx_source_command_state_v1 state={{0}};\nstate.command.input=&input;\nstate.file_root=open(\".\",O_RDONLY|O_DIRECTORY|O_CLOEXEC);\nREQUIRE(state.file_root>=0);\nstruct spx_status_entry entries[32];\nstruct spx_context context={{0}};\nREQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,&state));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    for (id, domain, code) in [
        ("loop.read_all", "", 0),
        ("loop.argument_failure", "semaprax.command-input.v1", 1),
        ("loop.read_all", "semaprax.filesystem.v1", 2),
    ] {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if domain == "semaprax.filesystem.v1" {
            probe.push_str(
                "input.arguments[1]=(spx_str_v1){.data=(const uint8_t*)\"missing.txt\",.len=11};\n",
            );
        }
        probe.push_str(&format!(
            r#"
{{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if({code}==0) {{ REQUIRE(token==0 && value==0); }}
    else {{
        REQUIRE(token!=0 && value==INT64_MIN);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL && strcmp(status->domain_id,"{domain}")==0 && status->code=={code});
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
    }
    probe.push_str("REQUIRE(close(state.file_root)==0); return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("one.txt", "abc");
    fixture.write("two.txt", "12345");
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), "");
    }
    fixture.cleanup();
}

#[test]
fn loop_input_preserves_effect_ownership_and_single_read_refusals() {
    for (source, expected) in [
        (
            SOURCE.replace(
                "uses { fs.read, process.args.read }",
                "uses { process.args.read }",
            ),
            "SPX-E102",
        ),
        (
            SOURCE.replace(
                "let view = string_as_str(text);",
                "let view = string_as_str(\"temporary\");",
            ),
            "SPX-T266",
        ),
        (
            SOURCE.replace(
                "let path = arg_utf8(index);",
                "let path = arg_utf8(index); let data = stdin_read();",
            ),
            "SPX-T270",
        ),
        (
            SOURCE.replace(
                "let view = string_as_str(text);",
                "let view = string_as_str(text); let stolen = text;",
            ),
            "SPX-T265",
        ),
    ] {
        let program = parse(&source, Path::new("loop-command-input-refused.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected),
            "expected {expected}: {diagnostics:?}"
        );
    }
    let effectful = r#"module test.loop_write;
permit { process.stdout.write }
@id("write.helper") fn helper(value: i64) -> i64 uses { process.stdout.write } { value }
@id("app.main") fn main() -> i64 uses { process.stdout.write } {
    let mut index = 0;
    while index < 2 { index = helper(index + 1); 0 }
    index
}
"#;
    let program = parse(effectful, Path::new("loop-write-refused.spx")).unwrap();
    assert!(verify::verify(&program).iter().any(|diagnostic| {
        diagnostic.code == "SPX-T252"
            && diagnostic.message.contains("read-only input effects")
            && diagnostic.message.contains("write it once afterwards")
    }));
}

#[test]
fn hostile_loop_views_cannot_forge_operation_projection_or_ownership() {
    let program = parse(SOURCE, Path::new("loop-command-input-hostile.spx")).unwrap();
    let baseline = hir::resolve(&program).unwrap();
    for mutation in 0..3 {
        let mut hostile = baseline.clone();
        let function = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "loop.read_all")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            unreachable!()
        };
        let hir::ResolvedStatement::While { body, .. } = statements
            .iter_mut()
            .find(|statement| matches!(statement, hir::ResolvedStatement::While { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            unreachable!()
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[3] else {
            unreachable!()
        };
        let hir::ResolvedExprKind::BorrowPlace { operation, place } = &mut value.kind else {
            unreachable!()
        };
        match mutation {
            0 => *operation = hir::DeclarationId::new("forged.string.view"),
            1 => place
                .projections
                .push(hir::PlaceProjection::Field(hir::DeclarationId::new(
                    "forged.field",
                ))),
            2 => value.ownership = hir::OwnershipMode::Own,
            _ => unreachable!(),
        }
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
}
