//! Runtime equivalence for Function Value v1.  The source keeps callback
//! values private while exercising genuine dynamic dispatch in every lane.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.function_values_runtime;
@id("fv.inc") fn inc(value: i64) -> i64 { value + 1 }
@id("fv.dec") fn dec(value: i64) -> i64 { value - 1 }
@id("fv.pick") fn pick(flag: bool) -> fn(i64) -> i64 { if flag { inc } else { dec } }
@id("fv.pass") fn pass(callback: fn(i64) -> i64) -> fn(i64) -> i64 { callback }
@id("fv.apply") fn apply(callback: fn(i64) -> i64, value: i64) -> i64 { callback(value) }
@id("fv.nested") fn nested(flag: bool, value: i64) -> i64 { apply(pass(pick(flag)), value) }
@id("fv.i32") fn identity_i32(value: i32) -> i32 { value }
@id("fv.i32.alt") fn alternate_i32(value: i32) -> i32 { value }
@id("fv.i32.pick") fn pick_i32(flag: bool) -> fn(i32) -> i32 { if flag { identity_i32 } else { alternate_i32 } }
@id("fv.i32.probe") fn probe_i32(flag: bool) -> i64 { let callback = pick_i32(flag); if callback(7i32) == 7i32 { 1 } else { 0 } }
@id("fv.u8") fn identity_u8(value: u8) -> u8 { value }
@id("fv.u8.alt") fn alternate_u8(value: u8) -> u8 { value }
@id("fv.u8.pick") fn pick_u8(flag: bool) -> fn(u8) -> u8 { if flag { identity_u8 } else { alternate_u8 } }
@id("fv.u8.probe") fn probe_u8(flag: bool) -> i64 { let callback = pick_u8(flag); if callback(7u8) == 7u8 { 1 } else { 0 } }
@id("fv.usize") fn identity_usize(value: usize) -> usize { value }
@id("fv.usize.alt") fn alternate_usize(value: usize) -> usize { value }
@id("fv.usize.pick") fn pick_usize(flag: bool) -> fn(usize) -> usize { if flag { identity_usize } else { alternate_usize } }
@id("fv.usize.probe") fn probe_usize(flag: bool) -> i64 { let callback = pick_usize(flag); if callback(7usize) == 7usize { 1 } else { 0 } }
@id("fv.char") fn identity_char(value: char) -> char { value }
@id("fv.char.alt") fn alternate_char(value: char) -> char { value }
@id("fv.char.pick") fn pick_char(flag: bool) -> fn(char) -> char { if flag { identity_char } else { alternate_char } }
@id("fv.char.probe") fn probe_char(flag: bool) -> i64 { let callback = pick_char(flag); if callback('x') == 'x' { 1 } else { 0 } }
@id("fv.f32") fn identity_f32(value: f32) -> f32 { value }
@id("fv.f32.alt") fn alternate_f32(value: f32) -> f32 { value }
@id("fv.f32.pick") fn pick_f32(flag: bool) -> fn(f32) -> f32 { if flag { identity_f32 } else { alternate_f32 } }
@id("fv.f32.probe") fn probe_f32(flag: bool) -> i64 { let callback = pick_f32(flag); if callback(7.0f32) == 7.0f32 { 1 } else { 0 } }
@id("fv.f64") fn identity_f64(value: f64) -> f64 { value }
@id("fv.f64.alt") fn alternate_f64(value: f64) -> f64 { value }
@id("fv.f64.pick") fn pick_f64(flag: bool) -> fn(f64) -> f64 { if flag { identity_f64 } else { alternate_f64 } }
@id("fv.f64.probe") fn probe_f64(flag: bool) -> i64 { let callback = pick_f64(flag); if callback(7.0f64) == 7.0f64 { 1 } else { 0 } }
@id("fv.bool") fn identity_bool(value: bool) -> bool { value }
@id("fv.bool.alt") fn alternate_bool(value: bool) -> bool { value }
@id("fv.bool.pick") fn pick_bool(flag: bool) -> fn(bool) -> bool { if flag { identity_bool } else { alternate_bool } }
@id("fv.bool.probe") fn probe_bool(flag: bool) -> i64 { let callback = pick_bool(flag); if callback(true) == true { 1 } else { 0 } }
@id("app.main") fn main() -> i64 { nested(true, nested(false, 42)) + probe_i32(true) + probe_i32(false) + probe_u8(true) + probe_u8(false) + probe_usize(true) + probe_usize(false) + probe_char(true) + probe_char(false) + probe_f32(true) + probe_f32(false) + probe_f64(true) + probe_f64(false) + probe_bool(true) + probe_bool(false) }
"#;

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn required_or_available(command: &str, requirement: &str) -> bool {
    let available = command_available(command);
    assert!(
        available || std::env::var_os(requirement).is_none(),
        "{requirement} requires {command} for Function Value v1 runtime evidence",
    );
    available
}

fn hex_identity(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn fixture_path(suffix: &str) -> std::path::PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-function-values-{}-{id}.{suffix}",
        std::process::id()
    ))
}

fn checked() -> semaprax::ast::Program {
    let program = parse(SOURCE, Path::new("function-values-runtime.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(
        canonical,
        format::canonical(
            &parse(
                &canonical,
                Path::new("function-values-runtime-canonical.spx")
            )
            .unwrap()
        )
    );
    program
}

#[test]
fn callback_runtime_matches_repeated_interpreter_native_o0_o2_and_wasm() {
    let program = checked();
    let path = fixture_path("spx");
    std::fs::write(&path, format::canonical(&program)).unwrap();
    for _ in 0..2 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert!(
            result.envelope.contains("\"value\":\"56\""),
            "{}",
            result.envelope
        );
    }
    let _ = std::fs::remove_file(&path);

    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    if required_or_available("clang", "SPX_REQUIRE_CLANG") {
        let symbol = format!("spx_decl_{}", hex_identity("app.main"));
        let probe = format!(
            r#"
int main(void) {{
  for (unsigned int run = 0; run != 2; ++run) {{
    struct spx_status_entry entries[UINT32_C(8)]; struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(700) + run, entries, UINT32_C(8), NULL, NULL, NULL)) return 10;
    int64_t value = INT64_C(-777); if ({symbol}(&context, &value) != SPX_STATUS_SUCCESS || value != INT64_C(56)) return 11;
  }} return 0;
}}
"#
        );
        for optimization in ["-O0", "-O2"] {
            let source = fixture_path("c");
            let executable = fixture_path("native");
            std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
            let compiled = Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            assert!(Command::new(&executable).status().unwrap().success());
            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(executable);
        }
    }

    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let mut indirect = 0usize;
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            for operator in body.get_operators_reader().unwrap() {
                if matches!(operator.unwrap(), wasmparser::Operator::CallIndirect { .. }) {
                    indirect += 1;
                }
            }
        }
    }
    assert!(indirect > 0, "Function Value v1 must retain call_indirect");
    if required_or_available("node", "SPX_REQUIRE_NODE") {
        let root = fixture_path("web");
        wasm::build_web(&program, &root).unwrap();
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let output = Command::new("node")
            .arg(script)
            .arg(&root)
            .arg("56")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "56");
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn callback_argument_failure_precedes_target_contract_failures_and_public_abi_stays_closed() {
    let declarations = r#"
@id("fv.guarded") fn guarded(value: i64) -> i64 requires value == 0 { value }
@id("fv.post") fn post(value: i64) -> i64 ensures result == 0 { value }
"#;
    let base = SOURCE.replace(
        "@id(\"app.main\")",
        &(declarations.to_owned() + "@id(\"app.main\")"),
    );
    let failures = [
        base.replace(
            "nested(true, nested(false, 42))",
            "let callback = pick(true); callback(1 / 0)",
        ),
        base.replace(
            "nested(true, nested(false, 42))",
            "let callback = guarded; callback(1)",
        ),
        base.replace(
            "nested(true, nested(false, 42))",
            "let callback = post; callback(1)",
        ),
    ];
    let expected = [
        ("semaprax.arithmetic.v1", 4),
        ("semaprax.contract.v1", 1),
        ("semaprax.contract.v1", 2),
    ];
    for (source, (domain, code)) in failures.into_iter().zip(expected) {
        let program = parse(&source, Path::new("function-values-failure.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let path = fixture_path("spx");
        std::fs::write(&path, format::canonical(&program)).unwrap();
        for _ in 0..3 {
            let result =
                interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
                    .unwrap();
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(envelope["payload"]["outcome"]["kind"], "failed");
            assert_eq!(
                envelope["payload"]["outcome"]["status"]["domain_id"],
                domain
            );
            assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
        }
        let _ = std::fs::remove_file(path);
        failure_backends(&program, domain, code);
    }
    assert!(wasm::emit_module_with_scalar_exports(&checked(), &["fv.apply".to_owned()]).is_err());
}

fn failure_backends(program: &semaprax::ast::Program, domain: &str, code: u32) {
    let generated = codegen::emit_c(program).unwrap();
    if required_or_available("clang", "SPX_REQUIRE_CLANG") {
        let probe = format!(
            r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(812), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
    for (unsigned int run=0; run<3; ++run) {{
        int64_t value=INT64_C(0x2525252525252525);
        uint32_t before=context.status_arena.length;
        spx_status_token status=spx_decl_6170702e6d61696e(&context, &value);
        if(status==SPX_STATUS_SUCCESS || value!=INT64_C(0x2525252525252525)) return 2;
        if(spx_status_resolve(&context,status)==NULL) return 3;
        if(strcmp(spx_status_resolve(&context,status)->domain_id,"{domain}")!=0 || spx_status_resolve(&context,status)->code!=UINT32_C({code})) return 4;
        if(context.status_arena.length != before+UINT32_C(1)) return 5;
    }} return 0;
}}
"#
        );
        for optimization in ["-O0", "-O2"] {
            let source = fixture_path("c");
            let executable = fixture_path("native");
            std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
            let output = Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let output = Command::new(&executable).output().unwrap();
            assert!(
                output.status.success(),
                "{domain}/{code} {optimization}: {output:?}"
            );
            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(executable);
        }
    }
    if required_or_available("node", "SPX_REQUIRE_NODE") {
        let root = fixture_path("web");
        wasm::build_web(program, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        std::fs::write(
            root.join("probe.mjs"),
            format!(
                r#"
import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const {{instance}}=await instantiateBytes(await readFile('./app.wasm'));
for(let i=0;i<3;i++){{
 let failed=false;
 try{{instance.exports.semaprax_main();}}catch(error){{
  const status=semanticStatus(error);
  if(status===null || status.domain_id!=={domain:?} || status.code!=={code})throw error;
  failed=true;
 }}
 if(!failed)throw Error('missing expected callback failure');
}}
"#
            ),
        )
        .unwrap();
        let output = Command::new("node")
            .arg("probe.mjs")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{domain}/{code}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
