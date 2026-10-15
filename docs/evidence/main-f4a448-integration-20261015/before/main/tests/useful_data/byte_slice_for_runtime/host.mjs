// Independent aggregate-byte ABI oracle, not the legacy scalar String facade.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const bytes=await readFile(process.argv[2]),kind=process.argv[3],expected=BigInt(process.argv[4]);
const catalog=/^catalog-(failure-)?(0|[1-9][0-9]*)$/.exec(kind);
const failure=kind==='failure'||Boolean(catalog?.[1]);
const expectedAllocated=kind==='success'?256+new TextEncoder().encode('SEMAPRAX').length:kind==='failure'?3:catalog?Number(catalog[2]):0;
const module=await WebAssembly.compile(bytes);
const names=['spx_add','spx_sub','spx_mul','spx_div','spx_rem','spx_neg','spx_contract_fail','spx_bytes_copy','spx_bytes_get','spx_bytes_drop','spx_bytes_as_slice'];
assert.deepEqual(WebAssembly.Module.imports(module),names.map(name=>({module:'env',name,kind:'function'})));
const MIN=-(1n<<63n),MAX=(1n<<63n)-1n;
const checked=n=>{assert(n>=MIN&&n<=MAX,'arithmetic overflow');return n};
for(let run=0;run<4;run++){
  let instance,serial=1,copies=0,allocated=0,drops=0,reports=0;
  const owners=new Map();
  const decode=carrier=>{
    assert.equal(typeof carrier,'bigint');
    const word=BigInt.asUintN(64,carrier),root=Number(word>>32n),length=Number(word&0xffffffffn);
    assert(length<=((root&0xc0000000)===0?65536:131072),'length');
    return {root,length};
  };
  const memory=()=>new Uint8Array(instance.exports.__spx_byte_memory.buffer);
  const read=carrier=>{
    const d=decode(carrier);
    if((d.root&0x80000000)!==0){
      const value=owners.get(d.root&0x7fffffff);
      assert(value&&value.length===d.length,'stale or malformed owner');return value;
    }
    if((d.root&0xc0000000)===0x40000000){
      const pointer=(d.root&0xffff)*8;
      assert(pointer<=131072-32,'descriptor bounds');
      const v=new DataView(memory().buffer,pointer,32),identity=v.getUint32(0,true);
      assert(identity!==0&&identity===((d.root>>>16)&0x1fff)&&v.getUint32(4,true)===pointer,'descriptor identity');
      const root=v.getBigInt64(8,true),r=decode(root),offset=v.getBigUint64(16,true),length=v.getBigUint64(24,true);
      assert((r.root&0xc0000000)!==0x40000000,'nested descriptor');
      assert(length===BigInt(d.length)&&offset<=BigInt(r.length)&&length<=BigInt(r.length)-offset,'descriptor extent');
      return read(root).subarray(Number(offset),Number(offset+length));
    }
    const m=memory();
    assert(d.root<=131072-d.length || (m.length===262144&&d.root>=196608&&d.root<262144&&d.root<=262144-d.length),'fixed bounds');
    return m.subarray(d.root,d.root+d.length);
  };
  const env={
    spx_add:(a,b)=>checked(a+b),spx_sub:(a,b)=>checked(a-b),spx_mul:(a,b)=>checked(a*b),
    spx_div:(a,b)=>{assert(b!==0n);return checked(a/b)},spx_rem:(a,b)=>{assert(b!==0n);return a%b},spx_neg:a=>checked(-a),
    spx_contract_fail:status=>{
      assert(failure);assert.equal(status,9,'requires status contract.v1/code1');
      assert.equal(++reports,1,'exactly one selected report');
      assert.equal(owners.size,0,'report follows complete non-result cleanup');
      assert.equal(copies,1);assert.equal(drops,1);assert.equal(allocated,expectedAllocated);
      throw Error('contract');
    },
    spx_bytes_copy:carrier=>{
      assert(kind!=='view','view traversal called copy');
      const source=read(carrier);assert(owners.size<16,'owner capacity');
      const value=Uint8Array.from(source),token=serial++;
      owners.set(token,value);copies++;allocated+=value.length;
      return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(value.length));
    },
    spx_bytes_get:(carrier,index)=>{const value=read(carrier),n=BigInt.asUintN(64,index);return n<BigInt(value.length)?value[Number(n)]:-1},
    spx_bytes_drop:carrier=>{const d=decode(carrier);read(carrier);assert((d.root&0x80000000)!==0&&owners.delete(d.root&0x7fffffff),'drop');drops++},
    spx_bytes_as_slice:carrier=>{read(carrier);return carrier},
  };
  instance=await WebAssembly.instantiate(module,{env});
  // Host authority checks themselves must not mint a replacement payload.
  assert.throws(()=>read(BigInt.asIntN(64,0x80000001n<<32n)),/owner/);
  assert.throws(()=>read((0x40010000n<<32n)|1n),/descriptor/);
  assert.throws(()=>env.spx_bytes_drop(0n),/drop/);
  assert.equal(copies,0);assert.equal(allocated,0);assert.equal(owners.size,0);
  let value,error;
  try{value=instance.exports.semaprax_main()}catch(e){error=e}
  if(failure){assert(error,'body failure returned');assert.match(error.message,/^contract$/);assert.equal(value,undefined)}
  else {assert.equal(error,undefined);assert.equal(value,expected)}
  assert.equal(reports,failure?1:0);
  assert.equal(owners.size,0,'all reached owned carriers settle');
  const count=kind==='success'?2:kind==='failure'||catalog?1:0;
  assert.equal(copies,count);assert.equal(drops,count);
  // Complete success source copies the 256-byte array and 8-byte SEMAPRAX String.
  assert.equal(allocated,expectedAllocated);
}
console.log('byte-slice-for-ok');
