use std::process::Command;

use semaprax::{codegen, hir, interpreter, parse, wasm};

const SOURCE: &str = r#"
module test.owned_box_runtime;

@id("box.main")
fn main() -> i64 {
    let i64_box = box_new<i64>(11);
    let i32_box = box_new<i32>(12i32);
    let u8_box = box_new<u8>(13u8);
    let usize_box = box_new<usize>(14usize);
    let char_box = box_new<char>('A');
    let f32_box = box_new<f32>(1.5f32);
    let f64_box = box_new<f64>(2.5);
    let bool_box = box_new<bool>(true);
    let lexical_drop = box_new<i64>(99);
    let lexical_seen = box_get<i64>(lexical_drop);
    let borrowed = box_get<i64>(i64_box);
    let inner = box_into_inner<i64>(i64_box);
    if borrowed == 11
        && inner == 11
        && box_into_inner<i32>(i32_box) == 12i32
        && box_into_inner<u8>(u8_box) == 13u8
        && box_into_inner<usize>(usize_box) == 14usize
        && box_into_inner<char>(char_box) == 'A'
        && box_into_inner<f32>(f32_box) == 1.5f32
        && box_into_inner<f64>(f64_box) == 2.5
        && box_into_inner<bool>(bool_box)
        && lexical_seen == 99
    { 7 } else { 1 }
}
"#;

#[test]
fn owned_bounded_box_copy_scalars_interpret_with_exact_cleanup() {
    let parsed = parse(SOURCE, "owned-bounded-box-runtime.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
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
                declaration: hir::DeclarationId::new("core.box"),
                arguments: vec![element],
            })
            .unwrap();
        assert!(facts.sized && facts.needs_drop && !facts.copy);
    }
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "box.main")
        .unwrap();
    let box_leaves = function.cleanup_plan.slots.iter().filter(|slot| {
        matches!(&slot.field_liveness_shape, semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } if lifecycle.as_str() == "core.box.drop")
    }).count();
    // Nine lexical owners, nine allocation temporaries, and eight consuming
    // call epochs each carry one independently replayed Box finalizer. The
    // ninth owner is intentionally settled by lexical cleanup.
    assert_eq!(box_leaves, 26);

    let root =
        std::env::temp_dir().join(format!("semaprax-owned-bounded-box-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("program.spx");
    std::fs::write(&path, SOURCE).unwrap();
    for _ in 0..4 {
        let outcome = interpreter::interpret(
            &path,
            "box.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(outcome.returned);
        assert!(outcome.envelope.contains("\"value\":\"7\""));
    }

    let generated = codegen::emit_c(&parsed).unwrap();
    assert!(generated.contains("struct spx_box_authority_entry"));
    assert!(generated.contains("spx_box_into_inner"));
    assert!(!generated.contains("memcpy(result, source"));
    assert!(!generated.contains("*result = *source"));
    let c_path = root.join("program.c");
    std::fs::write(&c_path, generated).unwrap();
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
        for _ in 0..4 {
            let output = Command::new(&binary).output().unwrap();
            assert!(
                output.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
        }
    }

    let core_wasm = wasm::emit_module(&parsed).unwrap();
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
    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, core_wasm).unwrap();
    assert!(Command::new("node").arg("--version").output().is_ok());
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]);
let next=1n;const entries=new Map(),key=v=>{if(typeof v!=='bigint'||v===0n)throw Error('carrier');return v.toString()};
const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('stale-or-type');return e};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:s=>{throw Error(`unexpected-status:${s}`)},
spx_box_new:(tag,bits)=>{if(entries.size>=4096)return 0n;const token=next++;entries.set(key(token),{tag,bits});return token},
spx_box_get:(v,t)=>read(v,t).bits,
spx_box_into_inner:(v,t)=>{const e=read(v,t);entries.delete(key(v));return e.bits},
spx_box_drop:v=>{if(!entries.delete(key(v)))throw Error('double-drop')}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==7n||entries.size!==0)throw Error(`semantic-or-settlement:${value}:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
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
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owned_bounded_box_allocation_failure_is_exact_and_reentrant() {
    let parsed = parse(SOURCE, "owned-bounded-box-allocation.spx").unwrap();
    let generated = codegen::emit_c(&parsed).unwrap();
    let injected = generated.replacen(
        "uint64_t *payload = (uint64_t *)malloc(sizeof(uint64_t));",
        "uint64_t *payload = NULL;",
        1,
    );
    assert_ne!(generated, injected);
    let root = std::env::temp_dir().join(format!("semaprax-box-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let c_path = root.join("program.c");
    std::fs::write(&c_path, injected).unwrap();
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
        for _ in 0..4 {
            let output = Command::new(&binary).output().unwrap();
            assert_eq!(output.status.code(), Some(73));
            let newline = if cfg!(windows) { "\r\n" } else { "\n" };
            assert_eq!(
                String::from_utf8_lossy(&output.stderr),
                format!("SEMAPRAX operation failure: semaprax.box.v1/1{newline}")
            );
        }
    }

    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, wasm::emit_module(&parsed).unwrap()).unwrap();
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]);let calls=0;
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:s=>{if(s!==17)throw Error(`selector:${s}`);throw Object.assign(Error('box failure'),{domain_id:'semaprax.box.v1',code:1})},
spx_box_new:()=>{calls+=1;return 0n},spx_box_get:()=>{throw Error('get-after-refusal')},spx_box_into_inner:()=>{throw Error('consume-after-refusal')},spx_box_drop:()=>{throw Error('drop-after-refusal')}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(e){if(e.domain_id!=='semaprax.box.v1'||e.code!==1)throw e;failed=true}if(!failed)throw Error('missing-failure')}if(calls!==4)throw Error(`calls:${calls}`)}).catch(e=>{console.error(e);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Core-Wasm refusal: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owned_bounded_box_native_rejects_stale_and_forged_carriers_before_access() {
    assert!(Command::new("clang").arg("--version").output().is_ok());
    let parsed = parse(SOURCE, "owned-box-native-hostile.spx").unwrap();
    let generated = codegen::emit_c(&parsed).unwrap();
    let probe = r#"
#undef main
int main(int argc, char **argv) {
    struct spx_status_entry entries[UINT32_C(4)]; struct spx_context ctx = {0};
    if (!spx_context_init(&ctx, UINT64_C(77), entries, UINT32_C(4), NULL, NULL, NULL)) return 2;
    spx_box_v1 value = {0};
    if (spx_box_new(&ctx, UINT32_C(1), UINT64_C(9), &value) != SPX_STATUS_SUCCESS) return 3;
    if (argc == 1) { spx_box_drop(&ctx, &value); return 0; }
    if (strcmp(argv[1], "stale") == 0) {
        spx_box_v1 stale = value; spx_box_v1 moved = spx_box_move(&ctx, &value); (void)moved;
        (void)spx_box_get(&ctx, &stale, UINT32_C(1));
    } else if (strcmp(argv[1], "pointer-read") == 0) {
        value.ptr = (uint64_t *)(uintptr_t)UINT64_C(1); (void)spx_box_get(&ctx, &value, UINT32_C(1));
    } else if (strcmp(argv[1], "pointer-free") == 0) {
        value.ptr = (uint64_t *)(uintptr_t)UINT64_C(1); spx_box_drop(&ctx, &value);
    } else if (strcmp(argv[1], "tag") == 0) {
        value.type_tag = UINT32_C(2); (void)spx_box_get(&ctx, &value, UINT32_C(1));
    } else if (strcmp(argv[1], "generation") == 0) {
        value.generation += UINT64_C(1); (void)spx_box_into_inner(&ctx, &value, UINT32_C(1));
    } else return 4;
    return 5;
}
"#;
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-box-native-hostile-{}",
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
