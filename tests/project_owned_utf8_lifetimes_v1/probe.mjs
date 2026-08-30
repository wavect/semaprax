import {readFile} from 'node:fs/promises';
import {webcrypto} from 'node:crypto';
import {probeArena, instantiate} from './arena-probe.mjs';
if (!globalThis.crypto) globalThis.crypto = webcrypto;
const wasm = new Uint8Array(await readFile(new URL('./app.wasm', import.meta.url)));
const arena = probeArena();
let copies = 0, drops = 0, live = 0, peak = 0;
const minted = [];
const env = {
  spx_add:(a,b)=>a+b, spx_sub:(a,b)=>a-b, spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b, spx_rem:(a,b)=>a%b, spx_neg:a=>-a,
  spx_contract_fail:()=>{throw Error('unexpected host contract failure');},
  ...arena.imports,
  spx_bytes_copy:carrier=>{
    const result=arena.imports.spx_bytes_copy(carrier);
    copies++; live++; peak=Math.max(peak,live);
    const length=Number(BigInt.asUintN(64,result)&0xffffffffn);
    minted.push(Uint8Array.from({length},(_,i)=>arena.imports.spx_bytes_get(result,BigInt(i))));
    return result;
  },
  spx_bytes_drop:carrier=>{arena.imports.spx_bytes_drop(carrier);drops++;live--;}
};
const {instance}=await WebAssembly.instantiate(wasm,{env});
arena.bind(instance);
const memory=new Uint8Array(instance.exports.memory.buffer), view=new DataView(memory.buffer);
const raw=id=>instance.exports['spx_owned_v1_'+Array.from(new TextEncoder().encode(id),b=>b.toString(16).padStart(2,'0')).join('')];
function run(id,args,expectedCopies,status=0){
  arena.begin();
  const beforeCopies=copies, beforeDrops=drops;
  memory.fill(0xa5,65536,65544);
  if(raw(id)(...args,65536)!==status)throw Error(`status ${id}`);
  if(copies-beforeCopies!==expectedCopies)throw Error(`copies ${id}: ${copies-beforeCopies} != ${expectedCopies}`);
  if(status===0){
    if(live!==1||drops-beforeDrops!==expectedCopies-1)throw Error(`success ownership ${id}`);
    const bytes=arena.consume(view.getBigInt64(65536,true)); live--;
    if(new TextDecoder().decode(bytes)!=='done')throw Error(`result ${id}`);
  }else{
    if(memory.subarray(65536,65544).some(byte=>byte!==0xa5))throw Error(`failure published ${id}`);
    if(live!==0||drops-beforeDrops!==expectedCopies)throw Error(`failed ownership ${id}`);
  }
  arena.settle();
  if(live!==0)throw Error('remaining owners');
}
for(let repeat=0;repeat<3;repeat++){
  run('s.simple',[],2);
  const before=minted.length;
  run('s.clone',[],4);
  if(minted.slice(before,before+3).some(bytes=>new TextDecoder().decode(bytes)!=='alpha'))throw Error('String read failed to clone source');
  run('s.branch',[1],2); run('s.branch',[0],2);
  run('s.loop',[40n],82); // 41 condition literals + 40 body literals + result.
  run('s.pressure',[],19); // 17 locals + clone + result.
  memory.set([7,0,255],0);
  run('s.mixed',[0,3],18); // one String + sixteen Bytes + result.
  run('s.late',[0n],1,4); run('s.late',[1n],2);
  run('s.outer',[0n],2,4); run('s.outer',[1n],3);
  run('s.local-fail',[0n],1,4); run('s.local-fail',[1n],2);
}
if(peak<18)throw Error(`fixture did not exceed old arena limit: ${peak}`);

// Real generated facade must recover after checked failures and oversized input
// rejection without retaining ownership from earlier invocations.
const facade=await instantiate(wasm);
for(let repeat=0;repeat<3;repeat++){
  for(const id of ['s.late','s.outer','s.local-fail']){
    let failed=false;try{facade.call(id,0n);}catch(error){failed=error.status===4;}
    if(!failed||facade.call(id,1n)!=='done')throw Error('facade checked failure recovery');
  }
  if(facade.call('s.pressure')!=='done'||facade.call('s.loop',40n)!=='done')throw Error('facade pressure/loop');
  let rejected=false;try{facade.call('s.mixed',new Uint8Array(65537));}catch(error){rejected=error instanceof RangeError;}
  if(!rejected||facade.call('s.mixed',new Uint8Array([7]))!=='done')throw Error('input rejection recovery');
}

// Exercise exact derived capacity and +1, without changing production bytes.
const text=await readFile(new URL('./semaprax.js',import.meta.url),'utf8');
const capacity=Number(/entries\.size>=(\d+)\|\|/.exec(text)?.[1]);
if(!Number.isInteger(capacity)||capacity<18||capacity>0x7fffffff)throw Error('derived capacity');
const quota=probeArena(); quota.bind(instance); quota.begin();
const owners=[];
for(let index=0;index<capacity;index++)owners.push(quota.imports.spx_bytes_copy(0n));
let rejected=false;try{quota.imports.spx_bytes_copy(0n);}catch(error){rejected=error.message==='SEMAPRAX owned arena exhausted';}
if(!rejected)throw Error('quota +1 admitted');
for(const owner of owners)quota.imports.spx_bytes_drop(owner);
quota.settle(); quota.begin();
const replacement=quota.imports.spx_bytes_copy(0n);
if(owners.includes(replacement))throw Error('token reused');
quota.imports.spx_bytes_drop(replacement); quota.settle();
const unsettled=probeArena(); unsettled.bind(instance);
const retained=unsettled.imports.spx_bytes_copy(0n);
let refused=false;try{unsettled.begin();}catch{refused=true;}
unsettled.imports.spx_bytes_drop(retained);
let sticky=false;try{unsettled.begin();}catch{sticky=true;}
if(!refused||!sticky)throw Error('unsettled reentry was not sticky');
