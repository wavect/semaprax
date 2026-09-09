import fs from 'node:fs';
import assert from 'node:assert/strict';
const [path,profile]=process.argv.slice(2);
const module=new WebAssembly.Module(fs.readFileSync(path));
let instance,mode='valid';
const memory=()=>instance.exports.memory??instance.exports.__spx_byte_memory;
const bytes=()=>new Uint8Array(memory().buffer);
const outCount=(p,value)=>new DataView(memory().buffer).setInt32(p,value,true);
const out=(p,value)=>new DataView(memory().buffer).setBigInt64(p,BigInt.asIntN(64,value),true);
const carrier=(root,len)=>(BigInt(root>>>0)<<32n)|BigInt(len>>>0);
const imports={};
for(const item of WebAssembly.Module.imports(module)) {
  assert.equal(item.kind,'function');
  imports[item.module]??={};
  imports[item.module][item.name]=()=>{throw Error(`unexpected import ${item.name}`)};
}
Object.assign(imports.env,{
  spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
  spx_environment_len_v1:p=>{if(mode==='none')return 4;if(mode==='len-unwritten')return 0;outCount(p,mode==='len257'?257:profile==='empty'?0:profile==='lookup'?2:1);return 0},
  spx_environment_name_utf8_v1:(index,p)=>{
    assert.equal(index,0n);
    if(mode.startsWith('status'))return Number(mode.slice(6));
    if(mode==='unknown')return 99;
    bytes().set([65],512);
    if(mode==='tagged')out(p,carrier(0x80000001,1));
    else if(mode==='outside')out(p,carrier(65535,2));
    else if(mode==='emptyname')out(p,carrier(512,0));
    else {if(mode==='equals')bytes()[512]=61;if(mode==='nul')bytes()[512]=0;out(p,carrier(512,1));}
    return 0;
  },
  spx_environment_value_utf8_v1:(index,p)=>{
    assert.equal(index,profile==='lookup'?1n:0n);
    if(mode==='value-unwritten')return 0;
    if(profile==='emptyvalue'){out(p,0n);return 0;}
    if(profile==='boundary'){bytes().fill(65,1,65536);out(p,carrier(1,65535));return 0;}
    const value=mode==='utf8'?[0xc0,0x80]:[0xc3,0xa9];
    bytes().set(value,1024);out(p,carrier(1024,value.length));return 0;
  },
});
instance=new WebAssembly.Instance(module,imports);
const symbol='spx_data_'+Buffer.from('environment.run').toString('hex');
assert.equal(typeof instance.exports[symbol],'function');
const envStatus=()=>Number(instance.exports.__spx_environment_status_v1.value);
const dataStatus=()=>Number(instance.exports.__spx_data_status_v1.value);
function success() {mode='valid';assert.equal(instance.exports[symbol](),1);assert.equal(envStatus(),0);assert.equal(dataStatus(),0);}
for(let repeat=0;repeat<2;repeat++) {
  success();mode='none';assert.equal(instance.exports[symbol](),0);assert.equal(envStatus(),4);success();
  const hostileModes=['len-unwritten','len257'];
  if(profile!=='empty')hostileModes.push('value-unwritten');
  if(profile==='lookup') {
    for(let code=1;code<=4;code++) {mode=`status${code}`;assert.equal(instance.exports[symbol](),0);assert.equal(envStatus(),code);success();}
    hostileModes.push('tagged','outside','emptyname','equals','nul','utf8','unknown');
  }
  for(const hostile of hostileModes) {
    mode=hostile;
    let trapped=false,value;
    try {value=instance.exports[symbol]();} catch(error) {if(!(error instanceof WebAssembly.RuntimeError))throw error;trapped=true;}
    assert.ok(trapped||(value===0&&dataStatus()!==0),`hostile ${hostile} was published`);
    success();
  }
}
console.log('environment carriers verified');
