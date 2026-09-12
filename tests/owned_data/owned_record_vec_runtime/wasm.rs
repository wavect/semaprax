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
const fs=require('fs'),moduleBytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectedValue=BigInt(process.argv[3]),expectedCopies=Number(process.argv[4]),expectedScalars=process.argv[5]===''?[]:process.argv[5].split(','),refusal=process.argv[6],recordMax=BigInt(process.argv[7]);
let instance,next=1,nextVec=1n,copies=0,drops=0,scalars=[];const data=new Map(),vectors=new Map();
const decode=c=>{const w=BigInt.asUintN(64,c);return {n:Number(w&0xffffffffn),r:Number(w>>32n)}};
const read=c=>{const {n,r}=decode(c);if(r&0x80000000){const a=data.get(r&0x7fffffff);if(!a||a.length!==n)throw Error('stale Bytes');return a;}const memory=instance.exports.__spx_byte_memory||instance.exports.memory;if(r>memory.buffer.byteLength-n)throw Error('range');return new Uint8Array(memory.buffer,r,n);};
const alloc=c=>{const a=new Uint8Array(read(c)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(a.length));};
const drop=c=>{read(c);const {r}=decode(c);if(!(r&0x80000000)||!data.delete(r&0x7fffffff))throw Error('double Bytes drop');drops++;};
const get=(h,t)=>{const v=vectors.get(h);if(!v||t<1||t>10||v.tag!==t)throw Error('stale Vec');return v;};
const move=(h,v)=>{vectors.delete(h);const successor=nextVec++;vectors.set(successor,v);return successor;};
const dropElements=v=>{if(v.tag===9){for(const b of v.values)drop(b)}else if(v.tag===10){for(const e of v.values){drop(e[0]);drop(e[1])}}};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:s=>{throw Error(`status:${s}`)},
spx_bytes_copy:alloc,spx_bytes_zeroed:n=>{if(n<0n||n>65536n)throw Error('Bytes capacity');const a=new Uint8Array(Number(n)),id=next++;data.set(id,a);copies++;return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|n);},spx_bytes_set:(c,i,v)=>{const a=read(c);if(typeof i!=='bigint'||i<0n||i>=BigInt(a.length)||!Number.isInteger(v)||v<0||v>255)throw Error('Bytes set');a[Number(i)]=v;return c;},spx_bytes_get:(c,i)=>read(c)[Number(i)]??-1,spx_bytes_as_slice:c=>{read(c);return c},spx_bytes_drop:drop,
spx_vec_with_capacity_v2:(tag,c)=>{if(tag<1||tag>10)throw Error('tag');const max=tag===10?recordMax:8192n;if(c>max||(refusal==='allocation'&&tag===10))return 0n;const h=nextVec++;vectors.set(h,{tag,capacity:c,values:[]});return h;},
spx_vec_push_v2:(h,t,b)=>{if(t===10)throw Error('owned record payload requires the record push');const v=get(h,t);if(t===9)read(b);if(BigInt(v.values.length)>=v.capacity)return 0n;v.values.push(b);return move(h,v);},
spx_vec_record_push_v2:(h,t,b0,b1,s)=>{const v=get(h,t);if(t!==10)throw Error('record push tag');read(b0);read(b1);if(b0===b1)throw Error('record element aliases one payload');if(BigInt(v.values.length)>=v.capacity)return 0n;scalars.push(s.toString());v.values.push([b0,b1,s]);return move(h,v);},
spx_vec_len_v2:(h,t)=>BigInt(get(h,t).values.length),spx_vec_capacity_v2:(h,t)=>get(h,t).capacity,
spx_vec_get_v2:(h,t,i)=>{const v=get(h,t);if(t>=9)throw Error('owned payload copied');return v.values[Number(i)]},
spx_vec_drop_v2:h=>{const raw=vectors.get(h);if(!raw)throw Error('stale Vec drop');const v=get(h,raw.tag);dropElements(v);vectors.delete(h);},
spx_vec_reserve_exact_v2:(h,t,n)=>{if(t===10)throw Error('owned record payload has no reserve');const v=get(h,t),target=BigInt(v.values.length)+n;if(target>8192n)return 0n;v.capacity=target>v.capacity?target:v.capacity;return move(h,v);},
spx_vec_set_v2:(h,t,i,b)=>{if(t===10)throw Error('owned record payload has no set');const v=get(h,t);if(t===9)read(b);if(i>=BigInt(v.values.length))return 0n;if(t===9)drop(v.values[Number(i)]);v.values[Number(i)]=b;return move(h,v);},
spx_vec_clear_v2:(h,t)=>{const v=get(h,t);dropElements(v);v.values=[];return move(h,v);}};
(async()=>{
// A host without the record push cannot link this module at all: the profile's
// element is not silently reinterpreted as some other admitted payload.
const legacy={...env};delete legacy.spx_vec_record_push_v2;let rejected=false;
try{await WebAssembly.instantiate(moduleBytes,{env:legacy})}catch(e){if(!(e instanceof WebAssembly.LinkError))throw e;rejected=true}
if(!rejected)throw Error('host without the record push admitted the module');
({instance}=await WebAssembly.instantiate(moduleBytes,{env}));
for(let i=0;i<4;i++){copies=0;drops=0;scalars=[];let selected=0,value;
try{value=instance.exports.semaprax_main()}catch(e){if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7));}
if(selected!==expected)throw Error(`status:${selected}:expected:${expected}`);
if(!expected&&value!==expectedValue)throw Error(`value:${value}:expected:${expectedValue}`);
if(copies!==expectedCopies||drops!==copies)throw Error(`payloads:${copies}:${drops}:expected:${expectedCopies}`);
if(scalars.join(',')!==expectedScalars.join(','))throw Error(`elements:${scalars.join(',')}:expected:${expectedScalars.join(',')}`);
if(vectors.size||data.size)throw Error(`live:${vectors.size}:${data.size}`);}
})().catch(e=>{console.error(e);process.exit(2)});
"#;
