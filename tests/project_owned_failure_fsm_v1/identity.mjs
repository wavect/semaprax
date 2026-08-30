import assert from 'node:assert/strict';

const MESSAGE='SEMAPRAX export identity must be a string';
const RESULT=65536,SENTINEL=0x6d;
function capture(action){let threw=false,value;try{action()}catch(error){threw=true;value=error}return{threw,value}}

export function exerciseIdentities(subject,assertSuccess,resultSize){
  let hooks=0;
  const touched=()=>{hooks++;throw new Error('identity hook must not run')};
  const getter=key=>Object.defineProperty({},key,{get:touched});
  const conversion={
    [Symbol.toPrimitive](){hooks++;try{assertSuccess(subject.api)}catch{}return 'case.copy'},
  };
  const revoked=Proxy.revocable({},{});revoked.revoke();
  const revokedCallable=Proxy.revocable(()=>{},{});revokedCallable.revoke();
  const handler={get:touched,getPrototypeOf:touched,ownKeys:touched,getOwnPropertyDescriptor:touched,apply:touched};
  const identities=[
    undefined,null,false,true,0,-0,1,NaN,Infinity,-Infinity,0n,1n,Symbol('case.copy'),
    new String('case.copy'),Object.create(null),[],{},()=>{hooks++},
    getter(Symbol.toPrimitive),getter('toString'),getter('valueOf'),
    {[Symbol.toPrimitive]:touched},{toString:touched,valueOf:touched},conversion,
    new Proxy({},handler),new Proxy(()=>{},handler),revoked.proxy,revokedCallable.proxy,
  ];
  function reject(id,message){
    const scratch=new Uint8Array(subject.memory.buffer,0,RESULT);
    const result=new Uint8Array(subject.memory.buffer,RESULT,resultSize);
    scratch.fill(SENTINEL);result.fill(SENTINEL);
    const before=subject.counters();
    const outcome=capture(()=>subject.api.call(id));
    assert.equal(outcome.threw,true);
    assert(outcome.value instanceof RangeError);
    assert.equal(outcome.value.message,message);
    assert.equal(hooks,0,'identity rejection invoked caller code');
    assert.deepEqual(subject.counters(),before,'identity rejection entered Wasm or touched its imports');
    assert(scratch.every(byte=>byte===SENTINEL),'identity rejection changed scratch');
    assert(result.every(byte=>byte===SENTINEL),'identity rejection changed result storage');
    assertSuccess(subject.api);
    const after=subject.counters();
    assert.equal(after.entries,before.entries+1,'recovery must enter an observed real export');
    assert.equal(after.mints,before.mints+1,'recovery must mint its real copied owner');
    assert.equal(after.drops,before.drops);
    assert.equal(hooks,0);
  }
  // Every rejection is followed by a positively counted call on this same instance.
  for(const id of identities)reject(id,MESSAGE);
  for(const id of ['', 'missing','__proto__','constructor','toString','hasOwnProperty']){
    reject(id,`unknown SEMAPRAX export: ${id}`);
  }
}

export async function exerciseBusyIdentities(poisoned){
  let hooks=0;
  const object=Object.defineProperty({},Symbol.toPrimitive,{get(){hooks++;throw new Error('busy identity hook')}});
  for(const identity of [null,Symbol('nested'),object]){
    for(const kind of ['reenter-import','reenter-publication'])await poisoned({kind,identity});
  }
  assert.equal(hooks,0,'busy rejection must precede identity inspection');
}
