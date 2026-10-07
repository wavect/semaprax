const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]);
let next=1n;const entries=new Map();
const key=h=>{if(typeof h!=='bigint'||h===0n)throw Error('invalid token');return h.toString()};
const read=(h,tag)=>{const v=entries.get(key(h));if(!v||v.tag!==tag)throw Error('stale or mistyped token');return v};
const alloc=(tag,capacity,values=[])=>{const h=next++;entries.set(key(h),{tag,capacity,values});return h};
const move=(h,v)=>{entries.delete(key(h));return alloc(v.tag,v.capacity,v.values)};
const sortKey=(tag,b)=>{b=BigInt.asUintN(64,b);if(tag===1)return b^0x8000000000000000n;if(tag===2)return (b&0xffffffffn)^0x80000000n;if(tag===6){b&=0xffffffffn;return b&0x80000000n?(~b&0xffffffffn):(b^0x80000000n)}if(tag===7)return b&0x8000000000000000n?BigInt.asUintN(64,~b):b^0x8000000000000000n;return b};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:c=>{throw Error('status '+c)},
spx_vec_with_capacity:(t,n)=>n<=8192n?alloc(t,Number(n)):0n,
spx_vec_push:(h,t,b)=>{const v=read(h,t);if(v.values.length===v.capacity)return 0n;v.values.push(b);return move(h,v)},
spx_vec_len:(h,t)=>BigInt(read(h,t).values.length),spx_vec_capacity:(h,t)=>BigInt(read(h,t).capacity),
spx_vec_get:(h,t,i)=>{const v=read(h,t);if(i<0n||i>=BigInt(v.values.length))throw Error('bounds');return v.values[Number(i)]},
spx_vec_drop:h=>{if(!entries.delete(key(h)))throw Error('double drop')},
spx_vec_sort_v3:(h,t)=>{if(t<1||t>8)throw Error('owned payload sort');const v=read(h,t);v.values.sort((a,b)=>sortKey(t,a)<sortKey(t,b)?-1:sortKey(t,a)>sortKey(t,b)?1:0);return move(h,v)}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let n=0;n<4;n++){const result=instance.exports.semaprax_main();if(result!==42n||entries.size!==0)throw Error(`sort/settlement ${result}/${entries.size}`)}}).catch(e=>{console.error(e);process.exit(2)});
