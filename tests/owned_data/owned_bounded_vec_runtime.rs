use std::process::Command;

use semaprax::{codegen, hir, interpreter, parse, wasm};

const SOURCE: &str = r#"
module test.owned_vec_runtime;

@id("vec.main")
fn main() -> i64
{
    let mut i64s = vec_with_capacity<i64>(1usize);
    i64s = vec_push<i64>(i64s, 11);
    i64s = vec_reserve_exact<i64>(i64s, 2usize);
    i64s = vec_push<i64>(i64s, 22);
    i64s = vec_push<i64>(i64s, 33);
    i64s = vec_set<i64>(i64s, 1usize, 44);
    let mut i32s = vec_with_capacity<i32>(1usize);
    i32s = vec_push<i32>(i32s, 12i32);
    i32s = vec_reserve_exact<i32>(i32s, 1usize);
    i32s = vec_set<i32>(i32s, 0usize, 22i32);
    let mut u8s = vec_with_capacity<u8>(1usize);
    u8s = vec_push<u8>(u8s, 13u8);
    u8s = vec_reserve_exact<u8>(u8s, 1usize);
    u8s = vec_set<u8>(u8s, 0usize, 23u8);
    let mut usizes = vec_with_capacity<usize>(1usize);
    usizes = vec_push<usize>(usizes, 14usize);
    usizes = vec_reserve_exact<usize>(usizes, 1usize);
    usizes = vec_set<usize>(usizes, 0usize, 24usize);
    let mut chars = vec_with_capacity<char>(1usize);
    chars = vec_push<char>(chars, 'A');
    chars = vec_reserve_exact<char>(chars, 1usize);
    chars = vec_set<char>(chars, 0usize, 'B');
    let mut f32s = vec_with_capacity<f32>(1usize);
    f32s = vec_push<f32>(f32s, 1.5f32);
    f32s = vec_reserve_exact<f32>(f32s, 1usize);
    f32s = vec_set<f32>(f32s, 0usize, 3.5f32);
    let mut f64s = vec_with_capacity<f64>(1usize);
    f64s = vec_push<f64>(f64s, 2.5);
    f64s = vec_reserve_exact<f64>(f64s, 1usize);
    f64s = vec_set<f64>(f64s, 0usize, 4.5);
    let mut bools = vec_with_capacity<bool>(1usize);
    bools = vec_push<bool>(bools, true);
    bools = vec_reserve_exact<bool>(bools, 1usize);
    bools = vec_set<bool>(bools, 0usize, false);
    bools = vec_clear<bool>(bools);
    if vec_len<i64>(i64s) == 3usize
        && vec_capacity<i64>(i64s) == 3usize
        && vec_get<i64>(i64s, 0usize) == 11
        && vec_get<i64>(i64s, 1usize) == 44
        && vec_get<i64>(i64s, 2usize) == 33
        && vec_get<i32>(i32s, 0usize) == 22i32
        && vec_get<u8>(u8s, 0usize) == 23u8
        && vec_get<usize>(usizes, 0usize) == 24usize
        && vec_get<char>(chars, 0usize) == 'B'
        && vec_get<f32>(f32s, 0usize) == 3.5f32
        && vec_get<f64>(f64s, 0usize) == 4.5
        && vec_len<bool>(bools) == 0usize
        && vec_capacity<bool>(bools) == 2usize
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

const RESERVE_FAILURE: &str = r#"
module test.owned_vec_reserve_failure;
@id("vec.reserve-failure")
fn main() -> i64 {
    let mut values = vec_with_capacity<i64>(1usize);
    values = vec_push<i64>(values, 1);
    let additional = 8192usize;
    values = vec_reserve_exact<i64>(values, additional);
    0
}
"#;

const SET_FAILURE: &str = r#"
module test.owned_vec_set_failure;
@id("vec.set-failure")
fn main() -> i64 {
    let mut values = vec_with_capacity<i64>(1usize);
    values = vec_push<i64>(values, 1);
    values = vec_set<i64>(values, 1usize, 2);
    0
}
"#;

const LOOP_CARRIED: &str = r#"
module test.owned_vec_loop_carried;

@id("vec.reading-at")
fn reading_at(index: i64) -> i64
    requires index >= 0
{
    (index * 37 + 11) % 100
}

@id("vec.kept")
fn kept(value: i64, threshold: i64) -> i64
{
    if value >= threshold { value } else { 0 }
}

@id("vec.alert-total")
fn alert_total(count: i64, threshold: i64) -> i64
    requires count >= 0 && count <= 12
{
    let mut readings = vec_with_capacity<i64>(12usize);
    let mut index = 0;
    while index < count {
        readings = vec_push<i64>(readings, reading_at(index));
        index = index + 1;
        index < count
    }
    let length = vec_len<i64>(readings);
    let mut position = 0usize;
    let mut total = 0;
    while position < length {
        total = total + kept(vec_get<i64>(readings, position), threshold);
        position = position + 1usize;
        position < length
    }
    total
}

@id("vec.loop-carried")
fn main() -> i64
{
    alert_total(9, 50)
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

const FOR_EACH_COPY_SCALARS: &str = r#"
module test.owned_vec_for_each;

@id("vec.for-each.empty")
fn empty_oracle() -> i64 {
    let empty = vec_with_capacity<i64>(0usize);
    let mut rolling = 7;
    for item in empty { rolling = rolling * 31 + item; 0 }
    rolling
}

@id("vec.for-each.one")
fn one_oracle() -> i64 {
    let one_base = vec_with_capacity<i64>(1usize);
    let one = vec_push<i64>(one_base, 4);
    let mut rolling = 7;
    for item in one { rolling = rolling * 31 + item; 0 }
    rolling
}

@id("vec.for-each.many")
fn many_oracle() -> i64 {
    let many_base = vec_with_capacity<i64>(4usize);
    let many_1 = vec_push<i64>(many_base, 1);
    let many_2 = vec_push<i64>(many_1, 2);
    let many = vec_push<i64>(many_2, 3);
    let mut rolling = 7;
    for item in many { rolling = rolling * 31 + item; 0 }
    rolling
}

@id("vec.for-each.full")
fn full_oracle() -> i64 {
    let full_base = vec_with_capacity<i64>(3usize);
    let full_1 = vec_push<i64>(full_base, 5);
    let full_2 = vec_push<i64>(full_1, 6);
    let full = vec_push<i64>(full_2, 7);
    let mut rolling = 7;
    for item in full { rolling = rolling * 31 + item; 0 }
    rolling
}

@id("vec.for-each.i32")
fn i32_oracle() -> i64 {
    let i32_base = vec_with_capacity<i32>(1usize);
    let i32s = vec_push<i32>(i32_base, 11i32);
    let mut observed = 9;
    for item in i32s { observed = if item == 11i32 { 1 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.u8")
fn u8_oracle() -> i64 {
    let u8_base = vec_with_capacity<u8>(1usize);
    let u8s = vec_push<u8>(u8_base, 12u8);
    let mut observed = 9;
    for item in u8s { observed = if item == 12u8 { 2 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.usize")
fn usize_oracle() -> i64 {
    let usize_base = vec_with_capacity<usize>(1usize);
    let usizes = vec_push<usize>(usize_base, 13usize);
    let mut observed = 9;
    for item in usizes { observed = if item == 13usize { 3 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.char")
fn char_oracle() -> i64 {
    let char_base = vec_with_capacity<char>(1usize);
    let chars = vec_push<char>(char_base, 'Q');
    let mut observed = 9;
    for item in chars { observed = if item == 'Q' { 4 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.f32")
fn f32_oracle() -> i64 {
    let f32_base = vec_with_capacity<f32>(1usize);
    let f32s = vec_push<f32>(f32_base, 1.25f32);
    let mut observed = 9;
    for item in f32s { observed = if item == 1.25f32 { 5 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.f64")
fn f64_oracle() -> i64 {
    let f64_base = vec_with_capacity<f64>(1usize);
    let f64s = vec_push<f64>(f64_base, 2.5);
    let mut observed = 9;
    for item in f64s { observed = if item == 2.5 { 6 } else { 9 }; 0 }
    observed
}

@id("vec.for-each.bool")
fn bool_oracle() -> i64 {
    let bool_base = vec_with_capacity<bool>(1usize);
    let bools = vec_push<bool>(bool_base, true);
    let mut observed = 9;
    for item in bools { observed = if item { 7 } else { 9 }; 0 }
    observed
}

@id("vec.for-each")
fn main() -> i64 {
    if empty_oracle() == 7
        && one_oracle() == 221
        && many_oracle() == 209563
        && full_oracle() == 213535
        && i32_oracle() == 1
        && u8_oracle() == 2
        && usize_oracle() == 3
        && char_oracle() == 4
        && f32_oracle() == 5
        && f64_oracle() == 6
        && bool_oracle() == 7
    { 7 } else { 1 }
}
"#;

const FOR_EACH_PARTIAL_FAILURE: &str = r#"
module test.owned_vec_for_each_failure;

@id("vec.for-each.accept")
fn accept(value: i64) -> i64
    requires value != 2
{
    value
}

@id("vec.for-each.failure")
fn main() -> i64 {
    let base = vec_with_capacity<i64>(3usize);
    let first = vec_push<i64>(base, 1);
    let second = vec_push<i64>(first, 2);
    let values = vec_push<i64>(second, 3);
    let mut rolling = 0;
    for item in values { rolling = rolling * 31 + accept(item); 0 }
    rolling
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
    assert!(!generated.contains("*result = *source"));
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
  spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')},
  spx_vec_reserve_exact:(source,tag,additional)=>{const old=read(source,tag),n=Number(additional),capacity=Math.max(old.capacity,old.values.length+n);if(!Number.isSafeInteger(n)||n<0||capacity>8192)return 0n;entries.delete(key(source));return alloc(tag,capacity,old.values.slice())},
  spx_vec_set:(source,tag,index,bits)=>{const old=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=old.values.length)return 0n;const values=old.values.slice();values[n]=bits;entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_clear:(source,tag)=>{const old=read(source,tag);entries.delete(key(source));return alloc(tag,old.capacity,[])}
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
        ("reserve", RESERVE_FAILURE, 3_u64, 15_i32),
        ("set", SET_FAILURE, 2_u64, 14_i32),
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
                    "allocation" => "vec.allocation",
                    "reserve" => "vec.reserve-failure",
                    _ => "vec.set-failure",
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
                    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
                    assert_eq!(
                        String::from_utf8_lossy(&output.stderr),
                        format!("SEMAPRAX operation failure: semaprax.vec.v1/{code}{newline}")
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
env.spx_vec_reserve_exact=(v,t,a)=>{const e=read(v,t),n=Number(a),capacity=Math.max(e.capacity,e.values.length+n);if(!Number.isSafeInteger(n)||n<0||capacity>8192)return 0n;entries.delete(key(v));return alloc(t,capacity,e.values.slice())};
env.spx_vec_set=(v,t,i,b)=>{const e=read(v,t),n=Number(i);if(!Number.isSafeInteger(n)||n<0||n>=e.values.length)return 0n;const values=e.values.slice();values[n]=b;entries.delete(key(v));return alloc(t,e.capacity,values)};
env.spx_vec_clear=(v,t)=>{const e=read(v,t);entries.delete(key(v));return alloc(t,e.capacity,[])};
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
    } else if (strcmp(argv[1], "reserve-stale") == 0) {
        spx_vec_v1 stale = value; spx_vec_v1 moved = spx_vec_move(&ctx, &value), result = {0};
        (void)moved; (void)spx_vec_reserve_exact(&ctx, UINT32_C(1), &stale, UINT64_C(1), &result);
    } else if (strcmp(argv[1], "set-tag") == 0) {
        spx_vec_v1 result = {0}; value.type_tag = UINT32_C(2);
        (void)spx_vec_set(&ctx, UINT32_C(1), &value, UINT64_C(0), UINT64_C(9), &result);
    } else if (strcmp(argv[1], "clear-generation") == 0) {
        spx_vec_v1 result = {0}; value.generation += UINT64_C(1);
        (void)spx_vec_clear(&ctx, UINT32_C(1), &value, &result);
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
        for attack in [
            "stale",
            "pointer-read",
            "pointer-free",
            "tag",
            "generation",
            "reserve-stale",
            "set-tag",
            "clear-generation",
        ] {
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
fn reserve_exact_native_realloc_failure_preserves_owner_until_single_settlement() {
    assert!(Command::new("clang").arg("--version").output().is_ok());
    let ast = parse(SOURCE, "owned-vec-native-realloc-failure.spx").unwrap();
    let generated = codegen::emit_c(&ast).unwrap();
    let prefix = r#"
#include <stdlib.h>
static unsigned spx_test_realloc_count;
static unsigned spx_test_free_count;
static void *spx_test_realloc(void *ptr, size_t size) {
    (void)ptr; (void)size; spx_test_realloc_count += 1U; return NULL;
}
static void spx_test_free(void *ptr) {
    spx_test_free_count += 1U; free(ptr);
}
#define SPX_VEC_REALLOC spx_test_realloc
#define free spx_test_free
#define main spx_generated_main
"#;
    let probe = r#"
#undef main
#undef free
int main(void) {
    struct spx_status_entry entries[UINT32_C(4)]; struct spx_context ctx = {0};
    if (!spx_context_init(&ctx, UINT64_C(91), entries, UINT32_C(4), NULL, NULL, NULL)) return 2;
    spx_vec_v1 empty = {0}, value = {0}, result = {0};
    if (spx_vec_with_capacity(&ctx, UINT32_C(1), UINT64_C(1), &empty) != SPX_STATUS_SUCCESS) return 3;
    if (spx_vec_push(&ctx, UINT32_C(1), &empty, UINT64_C(7), &value) != SPX_STATUS_SUCCESS) return 4;
    uint64_t *ptr = value.ptr; uint64_t generation = value.generation;
    uint32_t authority = value.authority;
    if (spx_vec_reserve_exact(&ctx, UINT32_C(1), &value, UINT64_C(1), &result) == SPX_STATUS_SUCCESS) return 5;
    if (spx_test_realloc_count != 1U || spx_test_free_count != 0U) return 6;
    if (value.ptr != ptr || value.len != UINT64_C(1) || value.capacity != UINT64_C(1)
        || value.generation != generation || value.authority != authority || value.type_tag != UINT32_C(1)) return 7;
    if (result.ptr != NULL || result.len != UINT64_C(0) || result.capacity != UINT64_C(0)
        || result.generation != UINT64_C(0) || result.authority != UINT32_C(0)
        || result.type_tag != UINT32_C(0)) return 8;
    struct spx_vec_authority_entry *entry = &ctx.vec_authority[authority - UINT32_C(1)];
    if (!entry->live || entry->ptr != ptr || entry->generation != generation) return 9;
    spx_vec_drop(&ctx, &value);
    if (spx_test_free_count != 1U || entry->live || value.authority != UINT32_C(0)) return 10;
    return 0;
}
"#;
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-vec-native-realloc-failure-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let c_path = root.join("probe.c");
    std::fs::write(&c_path, format!("{prefix}\n{generated}\n{probe}")).unwrap();
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
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization}: exit={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nested_if_vec_routes_complete_core_wasm_imports_and_executes() {
    assert!(Command::new("node").arg("--version").output().is_ok());
    let ast = parse(NESTED_IF, "owned-vec-nested-if.spx").unwrap();
    let c = codegen::emit_c(&ast).unwrap();
    for extended in ["spx_vec_reserve_exact", "spx_vec_set", "spx_vec_clear"] {
        assert!(
            !c.contains(extended),
            "legacy Vec source unexpectedly emits native v3 helper {extended}"
        );
    }
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
for(const name of ['spx_vec_reserve_exact','spx_vec_set','spx_vec_clear'])if(names.includes(name))throw Error(`unexpected-v3:${name}`);
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

/// The loop-carried accumulate-and-filter shape, carried through every engine.
///
/// The loop-carried profile is the only shape in which a program can hold a
/// *variable* number of scalar values, so this is the regression that proves
/// the profile is more than a spec sentence. Before the cleanup-plan storage
/// ownership rule was corrected, `values = vec_push<T>(values, value)` inside a
/// bounded `while` re-homed the binding's slot into the loop body's own cleanup
/// region: the body's scope exit finalized the vector every iteration, one
/// linearized pass no longer preserved owned liveness, and the whole shape
/// fail-closed with `SPX-H006`. `examples/vector-stats-project` is the same
/// shape as a whole project.
#[test]
fn loop_carried_accumulate_and_filter_runs_on_every_engine() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-vec-loop-carried-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("program.spx");
    std::fs::write(&path, LOOP_CARRIED).unwrap();
    let ast = parse(LOOP_CARRIED, &path).unwrap();

    // The accumulated element count follows the argument rather than a fixed
    // unrolled sequence: with the filter disabled these are the exact running
    // prefix sums of `(index * 37 + 11) % 100`.
    for (count, expected) in [
        (0, 0),
        (1, 11),
        (2, 59),
        (3, 144),
        (4, 166),
        (9, 431),
        (12, 574),
    ] {
        let outcome = interpreter::interpret(
            &path,
            "vec.alert-total",
            &[count.to_string(), "0".to_owned()],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "count {count} did not accumulate {expected}: {}",
            outcome.envelope
        );
    }
    // The filter is a real predicate over the accumulated values, not a
    // constant: the same nine readings sum to 431, 310 and 0 under three
    // thresholds.
    for (threshold, expected) in [(0, 431), (50, 310), (97, 0)] {
        let outcome = interpreter::interpret(
            &path,
            "vec.alert-total",
            &["9".to_owned(), threshold.to_string()],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "threshold {threshold} did not filter to {expected}: {}",
            outcome.envelope
        );
    }

    let interpreted = interpreter::interpret(
        &path,
        "vec.loop-carried",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(interpreted.envelope.contains("\"value\":\"310\""));

    let generated = codegen::emit_c(&ast).unwrap();
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
        assert!(output.status.success(), "{optimization} native execution");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "310");
    }

    if Command::new("node").arg("--version").output().is_ok() {
        let wasm_path = root.join("program.wasm");
        std::fs::write(&wasm_path, wasm::emit_module(&ast).unwrap()).unwrap();
        // One host arena that keeps exactly one live carrier per generation:
        // a leaked or double-owned vector fails rather than passing quietly.
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
  for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==310n||entries.size!==0)throw Error(`semantic-or-settlement:${value}:${entries.size}`)}
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
fn for_each_copy_scalars_preserves_order_settlement_and_reentry_on_every_engine() {
    const EXPECTED: i64 = 7;
    let success_ast = parse(FOR_EACH_COPY_SCALARS, "owned-vec-for-each.spx").unwrap();
    let success_hir = hir::resolve(&success_ast).unwrap();
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
        let facts = success_hir
            .declarations
            .type_facts(&hir::ResolvedType::Nominal {
                declaration: hir::DeclarationId::new("core.vec"),
                arguments: vec![element],
            })
            .unwrap();
        assert!(facts.sized && facts.needs_drop && !facts.copy);
    }

    let failure_ast = parse(FOR_EACH_PARTIAL_FAILURE, "owned-vec-for-each-failure.spx").unwrap();
    hir::resolve(&failure_ast).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-vec-for-each-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let success_source = root.join("success.spx");
    let failure_source = root.join("failure.spx");
    std::fs::write(&success_source, FOR_EACH_COPY_SCALARS).unwrap();
    std::fs::write(&failure_source, FOR_EACH_PARTIAL_FAILURE).unwrap();

    for _ in 0..4 {
        let outcome = interpreter::interpret(
            &success_source,
            "vec.for-each",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(outcome.returned);
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{EXPECTED}\"")),
            "{}",
            outcome.envelope
        );

        let failure = interpreter::interpret(
            &failure_source,
            "vec.for-each.failure",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!failure.returned);
        let envelope: serde_json::Value = serde_json::from_str(&failure.envelope).unwrap();
        assert_eq!(
            envelope["payload"]["outcome"]["status"]["domain_id"],
            "semaprax.contract.v1"
        );
        assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 1);
    }

    let success_c = codegen::emit_c(&success_ast).unwrap();
    let failure_c = codegen::emit_c(&failure_ast).unwrap();
    for generated in [&success_c, &failure_c] {
        assert!(!generated.contains("memcpy(result, source"));
        assert!(!generated.contains("*result = *source"));
        assert!(!generated.contains("spx_vec_v1 moved = *source;\n    *result = moved"));
    }
    let success_c_path = root.join("success.c");
    let failure_c_path = root.join("failure.c");
    std::fs::write(&success_c_path, &success_c).unwrap();
    std::fs::write(&failure_c_path, &failure_c).unwrap();
    let tracked_failure_c = failure_c
        .replace("calloc(", "spx_test_calloc(")
        .replace("free(payload);", "spx_test_free(payload);");
    let allocator_probe = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_calloc(size_t count, size_t size) {
  void *allocation = calloc(count, size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
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
    let failure_probe = r#"
int main(void) {
  struct spx_status_entry entries[UINT32_C(8)];
  struct spx_context context = {0};
  if (!spx_context_init(&context, UINT64_C(17), entries, UINT32_C(8), NULL, NULL, NULL)) return 1;
  for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {
    int64_t result = INT64_C(0x2525252525252525);
    uint32_t before = context.status_arena.length;
    spx_status_token status = spx_decl_7665632e666f722d656163682e6661696c757265(&context, &result);
    const struct spx_normalized_status *resolved = spx_status_resolve(&context, status);
    if (status == SPX_STATUS_SUCCESS || resolved == NULL ||
        strcmp(resolved->domain_id, "semaprax.contract.v1") != 0 ||
        resolved->code != UINT32_C(1) ||
        result != INT64_C(0x2525252525252525)) return 2;
    if (context.status_arena.length != before + UINT32_C(1) ||
        context.call_depth != UINT32_C(0) ||
        spx_test_live_allocations != UINT64_C(0)) return 3;
    for (uint32_t slot = 0; slot < SPX_VEC_AUTHORITY_CAPACITY; ++slot) {
      const struct spx_vec_authority_entry *entry = &context.vec_authority[slot];
      if (entry->ptr != NULL || entry->len != UINT64_C(0) ||
          entry->capacity != UINT64_C(0) || entry->generation != UINT64_C(0) ||
          entry->type_tag != UINT32_C(0) || entry->live) return 4;
    }
  }
  return 0;
}
"#;
    assert!(Command::new("clang").arg("--version").output().is_ok());
    for optimization in ["-O0", "-O2"] {
        let success_binary = root.join(format!("success-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&success_c_path)
            .arg("-o")
            .arg(&success_binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "success/{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        for _ in 0..4 {
            let output = Command::new(&success_binary).output().unwrap();
            assert!(output.status.success(), "success/{optimization}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                EXPECTED.to_string()
            );
        }

        let failure_binary = root.join(format!("failure-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&failure_c_path)
            .arg("-o")
            .arg(&failure_binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "failure/{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        for _ in 0..4 {
            let output = Command::new(&failure_binary).output().unwrap();
            assert_eq!(output.status.code(), Some(70), "failure/{optimization}");
            let newline = if cfg!(windows) { "\r\n" } else { "\n" };
            assert_eq!(
                String::from_utf8_lossy(&output.stderr),
                format!(
                    "SEMAPRAX contract failure{newline}  contract: requires value != 2 in vec.for-each.accept{newline}  arguments: value = 2{newline}"
                )
            );
        }

        let probe_source = root.join(format!("failure-probe-{optimization}.c"));
        let probe_binary = root.join(format!("failure-probe-{optimization}"));
        std::fs::write(
            &probe_source,
            format!("{allocator_probe}\n{tracked_failure_c}\n{failure_probe}"),
        )
        .unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&probe_source)
            .arg("-o")
            .arg(&probe_binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "failure probe/{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&probe_binary).output().unwrap();
        assert!(
            output.status.success(),
            "failure probe/{optimization}: exit {:?}",
            output.status.code()
        );
    }

    let success_wasm = wasm::emit_module(&success_ast).unwrap();
    let failure_wasm = wasm::emit_module(&failure_ast).unwrap();
    for module in [&success_wasm, &failure_wasm] {
        for payload in wasmparser::Parser::new(0).parse_all(module) {
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
    }
    if Command::new("node").arg("--version").output().is_ok() {
        let success_wasm_path = root.join("success.wasm");
        let failure_wasm_path = root.join("failure.wasm");
        std::fs::write(&success_wasm_path, success_wasm).unwrap();
        std::fs::write(&failure_wasm_path, failure_wasm).unwrap();
        let script = r#"
const fs=require('fs');
const bytes=fs.readFileSync(process.argv[1]);
const mode=process.argv[2];
const expected=7n;
let next=1n;
const entries=new Map();
const key=value=>{if(typeof value!=='bigint'||value===0n)throw Error('carrier');return value.toString()};
const read=(value,tag)=>{const entry=entries.get(key(value));if(!entry||entry.tag!==tag)throw Error('stale-or-type');return entry};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={
  spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
  spx_contract_fail:selector=>{if(selector!==9)throw Error(`selector:${selector}`);throw Object.assign(Error('SEMAPRAX contract failure'),{domain_id:'semaprax.contract.v1',code:1})},
  spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?alloc(tag,n):0n},
  spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;const values=old.values.concat([bits]);entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),
  spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),
  spx_vec_get:(source,tag,index)=>{const entry=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=entry.values.length)throw Error('oob');return entry.values[n]},
  spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')}
};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{
  for(let i=0;i<4;i+=1){
    if(mode==='success'){
      const value=instance.exports.semaprax_main();
      if(value!==expected)throw Error(`order:${value}`);
    }else{
      let failed=false;
      try{instance.exports.semaprax_main()}catch(error){
        if(error.domain_id!=='semaprax.contract.v1'||error.code!==1||error.message!=='SEMAPRAX contract failure')throw error;
        failed=true;
      }
      if(!failed)throw Error('missing failure');
    }
    if(entries.size!==0)throw Error(`retained:${entries.size}`);
  }
}).catch(error=>{console.error(error);process.exit(2)});
"#;
        for (mode, path) in [
            ("success", &success_wasm_path),
            ("failure", &failure_wasm_path),
        ] {
            let output = Command::new("node")
                .arg("-e")
                .arg(script)
                .arg(path)
                .arg(mode)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode} Core-Wasm: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}
