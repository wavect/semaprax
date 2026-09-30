use crate::{codegen, format, graph, hir, interpreter, parse, verify, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const BODY: &str = "let moved = endpoint; let active = step(moved); close(active)";

fn source(body: &str) -> String {
    format!(
        r#"module endpoint.fixture;
@id("endpoint.step") fn step(value: own Bytes) -> Bytes {{ value }}
@id("endpoint.close") fn close(value: own Bytes) -> i64 {{
    if byte_len(bytes_as_slice(value)) == 2usize {{ 42 }} else {{ 1 }}
}}
@id("endpoint.cancel") fn cancel(value: own Bytes) -> i64 {{ 42 }}
@id("endpoint.escape") fn escape(value: own Bytes) -> Bytes {{ value }}
@id("endpoint.protocol") session protocol "endpoint-v1" {{
    states {{ Ready, Active, Closed }}
    initial Ready;
    endpoint Bytes;
    terminal Closed cleanup {{}}
    on Ready step: send Unit consumes resource via "endpoint.step" -> Active;
    on Ready cancel: cancel Unit consumes resource via "endpoint.cancel" -> Closed;
    on Active close: send Unit consumes resource via "endpoint.close" -> Closed;
    on Active cancel: cancel Unit consumes resource via "endpoint.cancel" -> Closed;
}}
@id("endpoint.workflow") fn workflow(endpoint: own Bytes) -> i64
    follows session protocol "endpoint.protocol"
{{ {body} }}
@id("app.main") fn main() -> i64 {{
    let data = [4u8, 2u8];
    workflow(bytes_copy(array_as_slice(data)))
}}
"#
    )
}

fn parsed(body: &str) -> crate::ast::Program {
    parse(&source(body), "endpoint.spx").unwrap()
}

fn has_code(text: &str, expected: &str) {
    let program = parse(text, "endpoint.spx").unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().any(|d| d.code == expected),
        "expected {expected}: {diagnostics:?}"
    );
    assert!(hir::resolve(&program).is_err());
    assert!(codegen::emit_c(&program).is_err());
    assert!(wasm::emit_module(&program).is_err());
}

#[test]
fn source_endpoint_moves_closes_and_roundtrips_through_hir_graph_and_cache() {
    let program = parsed(BODY);
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "endpoint.spx").unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::validate(&hir::resolve(&reparsed).unwrap()).unwrap();
    let encoded = crate::cache_codec::encode(&program).unwrap();
    let decoded: crate::ast::Program = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(decoded, program);
    let facts: serde_json::Value =
        serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap();
    assert_eq!(facts["schema"], "semaprax.graph.v51");
    assert_eq!(
        facts["session_protocols"]["declarations"][0]["endpoint"]["profile"],
        "affine-bytes.v1"
    );
    assert_eq!(
        facts["session_protocols"]["declarations"][0]["authority"],
        "none"
    );
    assert_eq!(
        codegen::emit_c(&program).unwrap(),
        codegen::emit_c(&program).unwrap()
    );
    assert_eq!(
        wasm::emit_module(&program).unwrap(),
        wasm::emit_module(&program).unwrap()
    );
}

#[test]
fn refuses_source_use_after_close_stale_alias_duplicate_and_replacement() {
    for body in [
        "let active = step(endpoint); let result = close(active); let stale = bytes_as_slice(active); result",
        "let moved = endpoint; let active = step(endpoint); close(active)",
        "let moved = endpoint; let duplicate = moved; let active = step(moved); close(active)",
        "let active = step(endpoint); let data = [1u8]; close(bytes_copy(array_as_slice(data)))",
        "let escaped = escape(endpoint); let active = step(escaped); close(active)",
        "let mut moved = endpoint; let active = step(moved); close(active)",
        "let delayed = fn() -> usize { byte_len(bytes_as_slice(endpoint)) }; let active = step(endpoint); close(active)",
    ] { has_code(&source(body), "SPX-K111"); }
}

#[test]
fn refuses_illegal_state_unclosed_endpoint_and_forged_declaration_profile() {
    has_code(&source("close(endpoint)"), "SPX-K108");
    has_code(&source("let active = step(endpoint); 0"), "SPX-K108");
    for (from, to) in [
        ("endpoint Bytes;", "endpoint i64;"),
        ("cleanup {}", "cleanup { release }"),
        ("consumes resource via", "via"),
        ("step(value: own Bytes)", "step(value: borrow Bytes)"),
    ] {
        has_code(&source(BODY).replace(from, to), "SPX-K110");
    }
}

#[test]
fn terminal_branches_and_immediate_cancel_consume_the_actual_owner() {
    for body in [
        "cancel(endpoint)",
        "let active = step(endpoint); if true { close(active) } else { cancel(active) }",
    ] {
        let program = parsed(body);
        let diagnostics = verify::verify(&program);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let resolved = hir::resolve(&program).unwrap();
        let execution =
            interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10_000).unwrap();
        assert_eq!(
            execution.outcome,
            interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
    }
}

#[test]
fn endpoint_carrier_executes_and_settles_on_interpreter_native_and_wasm() {
    let program = parsed(BODY);
    let resolved = hir::resolve(&program).unwrap();
    for _ in 0..4 {
        let execution =
            interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10_000).unwrap();
        assert_eq!(
            execution.outcome,
            interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        );
    }
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("semaprax-endpoint-{}-{serial}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let c = root.join("endpoint.c");
    std::fs::write(&c, codegen::emit_c(&program).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "endpoint{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compilation = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("endpoint gate requires clang");
        assert!(
            compilation.status.success(),
            "{}",
            String::from_utf8_lossy(&compilation.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert!(run.status.success(), "{run:?}");
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "42");
    }
    let web = root.join("web");
    wasm::build_web(&program, &web).unwrap();
    std::fs::write(web.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        web.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'), {maxOwnedByteEntries:1});
for(let i=0;i<8;i++) if(instance.exports.semaprax_main()!==42n) throw Error('endpoint result');
"#,
    )
    .unwrap();
    let run = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&web)
        .output()
        .expect("endpoint gate requires node");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn endpoint_declarations_cannot_mint_missing_effect_authority() {
    has_code(
        &source(BODY).replace(
            "on Active close: send Unit consumes",
            "on Active close: send Unit requires capability clock.read consumes",
        ),
        "SPX-K105",
    );
}

#[test]
fn terminal_contract_failure_settles_before_wasm_reentry() {
    let text = source(BODY).replace(
        "fn close(value: own Bytes) -> i64 {",
        "fn close(value: own Bytes) -> i64 ensures false {",
    );
    let program = parse(&text, "endpoint-failure.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    assert!(matches!(
        interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10_000)
            .unwrap()
            .outcome,
        interpreter::ResolvedEvaluationOutcome::LanguageFailure(_)
    ));
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-endpoint-failure-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("failure.c"), codegen::emit_c(&program).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "failure{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compilation = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(root.join("failure.c"))
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("endpoint gate requires clang");
        assert!(
            compilation.status.success(),
            "{}",
            String::from_utf8_lossy(&compilation.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert!(!run.status.success());
        assert!(run.stdout.is_empty());
        assert!(String::from_utf8_lossy(&run.stderr).contains("contract failure"));
    }
    let web = root.join("web");
    wasm::build_web(&program, &web).unwrap();
    std::fs::write(web.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(
        web.join("probe.mjs"),
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'), {maxOwnedByteEntries:1});
for(let i=0;i<8;i++) {
    let failed=false;
    try { instance.exports.semaprax_main(); }
    catch(error) { if(error.message!=='SEMAPRAX contract failure') throw error; failed=true; }
    if(!failed) throw Error('missing terminal failure');
}
"#,
    )
    .unwrap();
    let run = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&web)
        .output()
        .expect("endpoint gate requires node");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_source_example_is_canonical_and_executes() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/session-endpoint.spx"
    ));
    let program = parse(source, "session-endpoint.spx").unwrap();
    assert_eq!(format::canonical(&program), source);
    let resolved = hir::resolve(&program).unwrap();
    assert_eq!(
        interpreter::evaluate_resolved_zero_arg_i64(&resolved, "app.main", 10_000)
            .unwrap()
            .outcome,
        interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    );
}
