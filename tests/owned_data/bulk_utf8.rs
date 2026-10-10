//! Checked UTF-8 copy: exact bytes, scoped input, strict status, physical settlement.
use semaprax::{codegen, interpreter, wasm};
use sha2::{Digest as _, Sha256};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn directory() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "bulk-utf8-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn native(
    ast: &semaprax::ast::Program,
    root: &std::path::Path,
    domain: &str,
    code: u32,
    expected: i64,
    allocations: i64,
) {
    let resolved = semaprax::hir::resolve(ast).unwrap();
    let generated = codegen::emit_hir_c(&resolved).unwrap();
    let tracked = generated
        .replace("malloc(", "probe_malloc(")
        .replace("calloc(", "probe_calloc(")
        .replace("realloc(", "probe_realloc(")
        .replace("free(", "probe_free(");
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t live=0,allocations=0;
static __attribute__((unused)) void *probe_malloc(size_t n){void*p=malloc(n);if(p){++live;++allocations;}return p;}
static __attribute__((unused)) void *probe_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p){++live;++allocations;}return p;}
static __attribute__((unused)) void *probe_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p){++live;++allocations;}return r;}
static __attribute__((unused)) void probe_free(void*p){if(p){if(!live)abort();--live;free(p);}}
"#;
    let probe = format!(
        r#"
int main(void){{
 struct spx_status_entry entries[32];struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<3;++repeat){{
  allocations=0;int64_t result=INT64_C(252525);
  spx_status_token token=spx_decl_6170702e6d61696e(&context,&result);
  if({code}==0){{if(token!=SPX_STATUS_SUCCESS||result!=INT64_C({expected}))return 2;}}
  else{{const struct spx_normalized_status*status=spx_status_resolve(&context,token);
   if(token==SPX_STATUS_SUCCESS||result!=INT64_C(252525)||!status||strcmp(status->domain_id,"{domain}")||status->code!={code})return 3;}}
  if(live)return 4;
  if(INT64_C({allocations})>=0&&allocations!=(uint64_t)INT64_C({allocations}))return 5;
 }}return 0;
}}
"#
    );
    let c = root.join("probe.c");
    std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("probe{optimization}"));
        let output = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                optimization,
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status = Command::new(binary).status().unwrap();
        assert!(status.success(), "native {optimization}: {status}");
    }
}

fn run(source: &str, expected: i64, failure: Option<(&str, u32, u32)>, allocations: i64) {
    let root = directory();
    let path = root.join("app.spx");
    std::fs::write(&path, source).unwrap();
    let ast = semaprax::check(source, &path).unwrap();
    // This owner exercises the explicit private String evaluator. The public
    // legacy evaluator must continue refusing String-returning helper calls.
    let legacy = interpreter::interpret(
        &path,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    );
    if source.contains("fn copy(") {
        assert!(legacy
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-F102"));
    }
    for _ in 0..3 {
        let result = interpreter::internal_strings::interpret(
            &path,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        interpreter::internal_strings::verify_envelope(&result.envelope).unwrap();
        if let Some((domain, code, _)) = failure {
            assert!(!result.returned, "{}", result.envelope);
            assert!(result
                .envelope
                .contains(&format!("\"domain_id\":\"{domain}\"")));
            assert!(result.envelope.contains(&format!("\"code\":{code}")));
        } else {
            assert!(result.returned, "{}", result.envelope);
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(
                envelope["payload"]["outcome"]["value"],
                expected.to_string()
            );
        }
    }
    let (domain, code, status) = failure.unwrap_or(("", 0, 0));
    native(&ast, &root, domain, code, expected, allocations);
    let bytes = wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let module = root.join("app.wasm");
    std::fs::write(&module, &bytes).unwrap();
    // Reuse the strict owner host, omitting only its unrelated mandatory Vec
    // import probe: this source has no collection operation or hidden owner.
    let original = include_str!("owned_leaf_vec/host.js");
    let probe="  const missing={...env};delete missing.spx_vec_leaf_clone_at_v1;\n  let refused=false;\n  try{await WebAssembly.instantiate(moduleBytes,{env:missing})}\n  catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;refused=true}\n  if(!refused)throw Error('private boundary linked without clone import');\n";
    assert_eq!(original.matches(probe).count(), 1);
    let host = original.replace(probe, "");
    let host = host.replace(
        "codecPushAttempts=0;fieldReads=0;",
        "codecPushAttempts=0;fieldReads=0;const bulkCopiesBefore=copies;",
    );
    let host = host.replace("if(selected!==expectedStatus)", &format!("if({allocations}>=0&&copies-bulkCopiesBefore!=={allocations})throw Error('bulk constructor copied an unexpected number of payloads');if(selected!==expectedStatus)"));
    let output = Command::new("node")
        .arg("-e")
        .arg(host)
        .arg(module)
        .args([status.to_string(), expected.to_string(), "bulk-utf8".into()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Exercise the production browser adapter too. This is the actual private
    // aggregate module with digest binding, not a public-export admission.
    let digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))
    );
    let runtime = include_str!("../../src/wasm/browser_runtime.js")
        .replace("__SEMAPRAX_OWNED_EXPORTS__", "{}")
        .replace("__SEMAPRAX_WASM_SHA256__", &digest);
    std::fs::write(root.join("runtime.mjs"), runtime).unwrap();
    std::fs::write(root.join("run.mjs"), format!(r#"
import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './runtime.mjs';
const {{instance}}=await instantiateBytes(await readFile('./app.wasm'),{{maxOwnedByteEntries:4}});
for(let i=0;i<3;i++){{
 let selected=null,value;
 try{{value=instance.exports.semaprax_main();}}catch(error){{selected=semanticStatus(error);if(!selected)throw error;}}
 if({code}===0){{if(selected||value!=={expected}n)throw Error('bulk adapter success mismatch');}}
 else if(!selected||selected.domain_id!=={domain:?}||selected.code!=={code})throw Error('bulk adapter status mismatch');
}}
"#)).unwrap();
    let output = Command::new("node")
        .arg("run.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bulk_utf8_copies_exact_unicode_ranges_once_and_detaches_from_input() {
    // Invalid prefix/suffix prove that only the authenticated range is read.
    let source = r#"module bulk.valid;
@id("copy") fn copy()->string {let raw=[255u8,0u8,239u8,187u8,191u8,127u8,194u8,128u8,224u8,160u8,128u8,239u8,191u8,191u8,244u8,143u8,191u8,191u8,255u8];let input=array_as_slice(raw);string_from_utf8(byte_range(input,1usize,18usize))}
@id("app.main") fn main()->i64 {let text=copy();let bytes=str_as_bytes(string_as_str(text));let first=match byte_get(bytes,0usize){Option::Some{value}=>value==0u8,Option::None{}=>false,};let last=match byte_get(bytes,16usize){Option::Some{value}=>value==191u8,Option::None{}=>false,};if first && last && byte_len(bytes)==17usize{42}else{0}}
"#;
    run(source, 42, None, 1);
    let empty="module bulk.empty;@id(\"app.main\") fn main()->i64{let raw=[255u8];let input=array_as_slice(raw);let text=string_from_utf8(byte_range(input,0usize,0usize));str_len_bytes(string_as_str(text))}";
    run(empty, 0, None, 1);
    // Named borrowed slices remain valid across iterations; each conversion
    // owns one independent result and settles it before the next iteration.
    let looped = "module bulk.looped;@id(\"app.main\") fn main()->i64{let raw=[65u8,0u8,195u8,169u8];let view=array_as_slice(raw);let mut total=0;let mut i=0;while i<3{let text=string_from_utf8(view);total=total+str_len_bytes(string_as_str(text));i=i+1;0}total}";
    run(looped, 12, None, 3);
    // Existing internal owned capacity is not a foreign 64-KiB input limit.
    let large="module bulk.large;@id(\"app.main\") fn main()->i64{let raw=bytes_zeroed(131072usize);let text=string_from_utf8(bytes_as_slice(raw));string_len(text)}";
    run(large, 131072, None, -1);
}

#[test]
fn bulk_utf8_malformed_classes_select_conversion_without_result_allocation() {
    for bytes in [
        &[128u8][..],
        &[192, 128],
        &[193, 191],
        &[194],
        &[194, 65],
        &[224, 128, 128],
        &[237, 160, 128],
        &[237, 191, 191],
        &[240, 128, 128, 128],
        &[244, 144, 128, 128],
        &[245, 128, 128, 128],
        &[255],
        &[240, 159, 152],
    ] {
        let array = bytes
            .iter()
            .map(|b| format!("{b}u8"))
            .collect::<Vec<_>>()
            .join(",");
        let source=format!("module bulk.invalid;@id(\"app.main\") fn main()->i64{{let raw=[{array}];let unpublished=string_from_utf8(array_as_slice(raw));0}}");
        run(&source, 0, Some(("semaprax.convert.v1", 1, 21)), 0);
    }
}

#[test]
fn bulk_utf8_lazy_branches_and_staged_owners_preserve_first_failure() {
    let prefix = r#"module bulk.order;
@id("sink") fn sink(text:string,other:string)->i64{0}
@id("bad") fn bad()->string{let raw=[255u8];string_from_utf8(array_as_slice(raw))}
@id("boom") fn boom()->string{let bad=9223372036854775807+1;"unpublished"}
"#;
    for (body, failure) in [
        ("if true{42}else{let bad=bad();0}", None),
        (
            "sink(\"already-staged\",bad())",
            Some(("semaprax.convert.v1", 1, 21)),
        ),
        ("sink(boom(),bad())", Some(("semaprax.arithmetic.v1", 1, 1))),
        ("sink(bad(),boom())", Some(("semaprax.convert.v1", 1, 21))),
        (
            "let live=bytes_zeroed(3usize);let bad=bad();0",
            Some(("semaprax.convert.v1", 1, 21)),
        ),
    ] {
        run(
            &format!("{prefix}@id(\"app.main\") fn main()->i64{{{body}}}"),
            42,
            failure,
            -1,
        );
    }
}
