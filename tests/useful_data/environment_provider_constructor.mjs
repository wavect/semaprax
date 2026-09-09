import assert from 'node:assert/strict';
const memory=new WebAssembly.Memory({initial:2});
const view=()=>new DataView(memory.buffer),pointer=65536;
const read=()=>view().getBigInt64(pointer,true);
const decode=value=>{const word=BigInt.asUintN(64,value);return new Uint8Array(memory.buffer,Number(word>>32n),Number(word&0xffffffffn))};
const raw=new Uint8Array([0xc3,0xa9]),stdin=new Uint8Array([7]);
const entries=[['Z',raw],['A','alpha']];
const provider=createEnvironmentProvider({environment:entries,arguments:['argument'],stdin});
raw.fill(0);stdin.fill(0);entries[0][0]='changed';entries.push(['extra','']);
provider.attach(memory);
assert.equal(provider.byteLength,18);
assert.equal(provider.imports.spx_environment_len_v1(pointer),0);assert.equal(view().getUint32(pointer,true),2);
assert.equal(provider.imports.spx_environment_name_utf8_v1(0n,pointer),0);assert.equal(new TextDecoder().decode(decode(read())),'A');
assert.equal(provider.imports.spx_environment_value_utf8_v1(1n,pointer),0);assert.deepEqual([...decode(read())],[0xc3,0xa9]);
assert.equal(provider.imports.spx_command_args_len_v1(),1n);
assert.equal(provider.imports.spx_command_arg_utf8_v1(0n,pointer),0);assert.equal(new TextDecoder().decode(decode(read())),'argument');
view().setBigInt64(pointer,-1n,true);assert.equal(provider.imports.spx_environment_value_utf8_v1(2n,pointer),1);assert.equal(read(),-1n);
let allocated;
provider.attach(memory,{allocateOwned(bytes){allocated=bytes;return BigInt.asIntN(64,0x8000000100000001n)},validateOwned(value){return BigInt.asUintN(64,value)===0x8000000100000001n}});
assert.equal(provider.imports.spx_command_stdin_read_v1(pointer),0);assert.deepEqual([...allocated],[7]);
view().setBigInt64(pointer,-1n,true);assert.equal(provider.imports.spx_command_stdin_read_v1(pointer),3);assert.equal(read(),-1n);
for(const absent of [null,[]]) {
 const value=createEnvironmentProvider({environment:absent});value.attach(memory);
 view().setBigInt64(pointer,-1n,true);
 assert.equal(value.imports.spx_environment_len_v1(pointer),absent===null?4:0);
 if(absent===null)assert.equal(read(),-1n);else assert.equal(view().getUint32(pointer,true),0);
}
for(const options of [
 {environment:[['','x']]},{environment:[['A=B','x']]},{environment:[['A\0','x']]},{environment:[['A','x\0']]},
 {environment:[['A','one'],['A','two']]},{environment:[['A','\ud800']]},
 {environment:[[new Uint8Array([0xff]),'']]},{environment:[['A',new Uint8Array([0xc0,0x80])]]},
 {environment:Array.from({length:257},(_,i)=>[String(i),''])},{arguments:Array(17).fill('')},
 {environment:[['A','x'.repeat(65535)]],stdin:new Uint8Array([1])},
 {environment:[['A','x']],arguments:['a'.repeat(65535)]},
 {stdin:new Uint8Array(65537)},
])assert.throws(()=>createEnvironmentProvider(options),TypeError);
const exact=createEnvironmentProvider({environment:[['A','x'.repeat(65533)]],arguments:['a'],stdin:new Uint8Array([1])});
assert.equal(exact.byteLength,65536);exact.attach(memory);
assert.equal(exact.imports.spx_environment_value_utf8_v1(0n,pointer),0);assert.equal(decode(read()).length,65533);
assert.throws(()=>createEnvironmentProvider({environment:[['A','x'.repeat(65534)]],arguments:['a'],stdin:new Uint8Array([1])}),TypeError);
console.log('environment provider constructor verified');
