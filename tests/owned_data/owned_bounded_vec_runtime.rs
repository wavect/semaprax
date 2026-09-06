use std::process::Command;

use semaprax::{codegen, hir, interpreter, parse, wasm};

const SOURCE: &str = r#"
module test.owned_vec_runtime;

@id("vec.main")
fn main() -> i64
{
    let mut i64s = vec_with_capacity<i64>(3usize);
    i64s = vec_push<i64>(i64s, 11);
    i64s = vec_push<i64>(i64s, 22);
    i64s = vec_push<i64>(i64s, 33);
    let mut i32s = vec_with_capacity<i32>(1usize);
    i32s = vec_push<i32>(i32s, 12i32);
    let mut u8s = vec_with_capacity<u8>(1usize);
    u8s = vec_push<u8>(u8s, 13u8);
    let mut usizes = vec_with_capacity<usize>(1usize);
    usizes = vec_push<usize>(usizes, 14usize);
    let mut chars = vec_with_capacity<char>(1usize);
    chars = vec_push<char>(chars, 'A');
    let mut f32s = vec_with_capacity<f32>(1usize);
    f32s = vec_push<f32>(f32s, 1.5f32);
    let mut f64s = vec_with_capacity<f64>(1usize);
    f64s = vec_push<f64>(f64s, 2.5);
    let mut bools = vec_with_capacity<bool>(1usize);
    bools = vec_push<bool>(bools, true);
    if vec_len<i64>(i64s) == 3usize
        && vec_capacity<i64>(i64s) == 3usize
        && vec_get<i64>(i64s, 0usize) == 11
        && vec_get<i64>(i64s, 1usize) == 22
        && vec_get<i64>(i64s, 2usize) == 33
        && vec_get<i32>(i32s, 0usize) == 12i32
        && vec_get<u8>(u8s, 0usize) == 13u8
        && vec_get<usize>(usizes, 0usize) == 14usize
        && vec_get<char>(chars, 0usize) == 'A'
        && vec_get<f32>(f32s, 0usize) == 1.5f32
        && vec_get<f64>(f64s, 0usize) == 2.5
        && vec_get<bool>(bools, 0usize)
    { 7 } else { 1 }
}
"#;

const PUSH_FULL: &str = r#"
module test.owned_vec_push_full;
@id("vec.push-full")
fn main() -> i64 {
    let mut values = vec_with_capacity<i64>(0usize);
    values = vec_push<i64>(values, 1);
    0
}
"#;

const GET_OOB: &str = r#"
module test.owned_vec_get_oob;
@id("vec.get-oob")
fn main() -> i64 {
    let values = vec_with_capacity<i64>(1usize);
    vec_get<i64>(values, 0usize)
}
"#;

const ALLOCATION_FAILURE: &str = r#"
module test.owned_vec_allocation;
@id("vec.allocation")
fn main() -> i64 {
    let capacity = 8193usize;
    let values = vec_with_capacity<i64>(capacity);
    let length = vec_len<i64>(values);
    0
}
"#;

const NESTED_IF: &str = r#"
module test.owned_vec_nested_if;
@id("vec.nested-if")
fn main() -> i64 {
    if true {
        let mut values = vec_with_capacity<i64>(1usize);
        values = vec_push<i64>(values, 41);
        vec_get<i64>(values, 0usize)
    } else { 0 }
}
"#;

#[test]
fn owned_bounded_vec_copy_scalars_run_all_engines_without_owner_copy() {
    let ast = parse(SOURCE, "owned-bounded-vec-runtime.spx").unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    for element in [
        hir::ResolvedType::I64,
        hir::ResolvedType::I32,
        hir::ResolvedType::U8,
        hir::ResolvedType::Usize,
        hir::ResolvedType::Char,
        hir::ResolvedType::F32,
        hir::ResolvedType::F64,
        hir::ResolvedType::Bool,
    ] {
        let facts = resolved
            .declarations
            .type_facts(&hir::ResolvedType::Nominal {
                declaration: hir::DeclarationId::new("core.vec"),
                arguments: vec![element],
            })
            .unwrap();
        assert!(facts.sized && facts.needs_drop && !facts.copy);
    }
    let root =
        std::env::temp_dir().join(format!("semaprax-owned-vec-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source_path = root.join("program.spx");
    std::fs::write(&source_path, SOURCE).unwrap();
    let interpreted = interpreter::interpret(
        &source_path,
        "vec.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(interpreted.envelope.contains("\"value\":\"7\""));

    let generated = codegen::emit_c(&ast).unwrap();
    let core_wasm = wasm::emit_module(&ast).unwrap();
    assert!(!generated.contains("memcpy(result, source"));
    assert!(!generated.contains("spx_vec_v1 moved = *source;\n    *result = moved"));
    for payload in wasmparser::Parser::new(0).parse_all(&core_wasm) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(!matches!(
                    operators.read().unwrap(),
                    wasmparser::Operator::MemoryCopy { .. }
                        | wasmparser::Operator::MemoryGrow { .. }
                ));
            }
        }
    }
    let c_path = root.join("program.c");
    std::fs::write(&c_path, &generated).unwrap();
    assert!(Command::new("clang").arg("--version").output().is_ok());
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("program-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization} native execution failed"
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
    }
    if Command::new("node").arg("--version").output().is_ok() {
        let wasm_path = root.join("program.wasm");
        std::fs::write(&wasm_path, core_wasm).unwrap();
        let script = r#"
const fs=require('fs');
const bytes=fs.readFileSync(process.argv[1]);
let next=1n;
const entries=new Map();
const key=value=>{if(typeof value!=='bigint'||value===0n)throw Error('carrier');return value.toString()};
const read=(value,tag)=>{const entry=entries.get(key(value));if(!entry||entry.tag!==tag)throw Error('stale-or-type');return entry};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={
  spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
  spx_contract_fail:code=>{throw Error(`unexpected-status:${code}`)},
  spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?alloc(tag,n):0n},
  spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;const values=old.values.concat([bits]);entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),
  spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),
  spx_vec_get:(source,tag,index)=>{const entry=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=entry.values.length)throw Error('oob');return entry.values[n]},
  spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')}
};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{
  for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==7n||entries.size!==0)throw Error(`semantic-or-settlement:${value}:${entries.size}`)}
}).catch(error=>{console.error(error);process.exit(2)});
"#;
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(&wasm_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Core-Wasm: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owned_bounded_vec_failures_are_sticky_and_settle_before_reentry() {
    for (name, source, code, selector) in [
        ("push-full", PUSH_FULL, 1_u64, 13_i32),
        ("get-oob", GET_OOB, 2_u64, 14_i32),
        ("allocation", ALLOCATION_FAILURE, 3_u64, 15_i32),
    ] {
        let ast = parse(source, format!("owned-vec-{name}.spx")).unwrap();
        let root =
            std::env::temp_dir().join(format!("semaprax-owned-vec-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("program.spx");
        std::fs::write(&source_path, source).unwrap();
        for _ in 0..4 {
            let result = interpreter::interpret(
                &source_path,
                match name {
                    "push-full" => "vec.push-full",
                    "get-oob" => "vec.get-oob",
                    _ => "vec.allocation",
                },
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            assert!(!result.returned, "{name}");
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(
                envelope["payload"]["outcome"]["status"]["domain_id"],
                "semaprax.vec.v1"
            );
            assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
        }

        let generated = codegen::emit_c(&ast).unwrap();
        let c_path = root.join("program.c");
        std::fs::write(&c_path, generated).unwrap();
        if Command::new("clang").arg("--version").output().is_ok() {
            for optimization in ["-O0", "-O2"] {
                let binary = root.join(format!("program-{optimization}"));
                let compiled = Command::new("clang")
                    .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
                assert!(
                    compiled.status.success(),
                    "{name}/{optimization}: {}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                for _ in 0..4 {
                    let output = Command::new(&binary).output().unwrap();
                    assert_eq!(output.status.code(), Some(73), "{name}/{optimization}");
                    assert_eq!(
                        String::from_utf8_lossy(&output.stderr),
                        format!("SEMAPRAX operation failure: semaprax.vec.v1/{code}\n")
                    );
                }
            }
        }

        if Command::new("node").arg("--version").output().is_ok() {
            let wasm_path = root.join("program.wasm");
            std::fs::write(&wasm_path, wasm::emit_module(&ast).unwrap()).unwrap();
            let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]);
let next=1n;const entries=new Map(),key=v=>v.toString();
const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('carrier');return e};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:selector=>{const code=selector-12;if(selector<13||selector>15)throw Error('selector');throw Object.assign(Error('vec failure'),{domain_id:'semaprax.vec.v1',code})},
spx_vec_with_capacity:(tag,c)=>{const n=Number(c);return n<=8192?alloc(tag,n):0n},
spx_vec_push:(v,t,b)=>{const e=read(v,t);if(e.values.length>=e.capacity)return 0n;entries.delete(key(v));return alloc(t,e.capacity,e.values.concat([b]))},
spx_vec_len:(v,t)=>BigInt(read(v,t).values.length),spx_vec_capacity:(v,t)=>BigInt(read(v,t).capacity),
spx_vec_get:(v,t,i)=>read(v,t).values[Number(i)],spx_vec_drop:v=>{if(!entries.delete(key(v)))throw Error('drop')}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(error){if(error.domain_id!=='semaprax.vec.v1'||error.code!==expected-12)throw error;failed=true}if(!failed||entries.size!==0)throw Error(`failure-or-arena:${failed}:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
"#;
            let output = Command::new("node")
                .arg("-e")
                .arg(script)
                .arg(&wasm_path)
                .arg(selector.to_string())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name} Core-Wasm: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn owned_bounded_vec_native_rejects_stale_and_forged_carriers_before_access() {
    assert!(Command::new("clang").arg("--version").output().is_ok());
    let ast = parse(SOURCE, "owned-vec-native-hostile.spx").unwrap();
    let generated = codegen::emit_c(&ast).unwrap();
    let probe = r#"
#undef main
int main(int argc, char **argv) {
    struct spx_status_entry entries[UINT32_C(4)]; struct spx_context ctx = {0};
    if (!spx_context_init(&ctx, UINT64_C(77), entries, UINT32_C(4), NULL, NULL, NULL)) return 2;
    spx_vec_v1 value = {0};
    if (spx_vec_with_capacity(&ctx, UINT32_C(1), UINT64_C(1), &value) != SPX_STATUS_SUCCESS) return 3;
    if (argc == 1) { spx_vec_drop(&ctx, &value); return 0; }
    if (strcmp(argv[1], "stale") == 0) {
        spx_vec_v1 stale = value; spx_vec_v1 moved = spx_vec_move(&ctx, &value); (void)moved;
        (void)spx_vec_len(&ctx, &stale, UINT32_C(1));
    } else if (strcmp(argv[1], "pointer-read") == 0) {
        value.ptr = (uint64_t *)(uintptr_t)UINT64_C(1); (void)spx_vec_len(&ctx, &value, UINT32_C(1));
    } else if (strcmp(argv[1], "pointer-free") == 0) {
        value.ptr = (uint64_t *)(uintptr_t)UINT64_C(1); spx_vec_drop(&ctx, &value);
    } else if (strcmp(argv[1], "tag") == 0) {
        value.type_tag = UINT32_C(2); (void)spx_vec_len(&ctx, &value, UINT32_C(1));
    } else if (strcmp(argv[1], "generation") == 0) {
        value.generation += UINT64_C(1); spx_vec_drop(&ctx, &value);
    } else return 4;
    return 5;
}
"#;
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-vec-native-hostile-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let c_path = root.join("probe.c");
    std::fs::write(
        &c_path,
        format!("#define main spx_generated_main\n{generated}\n{probe}"),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("probe-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        for attack in ["stale", "pointer-read", "pointer-free", "tag", "generation"] {
            assert!(
                !Command::new(&binary)
                    .arg(attack)
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "{optimization}/{attack} did not fail-stop"
            );
            assert!(
                Command::new(&binary).output().unwrap().status.success(),
                "{optimization}/{attack} poisoned fresh reentry"
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nested_if_vec_routes_complete_core_wasm_imports_and_executes() {
    assert!(Command::new("node").arg("--version").output().is_ok());
    let ast = parse(NESTED_IF, "owned-vec-nested-if.spx").unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-vec-nested-if-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, bytes).unwrap();
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),module=new WebAssembly.Module(bytes),names=WebAssembly.Module.imports(module).map(value=>value.name);
for(const name of ['spx_vec_with_capacity','spx_vec_push','spx_vec_len','spx_vec_capacity','spx_vec_get','spx_vec_drop'])if(!names.includes(name))throw Error(`missing:${name}`);
let next=1n;const entries=new Map(),key=v=>v.toString(),read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('carrier');return e},alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('status')},spx_vec_with_capacity:(t,c)=>alloc(t,Number(c)),spx_vec_push:(v,t,b)=>{const e=read(v,t);entries.delete(key(v));return alloc(t,e.capacity,e.values.concat([b]))},spx_vec_len:(v,t)=>BigInt(read(v,t).values.length),spx_vec_capacity:(v,t)=>BigInt(read(v,t).capacity),spx_vec_get:(v,t,i)=>read(v,t).values[Number(i)],spx_vec_drop:v=>{if(!entries.delete(key(v)))throw Error('drop')}};
WebAssembly.instantiate(module,{env}).then(({exports})=>{for(let i=0;i<3;i++){const value=exports.semaprax_main();if(value!==41n||entries.size!==0)throw Error(`execution:${value}:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "nested-if Core-Wasm: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}
