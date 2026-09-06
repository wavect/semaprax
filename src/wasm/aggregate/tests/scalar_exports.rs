use std::path::Path;
use std::process::Command;

use crate::hir;
use crate::parse;

const SOURCE: &str = r#"
module wasm.aggregate_scalar_exports;

@id("aggregate.scalar.pair")
record Pair<T, U> {
    @id("aggregate.scalar.pair.left") left: T,
    @id("aggregate.scalar.pair.right") right: U,
}

@id("aggregate.scalar.evaluate")
fn evaluate() -> i64 {
    let input = [1u8, 2u8];
    let pair = Pair<Bytes, bool> {
        left: bytes_copy(array_as_slice(input)),
        right: true,
    };
    match own pair {
        Pair { left: payload, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize { 2 } else { 0 },
    }
}

@id("aggregate.scalar.boolean")
fn boolean(value: bool) -> bool { value }

@id("aggregate.scalar.failure")
fn failure() -> i64 requires false { 7 }

@id("aggregate.scalar.main") fn main() -> i64 { evaluate() }
"#;

#[test]
fn aggregate_lane_scalar_adapters_are_deterministic_bounded_and_status_checked() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(SOURCE, Path::new("aggregate-scalar-exports.spx")).unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let selected = [
        "aggregate.scalar.evaluate".to_owned(),
        "aggregate.scalar.boolean".to_owned(),
        "aggregate.scalar.failure".to_owned(),
    ];
    let bytes = crate::wasm::emit_resolved_module_with_scalar_exports(&program, &selected).unwrap();
    assert_eq!(
        bytes,
        crate::wasm::emit_resolved_module_with_scalar_exports(&program, &selected).unwrap()
    );
    wasmparser::Validator::new().validate_all(&bytes).unwrap();

    let root = std::env::temp_dir().join(format!(
        "semaprax-aggregate-scalar-exports-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let wasm = root.join("module.wasm");
    let script = root.join("run.mjs");
    std::fs::write(&wasm, bytes).unwrap();
    let evaluate = super::super::super::scalar_exports::raw_symbol("aggregate.scalar.evaluate");
    let boolean = super::super::super::scalar_exports::raw_symbol("aggregate.scalar.boolean");
    let failure = super::super::super::scalar_exports::raw_symbol("aggregate.scalar.failure");
    std::fs::write(
        &script,
        format!(
            r#"import {{readFile}} from "node:fs/promises";
const bytes=await readFile(process.argv[2]);
let instance=null,next=1;
const entries=new Map();
const read=carrier=>{{const word=BigInt.asUintN(64,carrier),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn);if((root&0x80000000)!==0){{const token=root&0x7fffffff,entry=entries.get(token);if(!entry||entry.length!==length)throw Error("stale");return entry;}}const memory=instance.exports.__spx_byte_memory;if(!memory||root>memory.buffer.byteLength-length)throw Error("range");return new Uint8Array(memory.buffer,root,length);}};
const allocate=value=>{{const token=next++,copy=new Uint8Array(value);entries.set(token,copy);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(copy.length));}};
const env={{spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("status");}},spx_bytes_copy:c=>allocate(read(c)),spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_drop:c=>{{const root=Number((BigInt.asUintN(64,c)>>32n)&0xffffffffn);entries.delete(root&0x7fffffff);}},spx_bytes_as_slice:c=>c}};
({{instance}}=await WebAssembly.instantiate(bytes,{{env}}));
if(instance.exports["{evaluate}"]()!==2n||entries.size!==0)throw Error("no-arg i64 adapter");
if(instance.exports["{boolean}"](1)!==1)throw Error("bool adapter");
let boundary=false;try{{instance.exports["{boolean}"](2);}}catch{{boundary=true;}}if(!boundary)throw Error("bool boundary");
let status=false;try{{instance.exports["{failure}"]();}}catch{{status=true;}}if(!status)throw Error("status boundary");
if(instance.exports["{evaluate}"]()!==2n||entries.size!==0)throw Error("reentry");
"#
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script)
        .arg(&wasm)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}
