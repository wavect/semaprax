// Independent raw-ABI oracle: intentionally not the production arena.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
const bytes=readFileSync('program.wasm');
const descriptor=JSON.parse(readFileSync('program.json','utf8'));
const cases=JSON.parse(readFileSync('cases.json','utf8'));
const module=await WebAssembly.compile(bytes);
const decoder=new TextDecoder('utf-8',{fatal:true}),encoder=new TextEncoder();
const names=['literal','clone','concat','from_char','byte_len','char_len','eq','starts_with','contains','drop'];
assert.deepEqual(WebAssembly.Module.imports(module).map(i=>[i.module,i.name,i.kind]),names.map(n=>['semaprax.internal-strings.v1',n,'function']));
const expectedExports=['memory','__spx_stack_pointer',...descriptor.exports.map(e=>e.wasm_export)].sort();
assert.deepEqual(WebAssembly.Module.exports(module).map(e=>e.name).sort(),expectedExports);

async function oracle(){
  const live=new Map(),minted=new Set(),dropped=new Set();
  let serial=1n,attempts=0,refuseAt=Infinity,refused=false,peak=0,liveBytes=0,peakBytes=0;
  let instance;
  function get(handle){assert(live.has(handle),'unknown, stale or duplicated owner');return live.get(handle)}
  function mint(bytes){
    assert(!refused,'compiler continued allocation after capacity refusal');
    if(++attempts===refuseAt){refused=true;return 0n}
    const copy=Uint8Array.from(bytes),handle=(serial++<<32n)|BigInt(copy.length);
    assert(!minted.has(handle));minted.add(handle);live.set(handle,copy);
    liveBytes+=copy.length;peak=Math.max(peak,live.size);peakBytes=Math.max(peakBytes,liveBytes);
    return handle;
  }
  const text=h=>decoder.decode(get(h));
  const imports={
    literal(pointer,length){
      assert(pointer>=196608&&length>=0&&pointer+length<=262144);
      return mint(new Uint8Array(instance.exports.memory.buffer,pointer,length));
    },
    clone(handle){return mint(get(handle))},
    concat(a,b){const first=get(a),last=get(b);const result=new Uint8Array(first.length+last.length);result.set(first);result.set(last,first.length);return mint(result)},
    from_char(scalar){assert(scalar>=0&&scalar<=0x10ffff&&(scalar<0xd800||scalar>0xdfff));return mint(encoder.encode(String.fromCodePoint(scalar)))},
    byte_len(handle){return BigInt(get(handle).length)},
    char_len(handle){return BigInt([...text(handle)].length)},
    eq(a,b){return text(a)===text(b)?1:0},
    starts_with(a,b){return text(a).startsWith(text(b))?1:0},
    contains(a,b){return text(a).includes(text(b))?1:0},
    drop(handle){const bytes=get(handle);assert(!dropped.has(handle));dropped.add(handle);live.delete(handle);liveBytes-=bytes.length},
  };
  instance=await WebAssembly.instantiate(module,{'semaprax.internal-strings.v1':imports});
  assert.equal(instance.exports.memory.buffer.byteLength,262144);
  // The fixed maximum must reject actual growth, not merely advertise four pages.
  assert.throws(()=>instance.exports.memory.grow(1),RangeError);
  function call(id,args=[],fail=Infinity){
    assert.equal(live.size,0);assert.equal(liveBytes,0);
    attempts=0;refused=false;refuseAt=fail;peak=0;peakBytes=0;
    const fact=descriptor.exports.find(e=>e.stable_id===id);assert(fact);
    const view=new DataView(instance.exports.memory.buffer);
    // Wrapper, not the test, must erase this poison before invoking the callee.
    view.setBigUint64(65536,0xfeedfacecafebeefn,true);
    const status=instance.exports[fact.wasm_export](...args);
    assert.equal(instance.exports.__spx_stack_pointer.value,65536);
    assert.equal(live.size,0,`${id}: live owner escaped status ${status}`);
    assert.equal(liveBytes,0);assert.equal(minted.size,dropped.size);
    assert(peak<=descriptor.derived_owner_capacity);
    if(refused)assert.equal(status,11);
    else assert(status>=0&&status<=10);
    if(status!==0)assert.equal(view.getBigUint64(65536,true),0n);
    let outcome;
    if(status===0){
      if(fact.result==='bool'){
        const value=view.getUint32(65536,true);assert(value===0||value===1);
        assert.equal(view.getUint32(65540,true),0);outcome=`ok|${value===1}`;
      }else outcome=`ok|${view.getBigInt64(65536,true)}`;
    }else if(status===11)outcome='capacity';
    else outcome=status<=8?`semaprax.arithmetic.v1|${status}`:`semaprax.contract.v1|${status-8}`;
    return {outcome,attempts,peak,peakBytes};
  }
  return {call};
}
const runtime=await oracle();
for(const [id,expected] of cases){
  const baseline=runtime.call(id);assert.equal(baseline.outcome,expected);
  // Every reached mint in success and language-failure paths refuses once.
  // Each refusal must settle this same instance, including nested/late args.
  for(let ordinal=1;ordinal<=baseline.attempts;ordinal++){
    assert.equal(runtime.call(id,[],ordinal).outcome,'capacity');
    assert.equal(runtime.call('case.scalar',[41n]).outcome,'ok|42');
    assert.equal(runtime.call(id).outcome,expected);
  }
  if(id==='case.lazy')assert.equal(baseline.attempts,1,'lazy branches executed');
  if(id==='case.loop')assert(baseline.attempts>baseline.peak,'loop did not release between helper calls');
  process.stdout.write(`${id}|${expected}\n`);
}
for(const flag of [0,1]){
  const expected=`ok|${flag===1}`,baseline=runtime.call('case.bool',[flag]);
  assert.equal(baseline.outcome,expected);
  for(let ordinal=1;ordinal<=baseline.attempts;ordinal++){
    assert.equal(runtime.call('case.bool',[flag],ordinal).outcome,'capacity');
    assert.equal(runtime.call('case.bool',[flag]).outcome,expected);
  }
}
