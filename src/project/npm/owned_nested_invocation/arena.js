// Project-v11 private arena with one atomic multi-handle extraction boundary.
function createArena(){
  const entries=new Map();let nextToken=1,instance=null,poisoned=false;
  function poison(){poisoned=true}
  function check(){if(poisoned)throw new Error("SEMAPRAX owned-data runtime is poisoned")}
  function guarded(operation){return (...args)=>{check();try{const result=operation(...args);check();return result}catch(error){poison();throw error}}}
  function decode(carrier){
    check();if(typeof carrier!=="bigint")throw new Error("SEMAPRAX owned carrier is not i64");
    const word=BigInt.asUintN(64,carrier),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn),token=root&0x7fffffff;
    if(length>65536)throw new Error("SEMAPRAX owned carrier length invariant");
    return {length,root,token,tagged:(root&0x80000000)!==0,range:((root&0xc0000000)>>>0)===0x40000000};
  }
  function resolve(value){
    check();if(!value.tagged||value.token===0)throw new Error("SEMAPRAX owned carrier token invariant");
    const bytes=entries.get(value.token);if(!(bytes instanceof Uint8Array)||bytes.byteLength!==value.length)throw new Error("SEMAPRAX stale or wrong-length owned carrier");
    check();return bytes;
  }
  function memory(){check();const value=instance?.exports.memory;if(!(value instanceof WebAssembly.Memory))throw new Error("SEMAPRAX borrowed carrier memory invariant");const bytes=new Uint8Array(value.buffer);check();return bytes}
  function allocate(bytes){
    check();if(entries.size>=__SPX_CAPACITY__||nextToken>0x7fffffff)throw new Error("SEMAPRAX owned arena exhausted");
    const token=nextToken++,owned=new Uint8Array(bytes);check();entries.set(token,owned);check();return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(owned.byteLength));
  }
  function read(carrier){
    check();const value=decode(carrier);if(value.tagged)return resolve(value);const bytes=memory();
    if(value.range){
      const pointer=(value.root&0xffff)*8;if(pointer>131072-32)throw new Error("SEMAPRAX byte range descriptor bounds invariant");
      const descriptor=new DataView(bytes.buffer,bytes.byteOffset+pointer,32),identity=descriptor.getUint32(0,true),self=descriptor.getUint32(4,true),carrierIdentity=(value.root>>>16)&0x1fff;
      if(identity===0||identity!==carrierIdentity||self!==pointer)throw new Error("SEMAPRAX byte range descriptor identity invariant");
      const ultimate=decode(descriptor.getBigInt64(8,true)),offset=descriptor.getBigUint64(16,true),length=descriptor.getBigUint64(24,true);
      if(length!==BigInt(value.length)||ultimate.range)throw new Error("SEMAPRAX byte range descriptor invariant");
      if(offset>BigInt(ultimate.length)||length>BigInt(ultimate.length)-offset)throw new Error("SEMAPRAX byte range descriptor extent invariant");
      let base;if(ultimate.tagged)base=resolve(ultimate);else{if(ultimate.root>bytes.byteLength-ultimate.length)throw new Error("SEMAPRAX borrowed carrier range invariant");base=bytes.subarray(ultimate.root,ultimate.root+ultimate.length)}
      const start=Number(offset);check();return base.subarray(start,start+Number(length));
    }
    if(value.root>bytes.byteLength-value.length)throw new Error("SEMAPRAX borrowed carrier range invariant");check();return bytes.subarray(value.root,value.root+value.length);
  }
  function validUtf8(bytes){
    for(let i=0;i<bytes.length;){const first=bytes[i++];let extra,min,scalar;if(first<128)continue;if(first>=194&&first<=223){extra=1;min=128;scalar=first&31}else if(first>=224&&first<=239){extra=2;min=2048;scalar=first&15}else if(first>=240&&first<=244){extra=3;min=65536;scalar=first&7}else return 0;if(i+extra>bytes.length)return 0;for(let j=0;j<extra;j++){const byte=bytes[i++];if((byte&192)!==128)return 0;scalar=(scalar<<6)|(byte&63)}if(scalar<min||scalar>1114111||(scalar>=55296&&scalar<=57343))return 0}return 1;
  }
  function consumeMany(carriers){
    check();if(!Array.isArray(carriers)||carriers.length===0||carriers.length>256)throw new Error("SEMAPRAX owned carrier batch invariant");
    const resolved=[],tokens=new Set();let total=0;
    // Complete validation precedes allocation and settlement.
    for(const carrier of carriers){const value=decode(carrier),bytes=resolve(value);if(tokens.has(value.token))throw new Error("SEMAPRAX duplicate owned carrier token");tokens.add(value.token);total+=value.length;if(!Number.isSafeInteger(total)||total>65536)throw new Error("SEMAPRAX cumulative owned output invariant");resolved.push({token:value.token,bytes})}
    const copies=[];for(const item of resolved){copies.push(new Uint8Array(item.bytes));check()}
    // Map deletion for an authenticated extant key is the sole non-failing commit.
    for(const item of resolved)entries.delete(item.token);check();return copies;
  }
  return Object.freeze({
    imports:Object.freeze({spx_bytes_copy:guarded(c=>allocate(read(c))),spx_bytes_get:guarded((c,i)=>{const b=read(c),n=BigInt.asUintN(64,i);return n>=BigInt(b.byteLength)?-1:b[Number(n)]}),spx_bytes_drop:guarded(c=>{const value=decode(c);resolve(value);check();entries.delete(value.token)}),spx_bytes_as_slice:guarded(c=>{read(c);return BigInt.asIntN(64,c)}),spx_owned_utf8_validate_v1:guarded((offset,length)=>validUtf8(read((BigInt(offset)<<32n)|BigInt(length))))}),
    bind:guarded(value=>{if(instance!==null)throw new Error("SEMAPRAX arena already bound");instance=value}),begin:guarded(()=>{if(entries.size!==0)throw new Error("SEMAPRAX arena entered unsettled")}),consumeMany:guarded(consumeMany),settle:guarded(()=>{if(entries.size!==0)throw new Error("SEMAPRAX arena did not settle")}),check,poison,
  });
}
