//! Core-Wasm execution host for the SPX-AI-019 owned-record collection.
//!
//! The emitted module is instantiated under Node with a host that implements
//! the owned-payload boundary plus the one function the record element adds.
//! The host is the runtime here — the carrier never enters linear memory — so
//! the settlement probe is a host-side one: after every invocation it must hold
//! zero live vector handles and zero live `Bytes` handles, and it must have
//! dropped exactly as many payloads as it allocated. A double drop is an error,
//! not a silently tolerated decrement.
//!
//! The host also records the scalar of every element it is handed, in order.
//! That is the direct evidence of real per-element record storage: a carrier
//! that stored nothing, or that stored one fused blob, could not reproduce the
//! authored sequence.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// The capacity ceiling a conforming host enforces for the record element.
///
/// It is `crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES` divided by
/// `hir::owned_record_collection::OWNED_PAYLOAD_BYTES_PER_RECORD_ELEMENT`, the
/// same ratio the reference interpreter and the native C11 runtime bound
/// themselves by. `wasm::vec_ops::RECORD_ELEMENT_MAX_CAPACITY` computes it
/// from those constants and a `const` assertion pins it to this number, so a
/// change to either fails the compiler's own build rather than letting this
/// host drift away from the other two backends.
const RECORD_ELEMENT_MAX_CAPACITY: u64 = 4_096;

/// Run one fixture's emitted module under Node.
///
/// `status` is the Wasm status integer the module selects (0 for a published
/// value), `copies` the exact number of `Bytes` payloads the host must allocate
/// and drop, and `scalars` the ordered element scalars the host must be handed.
pub(crate) fn run_wasm(
    source: &str,
    status: u32,
    value: i64,
    copies: u32,
    scalars: &[i64],
    refusal: &str,
) {
    let case = source
        .lines()
        .find_map(|line| line.strip_prefix("module "))
        .unwrap_or("unnamed");
    let parsed = semaprax::check(source, "owned-record-vec-wasm.spx")
        .expect("the source verifier must admit the profile");
    let bytes = semaprax::wasm::emit_module(&parsed).expect("Wasm must emit the profile");
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "owned-record-vec-{}-{sequence}.wasm",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    let observed = scalars
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let out = Command::new("node")
        .arg("-e")
        .arg(HOST)
        .arg(&path)
        .arg(status.to_string())
        .arg(value.to_string())
        .arg(copies.to_string())
        .arg(&observed)
        .arg(refusal)
        .arg(RECORD_ELEMENT_MAX_CAPACITY.to_string())
        .output()
        .expect("node must be available to execute the Core Wasm lane");
    assert!(
        out.status.success(),
        "case {case} status {status} refusal {refusal}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_file(path).unwrap();
}

const HOST: &str = r#"
const fs=require('fs'),moduleBytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectsTrap=expected===4294967295,expectedValue=BigInt(process.argv[3]),expectedCopies=Number(process.argv[4]),expectedScalars=process.argv[5]===''?[]:process.argv[5].split(','),refusal=process.argv[6],recordMax=BigInt(process.argv[7]);
let instance,next=1,nextVec=1n,nextBox=1n<<61n,nextIter=1n<<62n,copies=0,drops=0,scalars=[];const data=new Map(),vectors=new Map(),boxes=new Map(),iterators=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {n:Number(w&0xffffffffn),r:Number(w>>32n)}};
const read=c=>{const {n,r}=decode(c);if(r&0x80000000){const a=data.get(r&0x7fffffff);if(!a||a.length!==n)throw Error('stale Bytes');return a;}const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(r>memory.buffer.byteLength-n)throw Error('range');return new Uint8Array(memory.buffer,r,n);};
const alloc=c=>{const a=new Uint8Array(read(c)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(a.length));};
const drop=c=>{read(c);const {r}=decode(c);if(!(r&0x80000000)||!data.delete(r&0x7fffffff))throw Error('double Bytes drop');drops++;};
const get=(h,t)=>{const v=vectors.get(h);if(!v||t<1||t>10||v.tag!==t)throw Error('stale Vec');return v;};
const move=(h,v)=>{vectors.delete(h);const successor=nextVec++;vectors.set(successor,v);return successor;};
const dropElements=v=>{if(v.tag===9){for(const b of v.values)drop(b)}else if(v.tag===10){for(const e of v.values){drop(e[0]);drop(e[1])}}};
const memoryView=(out,length)=>{if(!Number.isInteger(out)||out<0)throw Error('iterator output');const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(out>memory.buffer.byteLength-length)throw Error('iterator output');return new DataView(memory.buffer,out,length);};
const iterGet=(h,c)=>{const i=iterators.get(h);if(!i||c!==i.cursor||c<0n||c>BigInt(i.values.length))throw Error('stale Iter');return i;};
const recordInto=(h,out)=>{const v=get(h,10),view=memoryView(out,16),i=nextIter++;vectors.delete(h);iterators.set(i,{values:v.values,cursor:0n});view.setBigUint64(0,i,true);view.setBigUint64(8,0n,true);return 0;};
const recordNext=(h,c,out,b0,b1,s,rest)=>{const i=iterGet(h,c),view=memoryView(out,Math.max(rest+16,b0+8,b1+8,s+8));if(refusal==='record-nowrite-next')return 0;view.setUint32(0,0,true);for(const offset of [b0,b1,s,rest,rest+8])view.setBigUint64(offset,0n,true);if(refusal==='record-invalid-tag'){view.setUint32(0,2,true);return 0;}if(c===BigInt(i.values.length)){iterators.delete(h);return 0;}const value=i.values[Number(c)];read(value[0]);read(value[1]);const hostile=['record-borrowed-item','record-aliased-items','record-stale-rest','record-stale-handle','record-invalid-bool','record-invalid-u8','record-noncanonical-i32','record-noncanonical-f32','record-invalid-char'].includes(refusal);if(hostile){let scalar=BigInt.asUintN(64,value[2]);if(refusal==='record-invalid-bool')scalar=2n;if(refusal==='record-invalid-u8')scalar=256n;if(refusal==='record-noncanonical-i32')scalar=0xffffffffn;if(refusal==='record-noncanonical-f32')scalar=0x000000013fc00000n;if(refusal==='record-invalid-char')scalar=0xd800n;view.setUint32(0,1,true);view.setBigInt64(b0,refusal==='record-borrowed-item'?1n:value[0],true);view.setBigInt64(b1,refusal==='record-aliased-items'?value[0]:value[1],true);view.setBigUint64(s,scalar,true);view.setBigUint64(rest,refusal==='record-stale-handle'?h:h+1n,true);view.setBigUint64(rest+8,refusal==='record-stale-rest'?c+2n:c+1n,true);return 0;}const successor=nextIter++;iterators.delete(h);iterators.set(successor,{values:i.values,cursor:c+1n});view.setUint32(0,1,true);view.setBigInt64(b0,value[0],true);view.setBigInt64(b1,value[1],true);view.setBigUint64(s,BigInt.asUintN(64,value[2]),true);view.setBigUint64(rest,successor,true);view.setBigUint64(rest+8,c+1n,true);return 0;};
const recordIterDrop=(h,c)=>{const i=iterGet(h,c);for(let n=Number(c);n<i.values.length;n++){drop(i.values[n][0]);drop(i.values[n][1]);}iterators.delete(h);};
const boxGet=(h,t)=>{const value=boxes.get(h);if(!value||value.tag!==t)throw Error('stale Box');return value;};
const boxNew=(t,bits)=>{const h=nextBox++;boxes.set(h,{tag:t,bits});return h;};
const boxRead=(h,t)=>boxGet(h,t).bits;
const boxInto=(h,t)=>{const value=boxGet(h,t);boxes.delete(h);return value.bits;};
const boxDrop=h=>{if(!boxes.delete(h))throw Error('stale Box drop');};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:s=>{throw Error(`status:${s}`)},
spx_bytes_copy:alloc,spx_bytes_zeroed:n=>{if(n<0n||n>131072n)throw Error('Bytes capacity');const a=new Uint8Array(Number(n)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|n);},spx_bytes_set:(c,i,v)=>{const a=read(c);if(typeof i!=='bigint'||i<0n||i>=BigInt(a.length)||!Number.isInteger(v)||v<0||v>255)throw Error('Bytes set');a[Number(i)]=v;return c;},spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_as_slice:c=>{read(c);return c},spx_bytes_drop:drop,
spx_bytes_set5:()=>{throw Error('unexpected five-byte write')},spx_bytes_set1_or5:()=>{throw Error('unexpected one-or-five write')},spx_bytes_set1_or6_or48:()=>{throw Error('unexpected one-or-six-or-forty-eight write')},
spx_vec_with_capacity_v2:(tag,c)=>{if(tag<1||tag>10)throw Error('tag');const max=tag===10?recordMax:8192n;if(c>max||(refusal==='allocation'&&tag===10))return 0n;const h=nextVec++;vectors.set(h,{tag,capacity:c,values:[]});return h;},
spx_vec_push_v2:(h,t,b)=>{if(t===10)throw Error('owned record payload requires the record push');const v=get(h,t);if(t===9)read(b);if(BigInt(v.values.length)>=v.capacity)return 0n;v.values.push(b);return move(h,v);},
spx_vec_record_push_v2:(h,t,b0,b1,s)=>{const v=get(h,t);if(t!==10)throw Error('record push tag');read(b0);read(b1);if(b0===b1)throw Error('record element aliases one payload');if(BigInt(v.values.length)>=v.capacity)return 0n;scalars.push(s.toString());v.values.push([b0,b1,s]);return move(h,v);},
spx_vec_len_v2:(h,t)=>BigInt(get(h,t).values.length),spx_vec_capacity_v2:(h,t)=>get(h,t).capacity,
spx_vec_get_v2:(h,t,i)=>{const v=get(h,t);if(t>=9)throw Error('owned payload copied');return v.values[Number(i)]},
spx_vec_drop_v2:h=>{const raw=vectors.get(h);if(!raw)throw Error('stale Vec drop');const v=get(h,raw.tag);dropElements(v);vectors.delete(h);},
spx_vec_reserve_exact_v2:(h,t,n)=>{if(t===10)throw Error('owned record payload has no reserve');const v=get(h,t),target=BigInt(v.values.length)+n;if(target>8192n)return 0n;v.capacity=target>v.capacity?target:v.capacity;return move(h,v);},
spx_vec_set_v2:(h,t,i,b)=>{if(t===10)throw Error('owned record payload has no set');const v=get(h,t);if(t===9)read(b);if(i>=BigInt(v.values.length))return 0n;if(t===9)drop(v.values[Number(i)]);v.values[Number(i)]=b;return move(h,v);},
spx_vec_clear_v2:(h,t)=>{const v=get(h,t);dropElements(v);v.values=[];return move(h,v);},
spx_iter_record_into_v3:recordInto,spx_iter_record_next_v3:recordNext,spx_iter_record_drop_v3:recordIterDrop,
spx_box_new:boxNew,spx_box_get:boxRead,spx_box_into_inner:boxInto,spx_box_drop:boxDrop};
(async()=>{
// A host without the record push cannot link this module at all: the profile's
// element is not silently reinterpreted as some other admitted payload.
const legacy={...env};delete legacy.spx_vec_record_push_v2;let rejected=false;
try{await WebAssembly.instantiate(moduleBytes,{env:legacy})}catch(e){if(!(e instanceof WebAssembly.LinkError))throw e;rejected=true}
if(!rejected)throw Error('host without the record push admitted the module');
({instance}=await WebAssembly.instantiate(moduleBytes,{env}));
for(let i=0;i<4;i++){copies=0;drops=0;scalars=[];let selected=0,value,trapped=false;
try{value=instance.exports.semaprax_main()}catch(e){if(expectsTrap&&e instanceof WebAssembly.RuntimeError){trapped=true}else{if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7));}}
if(expectsTrap){if(!trapped)throw Error('malformed record iterator output published');if(vectors.size||boxes.size||iterators.size!==1||data.size!==expectedCopies)throw Error('hostile record iterator committed');const [handle,state]=iterators.entries().next().value;recordIterDrop(handle,state.cursor);if(vectors.size||boxes.size||iterators.size||data.size)throw Error('hostile record iterator cleanup');continue;}
if(selected!==expected)throw Error(`status:${selected}:expected:${expected}`);
if(!expected&&value!==expectedValue)throw Error(`value:${value}:expected:${expectedValue}`);
if(copies!==expectedCopies||drops!==copies)throw Error(`payloads:${copies}:${drops}:expected:${expectedCopies}`);
if(scalars.join(',')!==expectedScalars.join(','))throw Error(`elements:${scalars.join(',')}:expected:${expectedScalars.join(',')}`);
if(vectors.size||boxes.size||iterators.size||data.size)throw Error(`live:${vectors.size}:${boxes.size}:${iterators.size}:${data.size}`);}
})().catch(e=>{console.error(e);process.exit(2)});
"#;
