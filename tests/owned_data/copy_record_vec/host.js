const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]),expectedValue=BigInt(process.argv[3]),refuse=process.argv[4]==='1';
let next=1n;const entries=new Map();
const read=(h,tag)=>{const v=entries.get(h);if(typeof h!=='bigint'||h===0n||!v||v.tag!==tag||tag!==1)throw Error('stale or mistyped Copy record carrier');return v};
const alloc=(tag,capacity,values=[])=>{const h=next++;entries.set(h,{tag,capacity,values});return h};
const move=(h,v)=>{if(!entries.delete(h))throw Error('double move');return alloc(v.tag,v.capacity,v.values)};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:c=>{throw Error('status:'+c)},
spx_vec_with_capacity:(t,n)=>!refuse&&t===1&&n>=0n&&n<=8192n?alloc(t,Number(n)):0n,
spx_vec_push:(h,t,b)=>{const v=read(h,t);if(v.values.length===v.capacity)return 0n;v.values.push(b);return move(h,v)},
spx_vec_len:(h,t)=>BigInt(read(h,t).values.length),spx_vec_capacity:(h,t)=>BigInt(read(h,t).capacity),
spx_vec_get:(h,t,i)=>{const v=read(h,t);if(i<0n||i>=BigInt(v.values.length))throw Error('unchecked get');return v.values[Number(i)]},
spx_vec_drop:h=>{if(!entries.delete(h))throw Error('double drop')},
spx_vec_reserve_exact:(h,t,n)=>{const v=read(h,t),needed=BigInt(v.values.length)+n;if(n<0n||needed>8192n)return 0n;v.capacity=Math.max(v.capacity,Number(needed));return move(h,v)},
spx_vec_set:(h,t,i,b)=>{const v=read(h,t);if(i<0n||i>=BigInt(v.values.length))return 0n;v.values[Number(i)]=b;return move(h,v)},
spx_vec_clear:(h,t)=>{const v=read(h,t);v.values=[];return move(h,v)},
spx_vec_sort_v3:()=>{throw Error('record ordering must use checked field types')}};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let n=0;n<3;n++){let result,selected=0;try{result=instance.exports.semaprax_main()}catch(e){if(!e.message.startsWith('status:'))throw e;selected=Number(e.message.slice(7))}if(selected!==expected||(!expected&&result!==expectedValue)||entries.size!==0)throw Error(`result/settlement ${selected}/${result}/${entries.size}`)}}).catch(e=>{console.error(e);process.exit(2)});
