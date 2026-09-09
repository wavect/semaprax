//! Owned vector mutation must settle both the vector and staged payloads.
use semaprax::{interpreter, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static WASM_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    run_wasm_expected(source, status, copies, refusal, 29);
}

pub(crate) fn run_wasm_expected(
    source: &str,
    status: u32,
    copies: u32,
    refusal: &str,
    expected_value: i64,
) {
    let case = source
        .lines()
        .find_map(|line| line.strip_prefix("module "))
        .unwrap_or("unnamed");
    let parsed = semaprax::check(source, "vec-bytes-wasm.spx").unwrap();
    let bytes = wasm::emit_module(&parsed).unwrap();
    let sequence = WASM_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "owned-vec-bytes-{}-{sequence}.wasm",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let script = r#"
const fs=require('fs'),moduleBytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectsTrap=expected===4294967295,expectedCopies=Number(process.argv[3]),refusal=process.argv[4],expectedValue=BigInt(process.argv[5]);
let instance,next=1,nextVec=1n,nextIter=1n<<62n,copies=0,drops=0;const data=new Map(),vectors=new Map(),iterators=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {n:Number(w&0xffffffffn),r:Number(w>>32n)}};
const read=c=>{const {n,r}=decode(c);if(r&0x80000000){const a=data.get(r&0x7fffffff);if(!a||a.length!==n)throw Error('stale Bytes');return a;}const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(r>memory.buffer.byteLength-n)throw Error('range');return new Uint8Array(memory.buffer,r,n);};
const alloc=c=>{const a=new Uint8Array(read(c)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(a.length));};
const drop=c=>{read(c);const {r}=decode(c);if(!(r&0x80000000)||!data.delete(r&0x7fffffff))throw Error('double Bytes drop');drops++;};
const get=(h,t)=>{const v=vectors.get(h);if(!v||t<1||t>9||v.tag!==t)throw Error('stale Vec');return v;};
const move=(h,v)=>{vectors.delete(h);const next=nextVec++;vectors.set(next,v);return next;};
const memoryView=(out,length)=>{if(!Number.isInteger(out)||out<0)throw Error('iterator output');const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(out>memory.buffer.byteLength-length)throw Error('iterator output');return new DataView(memory.buffer,out,length);};
const iterGet=(handle,cursor)=>{const state=iterators.get(handle);if(!state||cursor<0n||cursor!==state.cursor||cursor>BigInt(state.values.length))return null;return state;};
const iterInto=(handle,out)=>{if(refusal==='iter-push')return 1;if(refusal==='iter-nowrite-into')return 0;let vector;try{vector=get(handle,9)}catch(_){return 2}let view;try{view=memoryView(out,16)}catch(_){return 2}const iterator=nextIter++;vectors.delete(handle);iterators.set(iterator,{values:vector.values,cursor:0n});view.setBigUint64(0,iterator,true);view.setBigUint64(8,0n,true);return 0;};
const iterNext=(handle,cursor,out)=>{if(refusal==='iter-get')return 2;if(refusal==='iter-allocation')return 3;if(refusal==='iter-nowrite-next')return 0;const state=iterGet(handle,cursor);if(!state)return 2;let view;try{view=memoryView(out,32)}catch(_){return 2}if(refusal==='iter-invalid-tag'){view.setUint32(0,2,true);return 0}const length=BigInt(state.values.length);if(cursor===length){iterators.delete(handle);for(let i=0;i<32;i++)view.setUint8(i,0);return 0;}const item=state.values[Number(cursor)];try{read(item)}catch(_){return 2}if(refusal==='iter-borrowed-item')drop(item);const successor=nextIter++;iterators.delete(handle);iterators.set(successor,{values:state.values,cursor:cursor+1n});view.setUint32(0,1,true);view.setUint32(4,0,true);view.setBigUint64(8,refusal==='iter-borrowed-item'?1n:item,true);view.setBigUint64(16,successor,true);view.setBigUint64(24,cursor+1n,true);return 0;};
const iterDrop=(handle,cursor)=>{const state=iterGet(handle,cursor);if(!state)throw Error('stale Iter drop');for(let index=Number(cursor);index<state.values.length;index++)drop(state.values[index]);iterators.delete(handle);};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:s=>{throw Error(`status:${s}`)},
spx_bytes_copy:alloc,spx_bytes_zeroed:n=>{if(n<0n||n>65536n)throw Error('Bytes capacity');const a=new Uint8Array(Number(n)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|n);},spx_bytes_set:(c,i,v)=>{const a=read(c);if(typeof i!=='bigint'||i<0n||i>=BigInt(a.length)||!Number.isInteger(v)||v<0||v>255)throw Error('Bytes set');a[Number(i)]=v;return c;},spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_as_slice:c=>{read(c);return c},spx_bytes_drop:drop,
spx_vec_with_capacity_v2:(tag,c)=>{if(tag<1||tag>9)throw Error('tag');if(c>8192n||refusal==='allocation')return 0n;const h=nextVec++;vectors.set(h,{tag,capacity:c,values:[]});return h;},
spx_vec_push_v2:(h,t,b)=>{const v=get(h,t);if(t===9)read(b);if(BigInt(v.values.length)>=v.capacity)return 0n;v.values.push(b);return move(h,v);},
spx_vec_len_v2:(h,t)=>BigInt(get(h,t).values.length),spx_vec_capacity_v2:(h,t)=>get(h,t).capacity,
spx_vec_get_v2:(h,t,i)=>{const v=get(h,t);if(t===9)throw Error('owned payload copied');return v.values[Number(i)]},
spx_vec_drop_v2:h=>{const raw=vectors.get(h);if(!raw)throw Error('stale Vec drop');const v=get(h,raw.tag);if(v.tag===9)for(const b of v.values)drop(b);vectors.delete(h);},
spx_vec_reserve_exact_v2:(h,t,n)=>{const v=get(h,t),target=BigInt(v.values.length)+n;if(target>8192n||refusal==='reserve')return 0n;v.capacity=target>v.capacity?target:v.capacity;return move(h,v);},
spx_vec_set_v2:(h,t,i,b)=>{const v=get(h,t);if(t===9)read(b);if(i>=BigInt(v.values.length))return 0n;if(t===9)drop(v.values[Number(i)]);v.values[Number(i)]=b;return move(h,v);},
spx_vec_clear_v2:(h,t)=>{const v=get(h,t);if(t===9)for(const b of v.values)drop(b);v.values=[];return move(h,v);},
spx_iter_bytes_into_v2:iterInto,spx_iter_bytes_next_v2:iterNext,spx_iter_bytes_drop_v2:iterDrop};
(async()=>{const legacy={...env};for(const n of Object.keys(legacy))if(n.endsWith('_v2'))delete legacy[n];let rejected=false;try{await WebAssembly.instantiate(moduleBytes,{env:legacy})}catch(e){if(!(e instanceof WebAssembly.LinkError))throw e;rejected=true}if(!rejected)throw Error('legacy host admitted');
({instance}=await WebAssembly.instantiate(moduleBytes,{env}));if(refusal==='none'){const memory=instance.exports.__spx_byte_memory||instance.exports.memory,output=memory.buffer.byteLength-32,view=new DataView(memory.buffer,output,32),vector=nextVec++;vectors.set(vector,{tag:9,capacity:0n,values:[]});if(iterInto(0n,output)!==2||vectors.size!==1||iterators.size)throw Error('stale iterator handle committed');if(iterInto(vector,-1)!==2||!vectors.has(vector)||iterators.size)throw Error('invalid iterator output committed');if(iterInto(vector,output)!==0||vectors.has(vector)||iterators.size!==1)throw Error('iterator admission failed');const iterator=view.getBigUint64(0,true);if(iterNext(iterator,1n,output)!==2||!iterators.has(iterator)||iterators.get(iterator).cursor!==0n)throw Error('stale iterator cursor committed');if(iterNext(iterator,0n,-1)!==2||!iterators.has(iterator)||iterators.get(iterator).cursor!==0n)throw Error('invalid step output committed');iterDrop(iterator,0n);if(vectors.size||iterators.size||data.size)throw Error('hostile iterator cleanup');}for(let i=0;i<4;i++){copies=0;drops=0;let selected=0,value,trapped=false;try{value=instance.exports.semaprax_main()}catch(e){if(expectsTrap&&e instanceof WebAssembly.RuntimeError){trapped=true}else{if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7));}}if(expectsTrap){if(!trapped)throw Error('malformed iterator output published');if(refusal==='iter-nowrite-into'){if(vectors.size!==1||iterators.size||data.size!==expectedCopies)throw Error('into trap committed');env.spx_vec_drop_v2(vectors.keys().next().value);}else{if(vectors.size||iterators.size!==1||data.size!==expectedCopies)throw Error('next trap committed');iterDrop(iterators.keys().next().value,refusal==='iter-borrowed-item'?1n:0n);}if(vectors.size||iterators.size||data.size)throw Error('malformed host cleanup');continue;}if(selected!==expected||(!expected&&value!==expectedValue)||copies!==expectedCopies||drops!==copies||vectors.size||iterators.size||data.size)throw Error(`settlement:${selected}:${value}:${copies}:${drops}:${vectors.size}:${iterators.size}:${data.size}`);}})().catch(e=>{console.error(e);process.exit(2)});
"#;
    let out = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&path)
        .arg(status.to_string())
        .arg(copies.to_string())
        .arg(refusal)
        .arg(expected_value.to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "case {case} status {status} refusal {refusal}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_file(path).unwrap();
}
