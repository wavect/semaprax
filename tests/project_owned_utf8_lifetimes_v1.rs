//! Authored physical V10 evidence. No tool absence is treated as success.
//! The descriptor gate runs before generation: argument blocks are already
//! admitted inside a direct String-returning call; outer String blocks are not.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, ProjectNpmBuild, PublicApiSubject,
    PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const SELECTED: &[&str] = &[
    "s.branch",
    "s.clone",
    "s.late",
    "s.local-fail",
    "s.loop",
    "s.mixed",
    "s.outer",
    "s.pressure",
    "s.simple",
];

fn source() -> String {
    let mut source = r#"module test.string_lifetimes;
@id("s.sink") fn sink(value: string) -> string { "done" }
@id("s.integer") fn integer(value: i64) -> string { "done" }
@id("s.two") fn two(value: string, number: i64) -> string { "done" }
@id("s.simple") fn simple() -> string { sink("argument") }
@id("s.clone") fn clone_value() -> string { sink({ let a = "alpha"; let b = a; b }) }
@id("s.branch") fn branch(flag: bool) -> string { sink(if flag { "left" } else { "right" }) }
@id("s.loop") fn repeated(count: i64) -> string {
    integer({ let retained = "across-loop"; let mut i = 0; while i < count {
        i = i + 1; 0
    } 0 })
}
@id("s.late") fn late(zero: i64) -> string { two("argument", 1 / zero) }
@id("s.callee") fn callee(value: string, zero: i64) -> string { two("inner", 1 / zero) }
@id("s.outer") fn outer(zero: i64) -> string { callee("outer", zero) }
@id("s.local-fail") fn local_failure(zero: i64) -> string {
    integer({ let kept = "kept"; let divided = 1 / zero; 0 })
}
@id("s.pressure") fn pressure() -> string { sink({
"#
    .to_owned();
    for index in 0..17 {
        source.push_str(&format!("let value{index} = \"payload\";\n"));
    }
    source.push_str("value16 }) }\n@id(\"s.mixed\") fn mixed(input: borrow Slice<u8>) -> string { integer({ let text = \"mixed\";\n");
    for index in 0..16 {
        source.push_str(&format!("let bytes{index} = bytes_copy(input);\n"));
    }
    source.push_str("0 }) }\n@id(\"s.main\") fn main() -> i64 { 0 }\n");
    source
}

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    }
}

fn prepare() -> ProjectNpmBuild {
    let program =
        semaprax::hir::resolve(&semaprax::check(&source(), "string-lifetimes.spx").unwrap())
            .unwrap();
    let descriptor = derive_public_api_descriptor(
        &program,
        &SELECTED
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>(),
        subject(),
    )
    .unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "string-lifetimes",
        "1.0.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    build.verify().unwrap();
    ProjectNpmBuild::inspect_envelope(build.envelope(), build.max_bytes()).unwrap();
    build
}

fn write_package(build: &ProjectNpmBuild, root: &Path) {
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    for row in envelope["artifacts"].as_array().unwrap() {
        let hex = row["hex"].as_str().unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        fs::write(root.join(row["path"].as_str().unwrap()), bytes).unwrap();
    }
}

fn temporary(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-string-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}

#[test]
fn admitted_v10_strings_settle_with_exact_clone_call_loop_and_failure_counts() {
    let build = prepare();
    let root = temporary("arena");
    write_package(&build, &root);
    // Test-only access to the exact generated private arena, after package
    // replay. No additional export is added to any production artifact.
    let runtime = fs::read_to_string(root.join("semaprax.js")).unwrap();
    assert!(!runtime.contains("entries.size>=16||"));
    fs::write(
        root.join("arena-probe.mjs"),
        format!("{runtime}\nexport {{createArena as probeArena}};\n"),
    )
    .unwrap();
    fs::write(
        root.join("probe.mjs"),
        include_str!("project_owned_utf8_lifetimes_v1/probe.mjs"),
    )
    .unwrap();
    let result = Command::new("node")
        .arg(root.join("probe.mjs"))
        .output()
        .expect("Node is required for V10 lifetime evidence");
    assert!(
        result.status.success(),
        "stdout={} stderr={} fixture={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        root.display()
    );
    for name in [
        "app.wasm",
        "semaprax.js",
        "semaprax.bindings.js",
        "semaprax.bindings.d.ts",
        "semaprax.api.json",
        "package.json",
        "arena-probe.mjs",
        "probe.mjs",
    ] {
        fs::remove_file(root.join(name)).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn closed_descriptor_shape_and_value_string_parameter_modes_are_not_widened() {
    for (body, result) in [
        ("{ let hidden = \"x\"; 7 }", "i64"),
        ("{ let hidden = \"x\"; \"done\" }", "string"),
    ] {
        let source = format!("module rejected.string; @id(\"s.selected\") fn selected() -> {result} {body} @id(\"s.main\") fn main() -> i64 {{0}}");
        let program =
            semaprax::hir::resolve(&semaprax::check(&source, "rejected-string.spx").unwrap())
                .unwrap();
        let error = derive_public_api_descriptor(&program, &["s.selected".to_owned()], subject())
            .unwrap_err();
        assert_eq!(error.code, "SPX-J113");
    }
    let source = "module rejected.mode; @id(\"s.borrow\") fn borrowed(value: borrow string) -> string {\"done\"} @id(\"s.main\") fn main()->i64 {0}";
    assert!(semaprax::check(source, "string-mode.spx")
        .unwrap_err()
        .iter()
        .any(|error| error.code == "SPX-O002"));
}

#[test]
fn native_success_values_match_the_admitted_v10_lifetime_corpus_at_o0_and_o2() {
    // This is value evidence, NOT native failure-path heap settlement evidence.
    let program = semaprax::check(&source(), "string-native.spx").unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let symbol = |id: &str| {
        format!(
            "spx_decl_{}",
            id.bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    };
    let mut probe = String::from("\nint main(void) { struct spx_status_entry entries[16]; struct spx_context ctx={0}; if(!spx_context_init(&ctx,91,entries,16,NULL,NULL,NULL))return 1; char *result=NULL;\n");
    for id in ["s.simple", "s.clone", "s.pressure"] {
        probe.push_str(&format!("if({}(&ctx,&result)!=SPX_STATUS_SUCCESS)return 2; if(strcmp(result,\"done\")!=0)return 3; spx_string_drop(result); result=NULL;\n", symbol(id)));
    }
    probe.push_str("return 0; }\n");
    let root = temporary("native");
    fs::write(root.join("probe.c"), format!("{generated}{probe}")).unwrap();
    let executable = root.join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    for optimization in ["-O0", "-O2"] {
        let built = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(root.join("probe.c"))
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for V10 native value evidence");
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
    }
    fs::remove_file(root.join("probe.c")).unwrap();
    fs::remove_file(executable).unwrap();
    fs::remove_dir(root).unwrap();
}
