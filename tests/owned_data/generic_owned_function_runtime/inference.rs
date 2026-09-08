//! Omitted bounded generic vectors keep the explicit owned-call runtime proof.
use super::*;
use std::fmt::Write as _;

const SCALARS: [(&str, &str); 8] = [
    ("i64", "7"),
    ("i32", "7i32"),
    ("u8", "7u8"),
    ("usize", "7usize"),
    ("char", "'x'"),
    ("f32", "1.5f32"),
    ("f64", "1.5"),
    ("bool", "true"),
];

fn source(inferred: bool, allowed: bool) -> String {
    let arguments = |ty: &str| {
        if inferred {
            String::new()
        } else {
            format!("<{ty}>")
        }
    };
    let mut source = String::from(
        r#"
module test.generic_owned_inference_runtime;
@id("inference.pair") record Pair<T,U>{
 @id("inference.pair.payload") payload:T,
 @id("inference.pair.marker") marker:U,
}
@id("inference.relay") fn relay<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T>{value}
@id("inference.reject") fn reject<T>(value:own Pair<Bytes,T>,allowed:bool)->Pair<Bytes,T> requires allowed {value}
"#,
    );
    let mut calls = Vec::new();
    for (ty, literal) in SCALARS {
        let arguments = arguments(ty);
        writeln!(
            source,
            r#"
@id("inference.make.{ty}") fn make_{ty}()->Pair<Bytes,{ty}>{{
 let input=[9u8];
 Pair<Bytes,{ty}>{{payload:bytes_copy(array_as_slice(input)),marker:{literal}}}
}}
@id("inference.consume.{ty}") fn consume_{ty}(value:own Pair<Bytes,{ty}>)->i64{{
 match own value{{Pair{{payload,marker}}=>if byte_len(bytes_as_slice(payload))==1usize&&marker=={literal}{{1}}else{{0}},}}
}}
@id("inference.run.{ty}") fn run_{ty}()->i64{{
 let input = make_{ty}();
 let relayed = relay{arguments}(input);
 let accepted = reject{arguments}(relayed,{allowed});
 consume_{ty}(accepted)
}}
"#
        )
        .unwrap();
        calls.push(format!("run_{ty}()"));
    }
    writeln!(
        source,
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    )
    .unwrap();
    source
}

#[test]
fn inferred_owned_calls_match_explicit_instances_and_settle_on_every_engine() {
    let explicit_text = source(false, true);
    let inferred_text = source(true, true);
    let explicit = semaprax::check(&explicit_text, "inference-explicit.spx").unwrap();
    let inferred = semaprax::check(&inferred_text, "inference-omitted.spx").unwrap();
    let explicit_hir = hir::resolve(&explicit).unwrap();
    let inferred_hir = hir::resolve(&inferred).unwrap();
    hir::validate(&explicit_hir).unwrap();
    hir::validate(&inferred_hir).unwrap();
    assert_eq!(
        inferred_hir.function_instances,
        explicit_hir.function_instances
    );
    assert_eq!(
        inferred_hir
            .function_instances
            .iter()
            .filter(|instance| instance.template.as_str() == "inference.relay")
            .count(),
        SCALARS.len()
    );
    let canonical = semaprax::format::canonical(&inferred);
    let reparsed = semaprax::check(&canonical, "inference-canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let graph = semaprax::graph::to_json(&inferred).unwrap();
    semaprax::graph::verify_json(&inferred, &graph).unwrap();

    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    let success = Expected::Value(SCALARS.len() as i64);
    run_interpreter_source("inferred owned calls", &inferred_text, success);
    if clang {
        run_native(&inferred, success);
    }
    if node {
        run_wasm_source(&inferred, &explicit_text, success);
    }

    let failure_text = source(true, false);
    let failure = semaprax::check(&failure_text, "inference-failure.spx").unwrap();
    let expected = Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure");
    run_interpreter_source("inferred owned failure", &failure_text, expected);
    if clang {
        run_native(&failure, expected);
    }
    if node {
        run_wasm_source(&failure, &source(false, false), expected);
    }
}

fn nested_source(inferred: bool, allowed: bool) -> String {
    let mut text = source(false, allowed);
    text = text.replace("@id(\"inference.relay\")", "@id(\"inference.owner\") fn owner<T,U>(value:own Pair<Bytes,T>,tag:U)->Pair<Bytes,T>{value}\n@id(\"inference.relay\")");
    for (ty, _) in SCALARS {
        let original=format!("let input = make_{ty}();\n let relayed = relay<{ty}>(input);\n let accepted = reject<{ty}>(relayed,{allowed});\n consume_{ty}(accepted)");
        let vector = if inferred {
            String::new()
        } else {
            format!("<{ty},i64>")
        };
        let replacement=format!("consume_{ty}(owner{vector}(reject<{ty}>(relay<{ty}>(if true {{make_{ty}()}} else {{make_fail_{ty}()}}),{allowed}),1+2))");
        assert!(text.contains(&original), "missingrunbody {ty}");
        text = text.replace(&original, &replacement);
        writeln!(text,"@id(\"inference.make_fail.{ty}\") fn make_fail_{ty}()->Pair<Bytes,{ty}>{{reject<{ty}>(make_{ty}(),false)}}").unwrap();
    }
    text
}

#[test]
fn inferred_nested_owned_call_results_materialize_ordered_vectors_and_execute_once() {
    for allowed in [true, false] {
        run_nested_case(
            allowed,
            &nested_source(true, allowed),
            &nested_source(false, allowed),
        );
    }
}

fn forwarding_source(inferred: bool, allowed: bool) -> String {
    let mut text = nested_source(false, allowed);
    text=text.replace("@id(\"inference.relay\")", "@id(\"inference.forward\") fn forward<T>(value:own Pair<Bytes,T>)->Pair<Bytes,T>{relay<T>(relay<T>(value))}\n@id(\"inference.relay\")");
    for (ty, _) in SCALARS {
        text = text.replace(
            &format!("owner<{ty},i64>(reject<{ty}>(relay<{ty}>("),
            &format!("owner<{ty},i64>(reject<{ty}>(forward<{ty}>("),
        );
    }
    if inferred {
        // Padded omission preserves every span in complete instance equality.
        text = text.replace("{relay<T>(relay<T>(value))}", "{relay   (relay   (value))}");
        for (ty, _) in SCALARS {
            for (callee, vector) in [
                ("owner", format!("<{ty},i64>")),
                ("reject", format!("<{ty}>")),
                ("forward", format!("<{ty}>")),
            ] {
                text = text.replace(
                    &format!("{callee}{vector}("),
                    &format!("{callee}{}(", " ".repeat(vector.len())),
                );
            }
        }
    }
    text
}

#[test]
fn inferred_transitive_owned_forwarding_and_nested_calls_execute_once_on_every_engine() {
    for allowed in [true, false] {
        run_nested_case(
            allowed,
            &forwarding_source(true, allowed),
            &forwarding_source(false, allowed),
        );
    }
}

fn run_nested_case(allowed: bool, inferred_text: &str, explicit_text: &str) {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    let inferred = semaprax::check(inferred_text, "inference-nested.spx").unwrap();
    let explicit = semaprax::check(explicit_text, "inference-nested-explicit.spx").unwrap();
    let inferred_hir = hir::resolve(&inferred).unwrap();
    let explicit_hir = hir::resolve(&explicit).unwrap();
    hir::validate(&inferred_hir).unwrap();
    hir::validate(&explicit_hir).unwrap();
    assert_eq!(
        inferred_hir.function_instances,
        explicit_hir.function_instances
    );
    if inferred_text.contains("inference.forward") {
        assert_eq!(
            inferred_hir
                .function_instances
                .iter()
                .filter(|i| i.template.as_str() == "inference.forward")
                .count(),
            8
        );
    }
    let instances: Vec<_> = inferred_hir
        .function_instances
        .iter()
        .filter(|i| i.template.as_str() == "inference.owner")
        .collect();
    assert_eq!(instances.len(), 8);
    for instance in instances {
        assert_eq!(instance.type_arguments.len(), 2);
        assert_eq!(instance.type_arguments[1], hir::ResolvedType::I64);
    }
    let canonical = semaprax::format::canonical(&inferred);
    let reparsed = semaprax::check(&canonical, "inference-nested-roundtrip.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let graph = semaprax::graph::to_json(&inferred).unwrap();
    semaprax::graph::verify_json(&inferred, &graph).unwrap();
    let evaluate = |program: &hir::ResolvedProgram| {
        let call = interpreter::retained_call::prepare_retained_call(program, "app.main").unwrap();
        interpreter::retained_call::evaluate_retained_call(program, &call, &[], 100_000).unwrap()
    };
    let actual = evaluate(&inferred_hir);
    let expected = evaluate(&explicit_hir);
    assert_eq!(actual.outcome, expected.outcome);
    assert_eq!(
        actual.steps_used, expected.steps_used,
        "inference must not add evaluation steps"
    );
    let expected = if allowed {
        Expected::Value(8)
    } else {
        Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure")
    };
    run_interpreter_source("nested inferred owned calls", inferred_text, expected);
    if clang {
        run_native(&inferred, expected);
        native_copy_count(&inferred, allowed, if allowed { 8 } else { 1 });
    }
    if node {
        run_wasm_source(&inferred, explicit_text, expected);
        wasm_copy_count(&inferred, allowed, if allowed { 8 } else { 1 });
    }
}

fn native_copy_count(parsed: &semaprax::ast::Program, allowed: bool, copies: usize) {
    let generated = codegen::emit_c(parsed).unwrap();
    let allocation = "uint8_t *payload = (uint8_t *)malloc(";
    assert_eq!(generated.matches(allocation).count(), 1);
    let generated = generated.replace(
        allocation,
        "uint8_t *payload = (uint8_t *)inference_counted_malloc(",
    );
    let allocator = r#"
#include <stdint.h>
#include <stdlib.h>
static uint64_t inference_copies = 0;
static void *inference_counted_malloc(size_t size) { inference_copies += 1; return malloc(size); }
"#;
    let check = if allowed {
        "status != SPX_STATUS_SUCCESS || result != INT64_C(8)"
    } else {
        "status == SPX_STATUS_SUCCESS || result != INT64_C(77)"
    };
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[32]; struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(uint32_t i=0;i<4;i++){{
  inference_copies=0;int64_t result=INT64_C(77);
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&result);
  if({check})return 2;
  if(inference_copies!={copies})return 3;
 }}
 return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-inference-once-{}-{serial}",
            std::process::id()
        ));
        let c = base.with_extension("c");
        let executable = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        assert!(
            Command::new(&executable).status().unwrap().success(),
            "constructor evaluation count differs"
        );
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

fn wasm_copy_count(parsed: &semaprax::ast::Program, allowed: bool, copies: usize) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-inference-once-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(parsed, &root).unwrap();
    let path = root.join("semaprax.js");
    let js = std::fs::read_to_string(&path).unwrap();
    let original = "spx_bytes_copy: carrier => allocate(read(decode(carrier))),";
    assert_eq!(js.matches(original).count(), 1);
    std::fs::write(path,js.replace(original,"spx_bytes_copy: carrier => {globalThis.inferenceCopies += 1; return allocate(read(decode(carrier)));},")).unwrap();
    let invocation = if allowed {
        "if(instance.exports.semaprax_main()!==8n)throw Error('wrong value');"
    } else {
        "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status?.domain_id!=='semaprax.contract.v1'||status.code!==1)throw error;failed=true;}if(!failed)throw Error('missing failure');"
    };
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(root.join("probe.mjs"),format!("import {{readFile}} from 'node:fs/promises';\nimport {{instantiateBytes,semanticStatus}} from './semaprax.js';\nconst {{instance}}=await instantiateBytes(await readFile('./app.wasm'),{{maxOwnedByteEntries:1}});\nfor(let i=0;i<4;i++){{globalThis.inferenceCopies=0;{invocation}if(globalThis.inferenceCopies!=={copies})throw Error('constructor evaluated extra times: '+globalThis.inferenceCopies);}}\n")).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
