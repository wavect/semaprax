//! Owned vector mutation must settle both the vector and staged payloads.
use semaprax::{interpreter, wasm};
use std::process::Command;

#[path = "owned_vec_bytes_runtime/native.rs"]
mod native;

const SUCCESS: &str = r#"module vec.bytes.runtime;
@id("app.main") fn main()->i64 {
 let input=[1u8,2u8,3u8];
 let empty=vec_with_capacity<Bytes>(1usize);
 let first=vec_push<Bytes>(empty,bytes_copy(array_as_slice(input)));
 let reserved=vec_reserve_exact<Bytes>(first,1usize);
 let replaced=vec_set<Bytes>(reserved,0usize,bytes_copy(array_as_slice(input)));
 let second=vec_push<Bytes>(replaced,bytes_copy(array_as_slice(input)));
 let observed=vec_len<Bytes>(second)==2usize && vec_capacity<Bytes>(second)==2usize;
 let cleared=vec_clear<Bytes>(second);
 let lexical=vec_push<Bytes>(cleared,bytes_copy(array_as_slice(input)));
 if observed && vec_len<Bytes>(lexical)==1usize && vec_capacity<Bytes>(lexical)==2usize {29}else{0}
}
"#;

const MUTABLE: &str = r#"module vec.bytes.mutable;
@id("app.main") fn main()->i64 {
 let input=[8u8];
 let mut values=vec_with_capacity<Bytes>(2usize);
 values=vec_push<Bytes>(values,bytes_copy(array_as_slice(input)));
 values=vec_set<Bytes>(values,0usize,bytes_copy(array_as_slice(input)));
 values=vec_reserve_exact<Bytes>(values,2usize);
 values=vec_clear<Bytes>(values);
 if vec_len<Bytes>(values)==0usize && vec_capacity<Bytes>(values)==3usize {29}else{0}
}
"#;

fn mixed_source() -> String {
    SUCCESS.replace(" let input=", r#" let scalar_empty=vec_with_capacity<i64>(1usize);
 let scalar_one=vec_push<i64>(scalar_empty,11);
 let scalar_room=vec_reserve_exact<i64>(scalar_one,1usize);
 let scalar_set=vec_set<i64>(scalar_room,0usize,17);
 let scalar_seen=vec_get<i64>(scalar_set,0usize)==17;
 let scalar_clear=vec_clear<i64>(scalar_set);
 let scalar_ok=scalar_seen && vec_capacity<i64>(scalar_clear)==2usize && vec_len<i64>(scalar_clear)==0usize;
 let input="#).replace(" if observed", " if scalar_ok && observed")
}

fn composed_source() -> String {
    SUCCESS.replace("@id(\"app.main\")", r#"@id("vec.bytes.append") fn append(values: own Vec<Bytes>, payload: own Bytes)->Vec<Bytes> {
 vec_push<Bytes>(values,payload)
}
@id("app.main")"#).replace("vec_push<Bytes>(empty,", "append(empty,")
}

fn composed_failure_source() -> String {
    composed_source().replace(
        "payload: own Bytes)->Vec<Bytes> {",
        "payload: own Bytes)->Vec<Bytes> requires false {",
    )
}

fn failure_source(operation: &str) -> String {
    format!(
        r#"module vec.bytes.failure;
@id("app.main") fn main()->i64 {{
 let input=[4u8,5u8];
 let empty=vec_with_capacity<Bytes>(1usize);
 let first=vec_push<Bytes>(empty,bytes_copy(array_as_slice(input)));
 let large=8192usize;
 let failed={operation};
 0
}}
"#
    )
}

#[test]
fn owned_vec_bytes_interpreter_and_native_mutation_settlement() {
    let cases = [
        (SUCCESS.to_owned(), "", 0, 29),
        (MUTABLE.to_owned(), "", 0, 29),
        (mixed_source(), "", 0, 29),
        (composed_source(), "", 0, 29),
        (composed_failure_source(), "semaprax.contract.v1", 1, 0),
        (
            failure_source("vec_push<Bytes>(first,bytes_copy(array_as_slice(input)))"),
            "semaprax.vec.v1",
            1,
            0,
        ),
        (
            failure_source("vec_set<Bytes>(first,1usize,bytes_copy(array_as_slice(input)))"),
            "semaprax.vec.v1",
            2,
            0,
        ),
        (
            failure_source("vec_reserve_exact<Bytes>(first,large)"),
            "semaprax.vec.v1",
            3,
            0,
        ),
        (
            SUCCESS.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
            "semaprax.contract.v1",
            2,
            0,
        ),
        (
            SUCCESS.replace("fn main()->i64 {", "fn main()->i64 requires false {"),
            "semaprax.contract.v1",
            1,
            0,
        ),
    ];
    let path = std::env::temp_dir().join(format!("owned-vec-bytes-{}.spx", std::process::id()));
    for (source, domain, code, value) in &cases {
        semaprax::check(source, &path).unwrap();
        std::fs::write(&path, source).unwrap();
        for _ in 0..4 {
            let outcome = interpreter::interpret(
                &path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            interpreter::verify_envelope(&outcome.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
            if *code == 0 {
                assert!(outcome.returned);
                assert_eq!(envelope["payload"]["outcome"]["value"], value.to_string());
            } else {
                assert!(!outcome.returned);
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    *domain
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], *code);
            }
        }
        native::run_native(source, domain, *code, *value, "none");
    }
    native::run_native(SUCCESS, "semaprax.vec.v1", 3, 0, "allocation");
    native::run_native(SUCCESS, "semaprax.vec.v1", 3, 0, "reserve");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn owned_vec_bytes_wasm_mutations_failures_and_legacy_host() {
    run_wasm(SUCCESS, 0, 4, "none");
    run_wasm(MUTABLE, 0, 2, "none");
    run_wasm(&mixed_source(), 0, 4, "none");
    run_wasm(&composed_source(), 0, 4, "none");
    run_wasm(&composed_failure_source(), 9, 1, "none");
    run_wasm(
        &failure_source("vec_push<Bytes>(first,bytes_copy(array_as_slice(input)))"),
        13,
        2,
        "none",
    );
    run_wasm(
        &failure_source("vec_set<Bytes>(first,1usize,bytes_copy(array_as_slice(input)))"),
        14,
        2,
        "none",
    );
    run_wasm(
        &failure_source("vec_reserve_exact<Bytes>(first,large)"),
        15,
        1,
        "none",
    );
    run_wasm(
        &SUCCESS.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
        10,
        4,
        "none",
    );
    run_wasm(
        &SUCCESS.replace("fn main()->i64 {", "fn main()->i64 requires false {"),
        9,
        0,
        "none",
    );
    run_wasm(SUCCESS, 15, 0, "allocation");
    run_wasm(SUCCESS, 15, 1, "reserve");
}

fn run_wasm(source: &str, status: u32, copies: u32, refusal: &str) {
    let parsed = semaprax::check(source, "vec-bytes-wasm.spx").unwrap();
    let bytes = wasm::emit_module(&parsed).unwrap();
    let path = std::env::temp_dir().join(format!("owned-vec-bytes-{}.wasm", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let script = r#"
const fs=require('fs'),moduleBytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectedCopies=Number(process.argv[3]),refusal=process.argv[4];
let instance,next=1,nextVec=1n,copies=0,drops=0;const data=new Map(),vectors=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {n:Number(w&0xffffffffn),r:Number(w>>32n)}};
const read=c=>{const {n,r}=decode(c);if(r&0x80000000){const a=data.get(r&0x7fffffff);if(!a||a.length!==n)throw Error('stale Bytes');return a;}const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(r>memory.buffer.byteLength-n)throw Error('range');return new Uint8Array(memory.buffer,r,n);};
const alloc=c=>{const a=new Uint8Array(read(c)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(a.length));};
const drop=c=>{read(c);const {r}=decode(c);if(!(r&0x80000000)||!data.delete(r&0x7fffffff))throw Error('double Bytes drop');drops++;};
const get=(h,t)=>{const v=vectors.get(h);if(!v||t<1||t>9||v.tag!==t)throw Error('stale Vec');return v;};
const move=(h,v)=>{vectors.delete(h);const next=nextVec++;vectors.set(next,v);return next;};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:s=>{throw Error(`status:${s}`)},
spx_bytes_copy:alloc,spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_as_slice:c=>{read(c);return c},spx_bytes_drop:drop,
spx_vec_with_capacity_v2:(tag,c)=>{if(tag<1||tag>9)throw Error('tag');if(c>8192n||refusal==='allocation')return 0n;const h=nextVec++;vectors.set(h,{tag,capacity:c,values:[]});return h;},
spx_vec_push_v2:(h,t,b)=>{const v=get(h,t);if(t===9)read(b);if(BigInt(v.values.length)>=v.capacity)return 0n;v.values.push(b);return move(h,v);},
spx_vec_len_v2:(h,t)=>BigInt(get(h,t).values.length),spx_vec_capacity_v2:(h,t)=>get(h,t).capacity,
spx_vec_get_v2:(h,t,i)=>{const v=get(h,t);if(t===9)throw Error('owned payload copied');return v.values[Number(i)]},
spx_vec_drop_v2:h=>{const raw=vectors.get(h);if(!raw)throw Error('stale Vec drop');const v=get(h,raw.tag);if(v.tag===9)for(const b of v.values)drop(b);vectors.delete(h);},
spx_vec_reserve_exact_v2:(h,t,n)=>{const v=get(h,t),target=BigInt(v.values.length)+n;if(target>8192n||refusal==='reserve')return 0n;v.capacity=target>v.capacity?target:v.capacity;return move(h,v);},
spx_vec_set_v2:(h,t,i,b)=>{const v=get(h,t);if(t===9)read(b);if(i>=BigInt(v.values.length))return 0n;if(t===9)drop(v.values[Number(i)]);v.values[Number(i)]=b;return move(h,v);},
spx_vec_clear_v2:(h,t)=>{const v=get(h,t);if(t===9)for(const b of v.values)drop(b);v.values=[];return move(h,v);}};
(async()=>{const legacy={...env};for(const n of Object.keys(legacy))if(n.endsWith('_v2'))delete legacy[n];let rejected=false;try{await WebAssembly.instantiate(moduleBytes,{env:legacy})}catch(e){if(!(e instanceof WebAssembly.LinkError))throw e;rejected=true}if(!rejected)throw Error('legacy host admitted');
({instance}=await WebAssembly.instantiate(moduleBytes,{env}));for(let i=0;i<4;i++){copies=0;drops=0;let selected=0,value;try{value=instance.exports.semaprax_main()}catch(e){if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7));}if(selected!==expected||(!expected&&value!==29n)||copies!==expectedCopies||drops!==copies||vectors.size||data.size)throw Error(`settlement:${selected}:${value}:${copies}:${drops}:${vectors.size}:${data.size}`);}})().catch(e=>{console.error(e);process.exit(2)});
"#;
    let out = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&path)
        .arg(status.to_string())
        .arg(copies.to_string())
        .arg(refusal)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "status {status} refusal {refusal}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_file(path).unwrap();
}
