use std::process::Command;

use semaprax::{hir, interpreter, parse, wasm};
#[path = "owned_box_bytes_runtime/native.rs"]
mod native;

const SOURCE: &str = r#"
module test.owned_box_bytes_runtime;

@id("app.main")
fn main() -> i64 {
    let input = [65u8, 66u8, 67u8];
    let boxed = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    let inner = box_into_inner<Bytes>(boxed);
    let lexical = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    if byte_len(bytes_as_slice(inner)) == 3usize { 29 } else { 1 }
}
"#;

const REJECT_GET: &str = r#"
module test.owned_box_bytes_reject;
@id("box.bytes.reject") fn main() -> i64 {
    let input = [1u8];
    let boxed = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    box_get<Bytes>(boxed)
}
"#;

const CONTRACT_FAILURE: &str = r#"
module test.owned_box_bytes_contract;
@id("box.bytes.main") fn main() -> i64 { after() }
@id("box.bytes.before") fn before() -> i64 requires false { 0 }
@id("box.bytes.after") fn after() -> i64 ensures false {
    let input = [1u8];
    let boxed = box_new<Bytes>(bytes_copy(array_as_slice(input)));
    let extracted = box_into_inner<Bytes>(boxed);
    0
}
"#;

#[test]
fn owned_box_bytes_borrowed_get_is_rejected_at_source_admission() {
    let errors = semaprax::check(REJECT_GET, "owned-box-bytes-reject.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T285"),
        "{errors:?}"
    );
}

#[test]
fn owned_box_bytes_contract_failures_keep_cleanup_in_the_execution_plan() {
    let parsed = parse(CONTRACT_FAILURE, "owned-box-bytes-contract.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    for function_id in ["box.bytes.before", "box.bytes.after"] {
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == function_id)
            .unwrap();
        assert!(function.cleanup_plan.slots.iter().any(|slot| matches!(&slot.field_liveness_shape, semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } if lifecycle.as_str() == "core.box.drop")) || function_id.ends_with("before"));
    }
    let path = std::env::temp_dir().join(format!("box-bytes-contract-{}.spx", std::process::id()));
    std::fs::write(&path, CONTRACT_FAILURE).unwrap();
    for (id, code) in [("box.bytes.before", 1), ("box.bytes.after", 2)] {
        for _ in 0..4 {
            let result =
                interpreter::interpret(&path, id, &[], &interpreter::InterpreterOptions::default())
                    .unwrap();
            assert!(!result.returned);
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(
                envelope["payload"]["outcome"]["status"]["domain_id"],
                "semaprax.contract.v1"
            );
            assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn owned_box_bytes_interpreter_has_consuming_and_lexical_cleanup() {
    let parsed = semaprax::check(SOURCE, "owned-box-bytes-runtime.spx").unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let drops = function.cleanup_plan.slots.iter().filter(|slot| {
        matches!(&slot.field_liveness_shape, semaprax::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } if lifecycle.as_str() == "core.box.drop")
    }).count();
    assert!(
        drops >= 2,
        "consuming and lexical Box<Bytes> owners need cleanup"
    );
    let facts = resolved
        .declarations
        .type_facts(&hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("core.box"),
            arguments: vec![hir::ResolvedType::Bytes],
        })
        .unwrap();
    assert!(facts.sized && facts.needs_drop && !facts.copy);
    let root =
        std::env::temp_dir().join(format!("semaprax-owned-box-bytes-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("program.spx");
    std::fs::write(&path, SOURCE).unwrap();
    for _ in 0..4 {
        let result = interpreter::interpret(
            &path,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(result.returned);
        assert!(result.envelope.contains("\"value\":\"29\""));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owned_box_bytes_native_repeats_at_o0_and_o2() {
    native::run_native(SOURCE, 0, 29, false);
    native::run_native(SOURCE, 1, 0, true);
    let failure = SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 ensures false {");
    native::run_native(&failure, 2, 0, false);
    let before = SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 requires false {");
    native::run_native(&before, 1, 0, false);
}

#[test]
fn owned_box_bytes_wasm_host_transfers_and_detaches_inner_handle() {
    run_wasm(SOURCE, 0);
    run_wasm(
        &SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 ensures false {"),
        10,
    );
    run_wasm(
        &SOURCE.replace("fn main() -> i64 {", "fn main() -> i64 requires false {"),
        9,
    );
}

fn run_wasm(source: &str, expected: u32) {
    let parsed = parse(source, "owned-box-bytes-wasm.spx").unwrap();
    let bytes = wasm::emit_module(&parsed).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-box-bytes-wasm-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let wasm_path = root.join("program.wasm");
    std::fs::write(&wasm_path, bytes).unwrap();
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]);
let instance,next=1,nextBox=1n,refuse=false,copies=0;const data=new Map(),boxes=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {n:Number(w&0xffffffffn),r:Number(w>>32n)}};
const read=c=>{const {n,r}=decode(c);if(r&0x80000000){const a=data.get(r&0x7fffffff);if(!a||a.length!==n)throw Error('stale Bytes');return a;}const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(r>memory.buffer.byteLength-n)throw Error('range');return new Uint8Array(memory.buffer,r,n);};
const alloc=c=>{const a=new Uint8Array(read(c)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(a.length));};
const drop=c=>{read(c);const {r}=decode(c);if(!(r&0x80000000)||!data.delete(r&0x7fffffff))throw Error('double drop');};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:s=>{throw Error(`failure:${s}`)},
spx_bytes_copy:alloc,spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_as_slice:c=>{read(c);return c},spx_bytes_drop:drop,
spx_box_new_v2:(tag,bits)=>{if(tag!==9)throw Error(`tag:${tag}`);read(bits);if(refuse)return 0n;const h=nextBox++;boxes.set(h,bits);return h},
spx_box_get_v2:()=>{throw Error('owned payload copied')},
spx_box_into_inner_v2:(h,tag)=>{if(tag!==9||!boxes.has(h))throw Error('into-inner');const bits=boxes.get(h);boxes.delete(h);return bits},
spx_box_drop_v2:h=>{if(!boxes.has(h))throw Error('box double drop');const bits=boxes.get(h);boxes.delete(h);drop(bits)}};
(async()=>{
const legacy={...env};for(const name of Object.keys(legacy))if(name.endsWith('_v2'))delete legacy[name];
let rejected=false;try{await WebAssembly.instantiate(bytes,{env:legacy})}catch(e){if(!(e instanceof WebAssembly.LinkError))throw e;rejected=true}if(!rejected)throw Error('legacy host admitted');
({instance}=await WebAssembly.instantiate(bytes,{env}));
for(let i=0;i<4;i++){copies=0;let selector=0,value;try{value=instance.exports.semaprax_main()}catch(e){if(!e.message.startsWith('failure:'))throw e;selector=Number(e.message.slice(8))}if(selector!==expected||(!expected&&value!==29n)||copies!==(expected===9?0:2)||boxes.size||data.size)throw Error(`settlement:${selector}:${copies}:${boxes.size}:${data.size}`);}
if(expected)return;
refuse=true;
for(let i=0;i<4;i++){copies=0;let failed=false;try{instance.exports.semaprax_main()}catch(e){if(e.message!=='failure:17')throw e;failed=true}if(!failed||copies!==1||boxes.size||data.size)throw Error(`refusal settlement:${failed}:${copies}:${boxes.size}:${data.size}`);}
})().catch(e=>{console.error(e);process.exit(2)});
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&wasm_path)
        .arg(expected.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}
