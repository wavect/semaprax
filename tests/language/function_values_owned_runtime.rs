//! Function values remain private while an owned Bytes export exercises the
//! aggregate status/cleanup ABI around `call_indirect`.

use semaprax::project::{
    derive_public_api_descriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use std::process::Command;

use semaprax::{codegen, hir, wasm};

const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

const SOURCE: &str = r#"
module test.function_values_owned_runtime;
@id("owned.first") fn first(left: usize, right: usize) -> usize { left }
@id("owned.package") fn package(value: borrow Slice<u8>) -> Bytes {
    let callback = first;
    let mut count = byte_len(value);
    let retained = callback(count, { count = 9usize; count });
    let fallback = [9u8];
    if retained == byte_len(value) { bytes_copy(value) } else { bytes_copy(array_as_slice(fallback)) }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    }
}

#[test]
fn private_callback_inside_owned_export_has_aggregate_table_and_native_cleanup_lowering() {
    let checked = semaprax::check(SOURCE, "function-values-owned-runtime.spx").unwrap();
    let program = hir::resolve(&checked).unwrap();
    let descriptor =
        derive_public_api_descriptor(&program, &["owned.package".to_owned()], subject()).unwrap();
    let module = wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor).unwrap();
    assert_eq!(
        module,
        wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor).unwrap()
    );
    wasmparser::Validator::new().validate_all(&module).unwrap();
    let mut tables = 0usize;
    let mut indirect = 0usize;
    for payload in wasmparser::Parser::new(0).parse_all(&module) {
        match payload.unwrap() {
            wasmparser::Payload::TableSection(section) => tables += section.count() as usize,
            wasmparser::Payload::CodeSectionEntry(body) => {
                for operator in body.get_operators_reader().unwrap() {
                    if matches!(operator.unwrap(), wasmparser::Operator::CallIndirect { .. }) {
                        indirect += 1;
                    }
                }
            }
            _ => {}
        }
    }
    assert_eq!(tables, 1, "owned aggregate callback needs one active table");
    assert!(indirect > 0, "owned aggregate callback needs call_indirect");
    assert!(codegen::emit_c(&checked).is_ok());
    if super::required_or_available("node", "SPX_REQUIRE_NODE") {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-function-values-owned-{}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("app.wasm"), &module).unwrap();
        std::fs::write(directory.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
const wasm=await readFile(new URL('./app.wasm',import.meta.url));let instance,next=1;const owners=new Map();
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('contract')},spx_bytes_copy:c=>{const w=BigInt.asUintN(64,c),o=Number(w>>32n),n=Number(w&0xffffffffn),v=new Uint8Array(instance.exports.memory.buffer,o,n).slice(),h=BigInt.asIntN(64,((0x80000000n|BigInt(next++))<<32n)|BigInt(n));owners.set(h,v);return h},spx_bytes_drop:c=>{if(!owners.delete(c))throw Error('drop')},spx_bytes_get:()=>-1,spx_bytes_as_slice:c=>c,spx_owned_utf8_validate_v1:()=>{throw Error('unexpected utf8 validation')}};
({instance}=await WebAssembly.instantiate(wasm,{env}));const symbol='spx_owned_v1_'+Array.from(new TextEncoder().encode('owned.package'),b=>b.toString(16).padStart(2,'0')).join('');new Uint8Array(instance.exports.memory.buffer).set([7,8],0);if(instance.exports[symbol](0,2,65536)!==0)throw Error('status');const c=new DataView(instance.exports.memory.buffer).getBigInt64(65536,true),v=owners.get(c);if(!v||v.length!==2||v[0]!==7||v[1]!==8)throw Error('left-to-right');env.spx_bytes_drop(c);if(owners.size)throw Error('leak');"#).unwrap();
        let output = Command::new("node")
            .arg("probe.mjs")
            .current_dir(&directory)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
