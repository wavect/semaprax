import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import instantiate from './semaprax.bindings.js';
import {exerciseValidator,faultValidator} from './utf8.mjs';

const config=JSON.parse(process.argv[2]);
const wasm=Uint8Array.from(readFileSync('app.wasm'));
// Compile the authentic package bytes once; every facade still performs its
// own production byte authentication and creates an independent real instance.
const compiled=await WebAssembly.compile(wasm);
const realInstantiate=WebAssembly.instantiate;
const raw=id=>'spx_owned_v1_'+Buffer.from(id).toString('hex');
const input=Uint8Array.of(0,255,195,40);
const RESULT=65536,POISON=0xa5;
const resultSize=config.family==='record'?32:config.family==='variant'?16:8;

function returnedBytes(value,length=4){
  if(config.family==='variant'){assert.equal(value.ok,true);return value.value}
  if(config.family==='record'){
    assert(Object.isFrozen(value));assert.equal(Object.getPrototypeOf(value),null);
    assert.deepEqual(Object.keys(value).sort(),[...config.fields].sort());
    assert.equal(value[config.fields[1]],7n);assert.equal(value[config.fields[2]],true);
    assert.equal(value[config.fields[3]],BigInt(length));return value[config.fields[0]];
  }
  return value;
}
function assertSuccess(api){
  const value=returnedBytes(api.call('case.copy',input,1n));
  assert(value instanceof Uint8Array);assert.deepEqual([...value],[...input]);
  assert.notEqual(value.buffer,input.buffer);
}
function capture(action){let threw=false,value;try{action()}catch(error){threw=true;value=error}return{threw,value}}

async function fixture(fault={}){
  let api,memory,entries=0,mints=0,drops=0,importsAfterReentry=0,blockedImport=false,primary,cleanupArmed=false,validatorReturned=0;
  const counters=()=>({entries,mints,drops,importsAfterReentry,blockedImport,validatorReturned});
  WebAssembly.instantiate=async(bytes,imports)=>{
    assert.deepEqual(new Uint8Array(bytes),wasm);
    const original=imports.env;
    const env={...original};
    env.spx_bytes_copy=(...args)=>{
      const carrier=original.spx_bytes_copy(...args);mints++;
      if(fault.kind==='post-mint')throw fault.value;
      if(fault.kind==='reenter-import'){
        assert(capture(()=>api.call('case.copy',input,1n)).threw);
        const attempted=capture(()=>{original.spx_bytes_copy(...args);importsAfterReentry++});
        blockedImport=attempted.threw;
      }
      return carrier;
    };
    env.spx_bytes_drop=(...args)=>{const value=original.spx_bytes_drop(...args);drops++;return value};
    const instance=await realInstantiate(compiled,{...imports,env});
    memory=instance.exports.memory;
    const exports={...instance.exports};
    for(const [name,fn] of Object.entries(exports))if(name.startsWith('spx_owned_v1_')){
      exports[name]=(...args)=>{
        entries++;
        if(fault.kind==='throw')throw fault.value;
        if(fault.kind==='status')return fault.value;
        if(fault.kind==='cleanup'){
          cleanupArmed=true;primary=fault.value;throw primary;
        }
        if(fault.kind==='utf8-corpus')exerciseValidator(original,memory);
        if(['utf8-stale','utf8-extent','utf8-memory','utf8-read'].includes(fault.kind)){
          return faultValidator(original,exports,memory,fault,()=>validatorReturned++);
        }
        if(fault.kind==='invalid-utf8'&&name===raw('case.text')){
          // Real arena allocation, then real facade consume and fatal decode.
          new Uint8Array(memory.buffer)[0]=255;
          const carrier=env.spx_bytes_copy(1n);
          new DataView(memory.buffer).setBigInt64(RESULT,carrier,true);
          return 0;
        }
        const status=fn(...args);
        if(status!==0){
          assert.equal(status,4);
          assert(new Uint8Array(memory.buffer,RESULT,resultSize).every(byte=>byte===POISON));
        }
        if(fault.kind==='reenter-publication'){
          assert.equal(status,0);
          assert(capture(()=>api.call('case.copy',input,1n)).threw);
          // Simulate an engine wrapper swallowing the nested exception and
          // offering a genuinely initialized successful result to the facade.
        }
        return status;
      };
    }
    return {instance:{exports}};
  };
  try{api=await instantiate(wasm)}finally{WebAssembly.instantiate=realInstantiate}
  return{api,counters,memory,cleanupArmed:()=>cleanupArmed,primary:()=>primary};
}

const baseline=await fixture();
assertSuccess(baseline.api);
const beforeInvalid=baseline.counters().entries;
for(const action of [
  ()=>baseline.api.call('missing'),
  ()=>baseline.api.call('case.copy'),
  ()=>baseline.api.call('case.copy',[],1n),
  ()=>baseline.api.call('case.copy',input,1),
  ()=>baseline.api.call('case.copy',new Uint8Array(65537),1n),
  ()=>baseline.api.call('case.utf8','\ud800'),
  ()=>baseline.api.call('case.utf8','\udc00'),
  ()=>baseline.api.call('case.utf8',input),
])assert(capture(action).threw);
assert.equal(baseline.counters().entries,beforeInvalid);
assertSuccess(baseline.api);
for(const id of ['case.before','case.copy']){
  const previous=baseline.counters();
  const failure=capture(()=>baseline.api.call(id,input,0n));
  assert(failure.threw);assert.equal(failure.value.status,4);
  const next=baseline.counters();
  assert.equal(next.entries,previous.entries+1);
  assert.equal(next.mints-previous.mints,id==='case.copy'?1:0);
  assert.equal(next.drops-previous.drops,id==='case.copy'?1:0);
  assertSuccess(baseline.api);
}
if(config.family==='variant'){
  assert.equal(baseline.api.call('case.none'),null);
  assert.deepEqual(baseline.api.call('case.err'),{ok:false,error:-7n});
}
if(config.family==='mixed'){
  assert.equal(baseline.api.call('case.flag',false),false);
  assert.equal(baseline.api.call('case.flag',true),true);
}
if(config.utf8)assert.equal(baseline.api.call('case.text'),'\ufeffA\0λ');
for(const text of ['', '\0', '\ufeff', 'A\0λ🙂', '\u007f\u0080\u07ff\u0800\ud7ff\ue000\uffff\u{10000}\u{10ffff}']){
  const expected=new TextEncoder().encode(text);
  assert.deepEqual(returnedBytes(baseline.api.call('case.utf8',text),expected.length),expected);
}
const validatorSubject=await fixture({kind:'utf8-corpus'});
assertSuccess(validatorSubject.api);assertSuccess(validatorSubject.api);

async function poisoned(fault,invoke=api=>api.call('case.copy',input,1n)){
  const subject=await fixture(fault);
  const outcome=capture(()=>invoke(subject.api));
  assert(outcome.threw,`${fault.kind} published a value`);
  if(Object.hasOwn(fault,'value')&&fault.kind!=='status')assert.strictEqual(outcome.value,fault.value,'first thrown value was replaced');
  assert.equal(subject.counters().entries,1);
  if(fault.kind==='post-mint')assert.equal(subject.counters().mints,1);
  if(['utf8-stale','utf8-extent','utf8-memory','utf8-read'].includes(fault.kind)){
    assert.equal(subject.counters().validatorReturned,0,'invalid authority was disguised as malformed UTF-8');
  }
  if(fault.kind==='reenter-import'){
    assert.equal(subject.counters().blockedImport,true);
    assert.equal(subject.counters().importsAfterReentry,0);
  }
  assert(capture(()=>subject.api.call('case.copy',input,1n)).threw,'poison was cleared');
  assert.equal(subject.counters().entries,1,'poisoned facade entered the engine again');
  return{subject,outcome};
}

for(const value of [new TypeError('post-entry type'),new RangeError('post-entry range'),null,undefined,false,0,'']){
  await poisoned({kind:'throw',value});
}
for(const value of [new TypeError('owner initialized'),new RangeError('owner initialized')])await poisoned({kind:'post-mint',value});
await poisoned({kind:'throw',value:Object.assign(new Error('forged marker'),{semapraxSemantic:true,status:4})});
let markerReads=0;
const getterError=Object.defineProperty(new Error('hostile marker getter'),'semapraxSemantic',{get(){markerReads++;return true}});
await poisoned({kind:'throw',value:getterError});assert.equal(markerReads,0);
for(const value of [-1,1.5,NaN,Infinity,11,99,'4',4n,undefined])await poisoned({kind:'status',value});
await poisoned({kind:'reenter-import'});
await poisoned({kind:'reenter-publication'});
for(const kind of ['utf8-stale','utf8-extent','utf8-memory'])await poisoned({kind});
await poisoned({kind:'utf8-read',value:new TypeError('authenticated UTF-8 read failed')});

// A cleanup throw is a simulated trusted-host failure, not a claim that this
// facade is a sandbox against co-resident prototype replacement.
const primary=Object.freeze({primary:'identity'}),cleanup=new RangeError('scratch cleanup');
const cleanupSubject=await fixture({kind:'cleanup',value:primary});
const realFill=Uint8Array.prototype.fill;
let cleanupCount=0;
Uint8Array.prototype.fill=function(...args){
  if(cleanupSubject.cleanupArmed()&&args[0]===0){cleanupCount++;throw cleanup}
  return Reflect.apply(realFill,this,args);
};
let cleanupOutcome;
try{cleanupOutcome=capture(()=>cleanupSubject.api.call('case.copy',input,1n))}finally{Uint8Array.prototype.fill=realFill}
assert(cleanupOutcome.threw);assert.strictEqual(cleanupOutcome.value,primary);
assert.equal(cleanupCount,1);assert(capture(()=>cleanupSubject.api.call('case.copy',input,1n)).threw);
assert.equal(cleanupSubject.counters().entries,1);

// View construction happens after validated admission but still belongs to
// the protected active invocation; failure must release busy without reuse.
const viewSubject=await fixture(),RealView=DataView,viewError=new TypeError('view construction');
globalThis.DataView=new Proxy(RealView,{construct(){throw viewError}});
let viewOutcome;
try{viewOutcome=capture(()=>viewSubject.api.call('case.copy',input,1n))}finally{globalThis.DataView=RealView}
assert(viewOutcome.threw);assert.strictEqual(viewOutcome.value,viewError);
assert.equal(viewSubject.counters().entries,0);
assert(capture(()=>viewSubject.api.call('case.copy',input,1n)).threw);
assert.equal(viewSubject.counters().entries,0);

if(config.utf8){
  const realDelete=Map.prototype.delete,realDecode=TextDecoder.prototype.decode;
  let consumed=0,decodes=0;
  Map.prototype.delete=function(key){
    const value=this.get(key),matched=typeof key==='number'&&value instanceof Uint8Array&&value.length===1&&value[0]===255;
    const deleted=Reflect.apply(realDelete,this,[key]);
    if(matched&&deleted)consumed++;
    return deleted;
  };
  TextDecoder.prototype.decode=function(value,...rest){
    if(value instanceof Uint8Array&&value.length===1&&value[0]===255){assert.equal(consumed,1);decodes++}
    return Reflect.apply(realDecode,this,[value,...rest]);
  };
  let observed;
  try{observed=await poisoned({kind:'invalid-utf8'},api=>api.call('case.text'))}
  finally{Map.prototype.delete=realDelete;TextDecoder.prototype.decode=realDecode}
  const {subject,outcome}=observed;
  assert(outcome.value instanceof TypeError);
  assert.equal(consumed,1);assert.equal(decodes,1);
  assert.equal(subject.counters().mints,1);
  // Bytes on the same real package remain bytes, including invalid UTF-8.
  assertSuccess(baseline.api);
}
console.log('owned-failure-fsm-ok');
