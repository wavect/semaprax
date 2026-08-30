import assert from 'node:assert/strict';

// Boundary scalar encodings, including BOM as data, empty/NUL and supplementary
// scalars. These go through the real generated arena import, not a copied codec.
export function exerciseValidator(env,memory){
  const bytes=new Uint8Array(memory.buffer),saved=bytes.slice(0,32);
  const valid=[[],[0],[65],[0xef,0xbb,0xbf],[0xc2,0x80],[0xdf,0xbf],
    [0xe0,0xa0,0x80],[0xed,0x9f,0xbf],[0xee,0x80,0x80],
    [0xf0,0x90,0x80,0x80],[0xf4,0x8f,0xbf,0xbf],
    [65,0,0xce,0xbb,0xf0,0x9f,0x99,0x82]];
  const malformed=[[0x80],[0xc0,0xaf],[0xc1,0x80],[0xc2],
    [0xe0,0x9f,0xbf],[0xed,0xa0,0x80],[0xe2,0x82],[0xe0,0x28,0xa1],
    [0xf0,0x8f,0xbf,0xbf],[0xf4,0x90,0x80,0x80],[0xf5,0x80,0x80,0x80],
    [0xf0,0x90,0x80],[0xff]];
  try{
    for(const [cases,expected] of [[valid,1],[malformed,0]])for(const value of cases){
      bytes.set(value,0);
      assert.equal(env.spx_owned_utf8_validate_v1(0,value.length),expected);
    }
  }finally{bytes.set(saved,0)}
}

// Faults enter the real import while its real facade invocation is active.
// Prototype replacement is explicitly simulated trusted-host failure, not a
// claim of isolation from a hostile JavaScript realm.
export function faultValidator(env,exports,memory,fault,onReturned){
  let offset=0,length=1;
  if(fault.kind==='utf8-stale')offset=0x80000001;
  if(fault.kind==='utf8-extent')offset=memory.buffer.byteLength;
  if(fault.kind==='utf8-memory')exports.memory={};
  const original=Uint8Array.prototype.subarray;
  if(fault.kind==='utf8-read')Uint8Array.prototype.subarray=function(){throw fault.value};
  try{
    const result=env.spx_owned_utf8_validate_v1(offset,length);
    onReturned();
    return result;
  }finally{Uint8Array.prototype.subarray=original}
}
