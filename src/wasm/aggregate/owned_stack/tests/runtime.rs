//! Real-engine evidence for nested private-frame/result-pointer isolation.
//! Authored separately from the arithmetic tests; Node absence is a failure.

use super::super::derive;
use crate::project::{
    derive_public_api_descriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use crate::variant_layout::{VariantLayoutCache, VariantTarget};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[test]
fn nested_helper_result_slots_never_alias_public_failure_storage() {
    let source = r#"module test.owned_stack;
@id("stack.leaf")
fn leaf(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }
@id("stack.middle")
fn middle(input: borrow Slice<u8>) -> Bytes { leaf(input) }
@id("stack.bytes")
fn bytes(input: borrow Slice<u8>, zero: i64) -> Bytes {
    let staged = middle(input);
    let ignored = 1 / zero;
    staged
}
@id("stack.option")
fn optional(input: borrow Slice<u8>, zero: i64) -> Option<Bytes> {
    let staged = middle(input);
    let ignored = 1 / zero;
    Option<Bytes>::Some { value: staged }
}
@id("stack.main") fn main() -> i64 { 0 }
"#;
    let program = crate::hir::resolve(&crate::check(source, "owned-stack.spx").unwrap()).unwrap();
    let revision = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let subject = PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: revision,
        workspace_revision: revision,
        project_graph_digest: revision,
    };
    let descriptor = derive_public_api_descriptor(
        &program,
        &["stack.bytes".to_owned(), "stack.option".to_owned()],
        subject,
    )
    .unwrap();
    let extents = derive(
        &program,
        &VariantLayoutCache::build(&program, VariantTarget::Wasm32).unwrap(),
        &[
            crate::hir::DeclarationId::new("stack.bytes"),
            crate::hir::DeclarationId::new("stack.option"),
        ],
    )
    .unwrap();
    let floors = [
        131_072 - 8 - extents[&crate::hir::DeclarationId::new("stack.bytes")],
        131_072 - 16 - extents[&crate::hir::DeclarationId::new("stack.option")],
    ];
    assert!(
        floors[0] <= 131_048,
        "fixture must actually stage nested call outputs"
    );
    let wasm =
        crate::wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-owned-stack-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("app.wasm"), wasm).unwrap();
    std::fs::write(directory.join("probe.mjs"), r#"import { readFile } from 'node:fs/promises';
const wasm = await readFile(new URL('./app.wasm', import.meta.url));
let instance, copies = 0, drops = 0, next = 1;
const owners = new Map();
const env = {
  spx_add: (a,b)=>a+b, spx_sub: (a,b)=>a-b, spx_mul: (a,b)=>a*b,
  spx_div: (a,b)=>{ if(b===0n)throw Error('missing semantic zero guard'); return a/b; },
  spx_rem: ()=>{ throw Error('unexpected remainder'); }, spx_neg: a=>-a,
  spx_contract_fail: ()=>{ throw Error('unexpected contract'); },
  spx_bytes_copy: carrier=>{
    const word=BigInt.asUintN(64,carrier), offset=Number(word>>32n), length=Number(word&0xffffffffn);
    const value=new Uint8Array(instance.exports.memory.buffer,offset,length).slice();
    const token=next++, result=BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(length));
    owners.set(result,value); copies++; return result;
  },
  spx_bytes_drop: carrier=>{if(!owners.delete(carrier))throw Error('duplicate drop');drops++;},
  spx_bytes_get: ()=>{throw Error('unexpected get');},
  spx_bytes_as_slice: carrier=>carrier,
  spx_owned_utf8_validate_v1: ()=>{throw Error('unexpected validation');}
};
({instance}=await WebAssembly.instantiate(wasm,{env}));
const memory=new Uint8Array(instance.exports.memory.buffer), view=new DataView(memory.buffer);
const symbol=id=>'spx_owned_v1_'+Array.from(new TextEncoder().encode(id),b=>b.toString(16).padStart(2,'0')).join('');
const fixtures=[['stack.bytes',8,Number(process.argv[2])],['stack.option',16,Number(process.argv[3])]];
for(const [id,size,floor] of fixtures){
  const call=instance.exports[symbol(id)];
  // Complete private range, including helper frames, and low partial overlap.
  const pointers=new Set([floor,131072-size]);
  if(size>8)pointers.add(floor-8);
  for(let pointer=floor;pointer<=131072-size;pointer+=8)pointers.add(pointer);
  for(const pointer of pointers){
    memory.fill(0x3c,pointer,pointer+size);
    const before=memory.slice(pointer,pointer+size), count=copies;
    if(call(0,0,0n,pointer)!==11)throw Error(`private alias admitted ${id}:${pointer}`);
    if(memory.slice(pointer,pointer+size).some((b,i)=>b!==before[i])||copies!==count||owners.size)throw Error('rejection performed action');
  }
  // Exact low disjoint boundary must remain valid, even on semantic failure.
  for(const pointer of [floor-size,65536]){
    memory.set([7,0,255],0); memory.fill(0xa5,pointer,pointer+size);
    const before=memory.slice(pointer,pointer+size), copyBefore=copies, dropBefore=drops;
    if(call(0,3,0n,pointer)!==4)throw Error('sticky semantic status');
    if(memory.slice(pointer,pointer+size).some((b,i)=>b!==before[i]))throw Error('failed helper call wrote result');
    if(copies!==copyBefore+1||drops!==dropBefore+1||owners.size)throw Error('failed helper did not settle exactly once');
    if(call(0,3,1n,pointer)!==0)throw Error('valid call after rejection');
    if(size===16&&view.getUint32(pointer,true)!==1)throw Error('Some tag');
    const carrier=view.getBigInt64(pointer+(size===16?8:0),true), value=owners.get(carrier);
    if(!value||value.length!==3||value[0]!==7||value[2]!==255)throw Error('success bytes');
    env.spx_bytes_drop(carrier);
    if(owners.size)throw Error('success did not settle');
  }
}
"#).unwrap();
    let output = Command::new("node")
        .arg(directory.join("probe.mjs"))
        .args(floors.map(|floor| floor.to_string()))
        .output()
        .expect("Node is required for owned-stack evidence");
    for file in ["app.wasm", "probe.mjs"] {
        std::fs::remove_file(directory.join(file)).unwrap();
    }
    std::fs::remove_dir(directory).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
