//! Box and Vec generic calls run with exact owner settlement on every engine.
use super::*;
#[test]
fn generic_collections_all_scalars_execute_on_every_engine() {
    for mode in 0..3 {
        run_profile(mode);
    }
}
fn run_profile(mode: u32) {
    let mut source = String::from(
        r#"module test.generic_collections_runtime;
@id("c.make") fn make<T>(value:T)->Box<T>{box_new<T>(value)}
@id("c.relay") fn relay<T>(value:own Box<T>)->Box<T>{value}
@id("c.local") fn local<T>(value:T)->T{box_into_inner<T>(box_new<T>(value))}
@id("c.take") fn take<T>(value:own Box<T>,allowed:bool)->T requires allowed {box_into_inner<T>(value)}
@id("c.vector") fn vector<T>(value:T,capacity:usize)->Vec<T>{
 let initial=vec_push<T>(vec_with_capacity<T>(capacity),value);
 let reserved=vec_reserve_exact<T>(initial,2usize);
 let replaced=vec_set<T>(reserved,0usize,value);
 let cleared=vec_clear<T>(replaced);
 vec_push<T>(cleared,value)
}
@id("c.read") fn read<T>(value:own Vec<T>)->T{vec_get<T>(value,0usize)}
"#,
    );
    let mut calls = Vec::new();
    for (ty, literal) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'x'"),
        ("f32", "1.5f32"),
        ("f64", "1.5"),
        ("bool", "true"),
    ] {
        let allowed = mode != 1;
        let capacity = if mode == 2 { 0 } else { 1 };
        source.push_str(&format!("@id(\"c.run.{ty}\") fn run_{ty}()->i64{{let dropped=make<{ty}>({literal});let seen=box_get<{ty}>(dropped);let value=take<{ty}>(relay<{ty}>(make<{ty}>(seen)),{allowed});let values=vector<{ty}>(local<{ty}>(value),{capacity}usize);if vec_len<{ty}>(values)==1usize && vec_capacity<{ty}>(values)==3usize && read<{ty}>(values)=={literal}{{1}}else{{0}}}}\n"));
        calls.push(format!("run_{ty}()"));
    }
    source.push_str(&format!(
        "@id(\"app.main\") fn main()->i64{{{}}}",
        calls.join("+")
    ));
    run_source(&source, mode);
}

pub(super) fn run_source(source: &str, mode: u32) {
    let expected = match mode {
        0 => Expected::Value(8),
        1 => Expected::Failure("semaprax.contract.v1", 1, "SEMAPRAX contract failure"),
        2 => Expected::Failure("semaprax.vec.v1", 1, "bounded Vec capacity failure"),
        3 => Expected::Failure("semaprax.vec.v1", 3, "bounded Vec allocation failure"),
        _ => panic!("unknown generic collection runtime mode"),
    };
    run_source_with_expected(source, expected);
}

/// Reuses the full interpreter/native/Core-Wasm Vec host and its live-registry
/// settlement checks for a successful callback composition with a distinct value.
pub(super) fn run_source_value(source: &str, value: i64) {
    run_source_with_expected(source, Expected::Value(value));
}

pub(super) fn run_source_postcondition_failure(source: &str) {
    run_source_with_expected(
        source,
        Expected::Failure("semaprax.contract.v1", 2, "SEMAPRAX contract failure"),
    );
}

fn run_source_with_expected(source: &str, expected: Expected) {
    let parsed = semaprax::check(source, "generic-collections-runtime.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "generic-collections-canonical.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&reparsed));
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let graph = semaprax::graph::to_json(&parsed).unwrap();
    semaprax::graph::verify_json(&parsed, &graph).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-collections-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    run_interpreter_source("generic collections", source, expected);
    run_native_collections(&parsed, expected, None);
    let core_wasm = wasm::emit_module(&parsed).unwrap();
    let scalar_closure_profile = hir::closure::requires_closures(&resolved);
    for payload in wasmparser::Parser::new(0).parse_all(&core_wasm) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            let mut previous_i32 = None;
            while !operators.eof() {
                let operator = operators.read().unwrap();
                assert!(!matches!(operator, wasmparser::Operator::MemoryGrow { .. }));
                if matches!(operator, wasmparser::Operator::MemoryCopy { .. }) {
                    // Graph v37 adds only a fixed Copy-scalar environment. All
                    // pre-closure collection fixtures still forbid every copy;
                    // this profile permits exactly its 80-byte frame carrier.
                    assert!(
                        scalar_closure_profile,
                        "owning collection carrier must not be copied"
                    );
                    assert_eq!(
                        previous_i32,
                        Some(80),
                        "only the fixed closure carrier may be copied"
                    );
                }
                previous_i32 = match operator {
                    wasmparser::Operator::I32Const { value } => Some(value),
                    _ => None,
                };
            }
        }
    }
    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, core_wasm).unwrap();
    assert!(Command::new("node").arg("--version").output().is_ok());
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectedValue=BigInt(process.argv[3]);
let next=1n;const entries=new Map(),key=v=>{if(typeof v!=='bigint'||v===0n)throw Error('carrier');return v.toString()};
const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('stale-or-type');return e};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:s=>{throw Object.assign(new Error(`status:${s}`),{status:Number(s)})},
spx_box_new:(tag,bits)=>{if(entries.size>=4096)return 0n;const token=next++;entries.set(key(token),{tag,bits});return token},
spx_box_get:(v,t)=>read(v,t).bits,
spx_box_into_inner:(v,t)=>{const e=read(v,t);entries.delete(key(v));return e.bits},
spx_box_drop:v=>{if(!entries.delete(key(v)))throw Error('double-drop')},
  spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?alloc(tag,n):0n},
  spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;const values=old.values.concat([bits]);entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),
  spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),
  spx_vec_get:(source,tag,index)=>{const entry=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=entry.values.length)throw Error('oob');return entry.values[n]},
  spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')},
  spx_vec_reserve_exact:(source,tag,additional)=>{const old=read(source,tag),n=Number(additional),capacity=Math.max(old.capacity,old.values.length+n);if(!Number.isSafeInteger(n)||n<0||capacity>8192)return 0n;entries.delete(key(source));return alloc(tag,capacity,old.values.slice())},
  spx_vec_set:(source,tag,index,bits)=>{const old=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=old.values.length)return 0n;const values=old.values.slice();values[n]=bits;entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_clear:(source,tag)=>{const old=read(source,tag);entries.delete(key(source));return alloc(tag,old.capacity,[])}
};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){let caught=null,value;try{value=instance.exports.semaprax_main()}catch(error){caught=error}if(expected===0?(caught!==null||value!==expectedValue):(!caught||caught.status!==expected))throw Error(`semantic:${value}:${caught}`);if(entries.size!==0)throw Error(`settlement:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm_path)
        .arg(match expected {
            Expected::Value(_) => "0",
            Expected::Failure("semaprax.contract.v1", 1, _) => "9",
            Expected::Failure("semaprax.contract.v1", 2, _) => "10",
            Expected::Failure("semaprax.vec.v1", 1, _) => "13",
            Expected::Failure("semaprax.vec.v1", 3, _) => "15",
            Expected::Failure(..) => {
                unreachable!("only established collection status codes reach the Core-Wasm host")
            }
        })
        .arg(match expected {
            Expected::Value(value) => value.to_string(),
            Expected::Failure(..) => "0".to_owned(),
        })
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Core-Wasm: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

/// Test-only host refusal: the valid input carrier allocates first and the
/// adapter output allocation is the second `with_capacity` request.
pub(super) fn run_backend_output_allocation_refusal(source: &str) {
    let parsed = semaprax::check(source, "generic-collections-refusal.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let graph = semaprax::graph::to_json(&parsed).unwrap();
    semaprax::graph::verify_json(&parsed, &graph).unwrap();
    let expected = Expected::Failure("semaprax.vec.v1", 3, "bounded Vec allocation failure");
    run_native_collections(&parsed, expected, Some(2));

    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-collections-refusal-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, wasm::emit_module(&parsed).unwrap()).unwrap();
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]);
let next=1n,capacityCalls=0;const entries=new Map(),key=v=>{if(typeof v!=='bigint'||v===0n)throw Error('carrier');return v.toString()};
const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('stale-or-type');return e};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:s=>{throw Object.assign(new Error(`status:${s}`),{status:Number(s)})},
spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);if(!Number.isSafeInteger(n)||n!==1)throw Error(`unexpected-capacity:${n}`);capacityCalls+=1;if(capacityCalls===2)return 0n;return alloc(tag,n)},
spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;entries.delete(key(source));return alloc(tag,old.capacity,old.values.concat([bits]))},
spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),
spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),
spx_vec_get:(source,tag,index)=>{const entry=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=entry.values.length)throw Error('oob');return entry.values[n]},
spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){capacityCalls=0;let caught=null;try{instance.exports.semaprax_main()}catch(error){caught=error}if(!caught||caught.status!==15)throw Error(`semantic:${caught}`);if(entries.size!==0)throw Error(`settlement:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "Core-Wasm refusal: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_native_collections(
    parsed: &semaprax::ast::Program,
    expected: Expected,
    refuse_calloc_call: Option<u64>,
) {
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    let mut ownership_surface = generated.clone();
    let admitted = "memcpy(entry->domain_storage, status.domain_id, domain_size);";
    assert_eq!(ownership_surface.matches(admitted).count(), 1);
    ownership_surface = ownership_surface.replacen(admitted, "", 1);
    if hir::closure::requires_closures(&hir::resolve(parsed).unwrap()) {
        // The new Copy-only closure profile uses exact scalar bit codecs.
        // Match their entire definitions so no aggregate/owner memcpy can be
        // hidden by a name or prefix exception. Frozen collection profiles
        // retain their original zero-copy assertion below.
        for (suffix, scalar) in [
            ("i64", "int64_t"),
            ("i32", "int32_t"),
            ("u32", "uint32_t"),
            ("f32", "float"),
            ("f64", "double"),
        ] {
            let initialization = if suffix == "f64" {
                ""
            } else {
                " = UINT64_C(0)"
            };
            let pack = format!("static __attribute__((unused)) uint64_t spx_closure_pack_{suffix}({scalar} value) {{ uint64_t cell{initialization}; memcpy(&cell, &value, sizeof value); return cell; }}");
            let unpack = format!("static __attribute__((unused)) {scalar} spx_closure_unpack_{suffix}(uint64_t cell) {{ {scalar} value; memcpy(&value, &cell, sizeof value); return value; }}");
            for definition in [pack, unpack] {
                assert_eq!(
                    ownership_surface.matches(&definition).count(),
                    1,
                    "scalar codec changed"
                );
                ownership_surface = ownership_surface.replacen(&definition, "", 1);
            }
        }
    }
    assert!(!ownership_surface.contains("memcpy("));
    let tracked = generated
        .replace("malloc(", "spx_test_malloc(")
        .replace("calloc(", "spx_test_calloc(")
        .replace("free(", "spx_test_free(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC spx_test_realloc",
        );
    let condition = match expected {
        Expected::Value(value) => {
            format!("status != SPX_STATUS_SUCCESS || result != INT64_C({value})")
        }
        Expected::Failure(domain, code, _) => format!(
            "status == SPX_STATUS_SUCCESS || result != INT64_C(0x2525252525252525) || spx_status_resolve(&context, status) == NULL || strcmp(spx_status_resolve(&context, status)->domain_id, \"{domain}\") != 0 || spx_status_resolve(&context, status)->code != UINT32_C({code})"
        ),
    };
    let probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(17), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t i = 0; i < UINT32_C(4); ++i) {{
    int64_t result = INT64_C(0x2525252525252525);
    uint32_t before = context.status_arena.length;
    spx_test_calloc_calls = UINT64_C(0);
    spx_test_refuse_calloc_call = UINT64_C({refuse_calloc_call});
    spx_status_token status = spx_decl_6170702e6d61696e(&context, &result);
    if ({condition}) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
    if (status == SPX_STATUS_SUCCESS && context.status_arena.length != before) return 4;
    if (status != SPX_STATUS_SUCCESS && context.status_arena.length != before + UINT32_C(1)) return 5;
    for (uint32_t authority = UINT32_C(0); authority < SPX_VEC_AUTHORITY_CAPACITY; ++authority) {{
      if (context.vec_authority[authority].live) return 6;
    }}
  }}
  return 0;
}}
"#,
        refuse_calloc_call = refuse_calloc_call.unwrap_or(0),
    );
    let allocator = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static uint64_t spx_test_calloc_calls = UINT64_C(0);
static uint64_t spx_test_refuse_calloc_call = UINT64_C(0);
static __attribute__((unused)) void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static __attribute__((unused)) void *spx_test_calloc(size_t count, size_t size) {
  spx_test_calloc_calls += UINT64_C(1);
  if (spx_test_refuse_calloc_call != UINT64_C(0)
      && spx_test_calloc_calls == spx_test_refuse_calloc_call) return NULL;
  void *allocation = calloc(count, size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static __attribute__((unused)) void *spx_test_realloc(void *old, size_t size) {
  if (size == 0) abort();
  int was_null = old == NULL;
  void *allocation = realloc(old, size);
  if (allocation != NULL && was_null) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) {
    if (spx_test_live_allocations == UINT64_C(0)) abort();
    spx_test_live_allocations -= UINT64_C(1);
    free(allocation);
  }
}
"#;
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-function-native-{}-{serial}",
            std::process::id()
        ));
        let c = base.with_extension("c");
        let executable = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let output = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg("-DSPX_NO_ENTRY_WRAPPER")
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}
