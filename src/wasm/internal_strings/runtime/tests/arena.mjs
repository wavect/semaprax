// Direct private-host fixture, not an exported runtime hook or compiler proof.
const EXPECTED_BYTES=4;
{
  const source=new Uint8Array([1,2,3,4]),copy=snapshotModule(source);
  source[0]=9;assert.deepEqual([...copy],[1,2,3,4]);
  let getters=0;
  Object.defineProperty(source,"byteLength",{get(){getters++;throw new Error("must not call user getter")}});
  assert.deepEqual([...snapshotModule(source)],[9,2,3,4]);assert.equal(getters,0);
  Object.defineProperty(source,"constructor",{get(){getters++;throw new Error("must not call user constructor")}});
  assert.throws(()=>snapshotModule(source),TypeError);assert.equal(getters,0);
  assert.throws(()=>snapshotModule(new Uint8Array(5)),RangeError);
  assert.throws(()=>snapshotModule(new (class extends Uint8Array {})(4)),TypeError);
  const disguised=new Uint16Array(2);Object.setPrototypeOf(disguised,Uint8Array.prototype);
  assert.throws(()=>snapshotModule(disguised),TypeError);
  assert.throws(()=>snapshotModule(new Proxy(new Uint8Array(4),{})),TypeError);
  assert.throws(()=>snapshotModule(new Uint8Array(new SharedArrayBuffer(4))),TypeError);
  const detached=new Uint8Array(4);structuredClone(detached.buffer,{transfer:[detached.buffer]});
  assert.throws(()=>snapshotModule(detached),TypeError);
}
let DESCRIPTOR,tokenPayloadAllocations=0;
function fixture(overrides={},factory=createArena){
  DESCRIPTOR={limits:{max_string_bytes:65536,max_live_bytes:1048576,max_cumulative_bytes:16777216,max_live_owners:8,...overrides}};
  let poisoned=false;
  const fail=()=>{poisoned=true;throw new Error("poison")};
  const arena=factory(fail,()=>poisoned),memory=new WebAssembly.Memory({initial:4,maximum:4});
  arena.bind(memory);arena.begin();
  return {arena,i:arena.imports,memory,poisoned:()=>poisoned};
}
function literal(f,bytes){new Uint8Array(f.memory.buffer,196608,bytes.length).set(bytes);return f.i.literal(196608,bytes.length)}
function capacity(overrides,expected){
  const f=fixture(overrides),h=literal(f,[120]);
  assert.equal(h,0n);assert.equal(f.arena.settle(11),expected);assert.equal(f.poisoned(),false);
  f.arena.begin();const empty=literal(f,[]);assert.notEqual(empty,0n);f.i.drop(empty);assert.equal(f.arena.settle(0),null);
}
capacity({max_string_bytes:0},"value_bytes");
capacity({max_live_bytes:0},"live_bytes");
capacity({max_cumulative_bytes:0},"cumulative_bytes");
{
  const f=fixture({},tokenArena),last=literal(f,[120]);
  assert.equal(last>>32n,0x7fffffffn);assert.equal(tokenPayloadAllocations,1);
  f.i.drop(last);assert.equal(literal(f,[120]),0n);assert.equal(f.arena.settle(11),"tokens");
  assert.equal(tokenPayloadAllocations,1);
  f.arena.begin();assert.equal(literal(f,[]),0n);assert.equal(f.arena.settle(11),"tokens");
  assert.equal(tokenPayloadAllocations,1);assert.equal(f.poisoned(),false);
}
{
  const f=fixture({max_live_owners:1,max_string_bytes:0,max_live_bytes:0,max_cumulative_bytes:0});
  const empty=literal(f,[]);assert.notEqual(empty,0n);
  assert.equal(f.i.clone(empty),0n);f.i.drop(empty);assert.equal(f.arena.settle(11),"owners");
  f.arena.begin();const next=literal(f,[]);assert.notEqual(next,empty);f.i.drop(next);f.arena.settle(0);
}
{
  const f=fixture({max_live_bytes:3,max_cumulative_bytes:3}),a=literal(f,[97]),b=literal(f,[98]);
  assert.equal(f.i.concat(a,b),0n); // Inputs AND output count toward peak live bytes.
  assert.equal(f.i.byte_len(a),1n);assert.equal(f.i.byte_len(b),1n);
  f.i.drop(a);f.i.drop(b);assert.equal(f.arena.settle(11),"live_bytes");
}
{
  const f=fixture({max_string_bytes:2,max_live_bytes:4,max_cumulative_bytes:4}),a=literal(f,[97]),b=f.i.clone(a),c=f.i.concat(a,b);
  assert.equal(f.i.byte_len(c),2n);f.i.drop(a);f.i.drop(b);f.i.drop(c);f.arena.settle(0);
  f.arena.begin();const exact=literal(f,[97,98]);f.i.drop(exact);
  const exactAgain=literal(f,[97,98]);f.i.drop(exactAgain);
  assert.equal(literal(f,[97]),0n);assert.equal(f.arena.settle(11),"cumulative_bytes");
}
{
  const f=fixture({max_string_bytes:3});assert.equal(f.i.from_char(0x1f642),0n);assert.equal(f.arena.settle(11),"value_bytes");
}
{
  const f=fixture(),a=literal(f,[0,0xc3,0xa9,0xf0,0x9f,0x99,0x82]),b=f.i.clone(a),nul=f.i.from_char(0),smile=f.i.from_char(0x1f642);
  assert.equal(f.i.byte_len(a),7n);assert.equal(f.i.char_len(a),3n);assert.equal(f.i.eq(a,b),1);
  assert.equal(f.i.starts_with(a,nul),1);assert.equal(f.i.contains(a,smile),1);assert.equal(f.i.contains(smile,a),0);
  for(const h of [a,b,nul,smile])f.i.drop(h);f.arena.settle(0);
}
for(const bytes of [[0xc0,0x80],[0xed,0xa0,0x80],[0xf4,0x90,0x80,0x80],[0xe2,0x82],[0x80]]){
  const f=fixture();assert.throws(()=>literal(f,bytes),/poison/);assert.equal(f.poisoned(),true);
}
for(const scalar of [-1,0xd800,0x110000]){
  const f=fixture();assert.throws(()=>f.i.from_char(scalar),/poison/);assert.equal(f.poisoned(),true);
}
for(const bad of [0n,-1n,0x8000000000000000n,1n,1,undefined]){
  const f=fixture();assert.throws(()=>f.i.drop(bad),/poison/);assert.equal(f.poisoned(),true);
}
{
  const f=fixture(),h=literal(f,[120]);assert.throws(()=>f.i.drop(h+1n),/poison/);assert.equal(f.poisoned(),true);
  assert.throws(()=>f.i.byte_len(h),/poison/);assert.throws(()=>f.i.drop(h),/poison/);assert.throws(()=>f.arena.begin(),/poison/);
}
{
  const f=fixture(),h=literal(f,[120]);f.i.drop(h);assert.throws(()=>f.i.drop(h),/poison/);assert.equal(f.poisoned(),true);
}
{
  const f=fixture();literal(f,[]);assert.throws(()=>f.arena.settle(0),/poison/);assert.equal(f.poisoned(),true);
}
{
  const f=fixture();assert.throws(()=>f.arena.settle(11),/poison/);assert.equal(f.poisoned(),true);
}
{
  const f=fixture();assert.throws(()=>f.i.literal(0,1),/poison/);assert.equal(f.poisoned(),true);
}
{
  const f=fixture(),original=Map.prototype.set;
  try{Map.prototype.set=()=>{throw new Error("unexpected host allocation failure")};assert.throws(()=>literal(f,[120]),/poison/)}
  finally{Map.prototype.set=original}
  assert.equal(f.poisoned(),true);
}
