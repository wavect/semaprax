//! Closed source, graph, ownership and backend gate for literal formatting.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"
module test.checked_literal_format;

@id("format.success")
fn success() -> i64 {
    let mut n = 7;
    let first = "left";
    let second = "right";
    let rendered = string_format("{}{{}}:{}:{}{}", n, { n = 9; true }, first, second);
    if rendered == "7{}:true:leftright" && n == 9 { 7 } else { 0 }
}

@id("format.failure")
fn failure() -> i64 {
    let first = "a";
    let second = "b";
    let rendered = string_format("{}{}", first, second);
    string_len(rendered)
}

@id("app.main")
fn main() -> i64 { success() }
"#;

#[test]
fn checked_literal_format_round_trips_and_projects_raw_template() {
    let program = parse(SOURCE, Path::new("literal-format.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let again = parse(&canonical, Path::new("literal-format-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&again), canonical);
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"kind\":\"literal_format\""), "{graph}");
    assert!(graph.contains("\"template\":\"{}{{}}:{}:{}{}\""), "{graph}");
    graph::verify_json(&program, &graph).unwrap();
    let c = codegen::emit_c(&program).unwrap();
    assert!(c.contains("spx_format_join_v1"));
    assert!(c.contains("spx_fmt_"));
}

#[test]
fn checked_literal_format_refuses_nonliteral_grammar_count_and_type() {
    for (body, code, phrase) in [
        (
            "let template = \"{}\"; string_len(string_format(template, 1))",
            "SPX-T204",
            "compile-time string literal",
        ),
        (
            "string_len(string_format(\"{name}\", 1))",
            "SPX-T204",
            "permits only",
        ),
        (
            "string_len(string_format(\"{}{}\", 1))",
            "SPX-T204",
            "fields",
        ),
        (
            "string_len(string_format(\"{}\", 1.0))",
            "SPX-T205",
            "must be i64",
        ),
    ] {
        let source =
            format!("module test.format_refusal; @id(\"app.main\") fn main() -> i64 {{ {body} }}");
        let program = parse(&source, Path::new("literal-format-refusal.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.message.contains(phrase)),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn checked_literal_format_interpreter_and_native_preserve_staged_scalar() {
    let program = parse(SOURCE, Path::new("literal-format-execution.spx")).unwrap();
    let mut fixture = Fixture::new(SOURCE);
    let interpreted = interpreter::interpret(
        &fixture.source,
        "format.success",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    assert!(
        interpreted.envelope.contains("\"value\":\"7\""),
        "{}",
        interpreted.envelope
    );
    assert!(
        Command::new("clang").arg("--version").output().is_ok(),
        "checked literal format native gate requires clang"
    );
    let executable = fixture
        .root
        .join(format!("format-native{}", std::env::consts::EXE_SUFFIX));
    codegen::build(&program, &executable).unwrap();
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
    std::fs::remove_file(executable).unwrap();
    fixture.cleanup();
}

#[test]
fn checked_literal_format_wasm_repeats_success_and_after_commit_failure() {
    assert!(
        Command::new("node").arg("--version").output().is_ok(),
        "checked literal format standalone Wasm gate requires Node.js"
    );
    let program = parse(SOURCE, Path::new("literal-format-wasm.spx")).unwrap();
    let artifact = emit_module(
        &program,
        &["format.success".into(), "format.failure".into()],
        InternalStringOptions {
            max_cumulative_bytes: 2,
            ..InternalStringOptions::default()
        },
    )
    .unwrap();
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    let script = fixture.write("probe.mjs", r#"import {readFileSync} from 'node:fs';
import {instantiate} from './program.mjs';
const api = await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for (let i=0;i<8;i++) {
  const result=api.call('format.failure');
  if(result.kind!=='failure'||result.domain!=='semaprax.string-format.v1'||result.code!==1) throw Error(JSON.stringify(result));
}
"#);
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.cleanup();
}

#[test]
fn checked_literal_format_native_join_failure_is_sticky_after_commit() {
    assert!(
        Command::new("clang").arg("--version").output().is_ok(),
        "checked literal format native failure gate requires clang"
    );
    let source = r#"module test.format_native_failure;
@id("app.main") fn main() -> i64 {
    let rendered = string_format("{}", string_from_i64(1));
    string_len(rendered)
}"#;
    let program = parse(source, Path::new("format-native-failure.spx")).unwrap();
    let emitted = codegen::emit_c(&program).unwrap();
    let marker = "#define SPX_FORMAT_STATUS_DOMAIN_V1";
    let (before, after) = emitted.split_once(marker).expect("format worker runtime");
    let old = "struct spx_string_v10 *value = (struct spx_string_v10 *)malloc(";
    assert!(after.matches(old).count() >= 2);
    let after = after.replacen(
        old,
        "struct spx_string_v10 *value = (struct spx_string_v10 *)spx_test_malloc(",
        2,
    );
    let injected = format!("{before}static unsigned spx_test_malloc_count;\nstatic void *spx_test_malloc(size_t bytes) {{ if (++spx_test_malloc_count == 3) return NULL; return malloc(bytes); }}\n{marker}{after}");
    let mut fixture = Fixture::new(source);
    let c = fixture.write("format-failure.c", injected);
    let binary = fixture
        .root
        .join(format!("format-failure{}", std::env::consts::EXE_SUFFIX));
    let compile = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(c)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(&binary).output().unwrap();
    assert_eq!(output.status.code(), Some(73), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("semaprax.string-format.v1/1"),
        "{output:?}"
    );
    std::fs::remove_file(binary).unwrap();
    fixture.cleanup();
}

#[test]
fn checked_literal_format_aggregate_wasm_host_failure_reenters() {
    assert!(
        Command::new("node").arg("--version").output().is_ok(),
        "checked literal format aggregate Wasm gate requires Node.js"
    );
    let source = r#"module test.format_wasm_failure;
@id("app.main") fn main() -> i64 {
    let rendered = string_format("{}{}", 1, 2);
    string_len(rendered)
}"#;
    let program = parse(source, Path::new("format-wasm-failure.spx")).unwrap();
    let mut fixture = Fixture::new(source);
    let web = fixture.root.join("web");
    wasm::build_web(&program, &web).unwrap();
    std::fs::write(web.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes,semanticStatus} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const limited=await instantiateBytes(bytes,{maxOwnedByteEntries:1});
for(let index=0;index<8;index++) {
  let status=null;
  try { limited.instance.exports.semaprax_main(); } catch(error) { status=semanticStatus(error); }
  if(status?.domain_id!=='semaprax.string-format.v1'||status.code!==1) throw Error(JSON.stringify(status));
}
const ordinary=await instantiateBytes(bytes,{maxOwnedByteEntries:8});
if(ordinary.instance.exports.semaprax_main()!==2n) throw Error('format success result');
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&web)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(web).unwrap();
    let success_program = parse(SOURCE, Path::new("format-wasm-success.spx")).unwrap();
    let success_web = fixture.root.join("success-web");
    wasm::build_web(&success_program, &success_web).unwrap();
    std::fs::write(
        success_web.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
if(instance.exports.semaprax_main()!==7n) throw Error('aggregate staged-scalar success');
"#,
    )
    .unwrap();
    let success_output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&success_web)
        .output()
        .unwrap();
    assert!(
        success_output.status.success(),
        "{}",
        String::from_utf8_lossy(&success_output.stderr)
    );
    std::fs::remove_dir_all(success_web).unwrap();
    fixture.cleanup();
}
