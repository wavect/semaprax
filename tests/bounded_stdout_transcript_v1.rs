use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hosted_interpreter::execute_stdout_transcript;
use semaprax::interpreter::ResolvedEvaluationOutcome;
use semaprax::{codegen, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.stdout_transcript;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64 uses { process.stdout.write } {
    let data = [65u8, 0u8, 66u8];
    let view = array_as_slice(data);
    let written = stdout_write(view);
    if written == 3usize { 7 } else { 0 }
}
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("stdout-transcript.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

#[test]
fn exact_effectful_write_seals_bytes_only_on_success_and_selects_graph_v18() {
    let program = resolved(SOURCE);
    let result = execute_stdout_transcript(&program, "app.main", 10_000).unwrap();
    assert_eq!(
        result.evaluation.outcome,
        ResolvedEvaluationOutcome::ReturnedI64(7)
    );
    assert_eq!(result.transcript, [65, 0, 66]);

    let ast = parse(SOURCE, Path::new("stdout-transcript-graph.spx")).unwrap();
    let json = graph::to_json(&ast).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v18\""));
    assert!(json.contains("\"operation\":\"core.host.stdout-write\""));
    assert!(json.contains("\"publication\":\"terminal-success-only\""));
}

#[test]
fn missing_authority_and_multiple_or_looped_writes_fail_closed() {
    let missing_uses = SOURCE.replace(
        "fn main() -> i64 uses { process.stdout.write }",
        "fn main() -> i64",
    );
    let ast = parse(&missing_uses, Path::new("missing-uses.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-E102"));

    let two = SOURCE.replace(
        "let written = stdout_write(view);",
        "let first = stdout_write(view);\n    let written = stdout_write(view);",
    );
    let ast = parse(&two, Path::new("two-writes.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-T269"));

    let looped = SOURCE.replace(
        "let written = stdout_write(view);",
        "let mut written = 0usize;\n    while written == 0usize { written = stdout_write(view); written == 0usize }",
    );
    let ast = parse(&looped, Path::new("looped-write.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-T269"));
}

#[test]
fn staged_bytes_are_discarded_on_terminal_language_failure() {
    let source = SOURCE.replace(
        "if written == 3usize { 7 } else { 0 }",
        "if written == 3usize { 1 / 0 } else { 0 }",
    );
    let program = resolved(&source);
    let result = execute_stdout_transcript(&program, "app.main", 10_000).unwrap();
    assert!(matches!(
        result.evaluation.outcome,
        ResolvedEvaluationOutcome::LanguageFailure(_)
    ));
    assert!(result.transcript.is_empty());
}

fn assert_hosted_and_native_authority_rejection(program: &hir::ResolvedProgram) {
    let hosted = execute_stdout_transcript(program, "app.main", 10_000).unwrap_err();
    assert_eq!(hosted.first().map(|item| item.code), Some("SPX-T269"));
    let native = codegen::emit_hir_c_with_stdout_transcript(program).unwrap_err();
    assert_eq!(native.code, "SPX-T269");
    assert!(native.message.contains("authority mismatch"));
}

fn source_with_unused_inventory(declaration: &str) -> String {
    SOURCE.replace(
        "@id(\"app.main\")",
        &format!("{declaration}\n\n@id(\"app.main\")"),
    )
}

#[test]
fn standalone_hosted_and_native_profiles_reject_widened_authority_inventory() {
    let mut widened_permit = resolved(SOURCE);
    widened_permit.permits.push("process.network".to_owned());
    assert_hosted_and_native_authority_rejection(&widened_permit);

    let mut widened_effect = resolved(SOURCE);
    widened_effect.permits.push("process.network".to_owned());
    widened_effect.functions[0]
        .effects
        .push("process.network".to_owned());
    assert_hosted_and_native_authority_rejection(&widened_effect);

    let interface_source = SOURCE.replace(
        "@id(\"app.main\")",
        r#"@id("host.interface")
interface Host permits {} {
    @id("host.echo")
    import rust fn echo(value: i64) -> i64
        effects {}
        failure status "host.echo.v1";
}

@id("app.main")"#,
    );
    let ast = parse(&interface_source, Path::new("stdout-profile-interface.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let with_interface = hir::resolve(&ast).unwrap();
    assert_hosted_and_native_authority_rejection(&with_interface);

    let generic = resolved(&source_with_unused_inventory(
        r#"@id("unused.identity")
fn identity<T>(value: T) -> T { value }"#,
    ));
    assert_hosted_and_native_authority_rejection(&generic);

    let authored_type_inventories = [
        r#"@id("unused.record")
record UnusedRecord { @id("unused.record.value") value: i64, }"#,
        r#"@id("unused.resource")
resource UnusedResource { @id("unused.resource.drop") drop trivial; }"#,
        r#"@id("unused.variant")
variant UnusedVariant { @id("unused.variant.none") None, }"#,
        r#"@id("unused.class")
class UnusedClass { @id("unused.class.value") value: i64, }"#,
    ];
    for declaration in authored_type_inventories {
        let program = resolved(&source_with_unused_inventory(declaration));
        assert_hosted_and_native_authority_rejection(&program);
    }
}

#[test]
fn reserved_name_and_identity_cannot_be_shadowed_by_authored_declarations() {
    let function_name = r#"
module test.stdout_name_alias;
@id("evil.function")
fn stdout_write(value: borrow Slice<u8>) -> usize { 0usize }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(function_name, Path::new("stdout-name-alias.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-S113"));

    let declaration_aliases = [
        r#"module t; @id("core.host.stdout-write") record R { @id("r.x") x: i64, } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("r") record R { @id("core.host.stdout-write") x: i64, } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("v") variant V { @id("core.host.stdout-write") A, } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("v") variant V { @id("v.a") A { @id("core.host.stdout-write") x: i64, }, } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("core.host.stdout-write") interface Host permits {} {} @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("r") resource R { @id("core.host.stdout-write") drop trivial; } @id("app.main") fn main() -> i64 { 0 }"#,
        r#"module t; @id("c") class C { @id("c.x") x: i64, @id("core.host.stdout-write") fn value(self: C) -> i64 { self.x } } @id("app.main") fn main() -> i64 { 0 }"#,
    ];
    for (index, source) in declaration_aliases.into_iter().enumerate() {
        let ast = parse(
            source,
            Path::new(&format!("stdout-declaration-alias-{index}.spx")),
        )
        .unwrap();
        assert!(
            verify::verify(&ast)
                .iter()
                .any(|item| item.code == "SPX-S113"),
            "reserved declaration ID case {index} was admitted"
        );
    }

    let function_id = r#"
module test.stdout_id_alias;
@id("core.host.stdout-write")
fn impostor(value: borrow Slice<u8>) -> usize { 0usize }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(function_id, Path::new("stdout-id-alias.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-S113"));

    let import_alias = r#"
module test.stdout_import_alias;
interface Host permits {} {
    @id("core.host.stdout-write")
    import rust fn host_alias(value: i64) -> i64
        effects {}
        failure status "host.stdout.v1";
    @id("host.stdout-name")
    import rust fn stdout_write(value: i64) -> i64
        effects {}
        failure status "host.stdout-name.v1";
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(import_alias, Path::new("stdout-import-alias.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|item| item.code == "SPX-S113"));
}

#[test]
fn hostile_hir_cannot_alias_compiler_owned_host_identity() {
    let mut function_alias = resolved(SOURCE);
    function_alias.functions[0].id = hir::DeclarationId::new("core.host.stdout-write");
    let diagnostic = hir::validate(&function_alias).unwrap_err();
    assert!(diagnostic
        .message
        .contains("aliases a compiler-owned host I/O operation"));

    let import_source = r#"
module test.stdout_hostile_import;
@id("host.interface")
interface Host permits {} {
    @id("host.echo")
    import rust fn echo(value: i64) -> i64
        effects {}
        failure status "host.echo.v1";
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = parse(import_source, Path::new("stdout-hostile-import.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let mut import_alias = hir::resolve(&ast).unwrap();
    import_alias.interfaces[0].imports[0].id = hir::DeclarationId::new("core.host.stdout-write");
    let diagnostic = hir::validate(&import_alias).unwrap_err();
    assert!(diagnostic
        .message
        .contains("aliases a compiler-owned host I/O operation"));
}

#[test]
fn native_o0_o2_publish_exact_bytes_and_discard_after_failure() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let success = codegen::emit_hir_c_with_stdout_transcript(&resolved(SOURCE)).unwrap();
    assert!(success.contains("spx_stdout_transcript_run_v1"));
    assert!(!success.contains("printf(\"%lld\\n\""));
    let failed_source = SOURCE.replace(
        "if written == 3usize { 7 } else { 0 }",
        "if written == 3usize { 1 / 0 } else { 0 }",
    );
    let failed = codegen::emit_hir_c_with_stdout_transcript(&resolved(&failed_source)).unwrap();
    let success_probe = r#"
int main(void) {
    struct spx_stdout_transcript_result_v1 observed;
    memset(&observed, 0xa5, sizeof(observed));
    if (!spx_stdout_transcript_run_v1(&observed)) return 10;
    if (observed.value != INT64_C(7) || observed.transcript_length != UINT64_C(3)) return 11;
    if (observed.transcript[0] != UINT8_C(65) || observed.transcript[1] != UINT8_C(0) || observed.transcript[2] != UINT8_C(66)) return 12;
    return 0;
}
"#;
    let failure_probe = r#"
int main(void) {
    struct spx_stdout_transcript_result_v1 observed;
    memset(&observed, 0xa5, sizeof(observed));
    if (spx_stdout_transcript_run_v1(&observed)) return 20;
    if (observed.transcript_length != UINT64_C(0)) return 21;
    for (size_t i = 0; i < sizeof(observed.transcript); ++i) if (observed.transcript[i] != UINT8_C(0)) return 22;
    return 0;
}
"#;
    for optimization in ["-O0", "-O2"] {
        for (kind, generated, probe) in [
            ("success", success.as_str(), success_probe),
            ("failure", failed.as_str(), failure_probe),
        ] {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let stem = format!("semaprax-stdout-{kind}-{}-{id}", std::process::id());
            let source = std::env::temp_dir().join(format!("{stem}.c"));
            let executable =
                std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
            let compiled = Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let executed = Command::new(&executable).output().unwrap();
            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(executable);
            assert!(executed.status.success(), "native transcript probe failed");
        }
    }
}
