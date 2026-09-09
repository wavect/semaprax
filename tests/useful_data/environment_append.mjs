import fs from 'node:fs';
import assert from 'node:assert/strict';
const [path,expected]=process.argv.slice(2);
const module=new WebAssembly.Module(fs.readFileSync(path));
const provider=environmentProvider(module);
const instance=new WebAssembly.Instance(module,provider.imports);provider.attach(instance);
const symbol='spx_data_'+Buffer.from('environment.run').toString('hex');
const memory=()=>instance.exports.memory??instance.exports.__spx_byte_memory;
function transcript(channel) {
  const length=Number(instance.exports[`__spx_${channel}_length_v1`].value);
  const base=Number(instance.exports[`__spx_${channel}_base_v1`].value);
  return [...new Uint8Array(memory().buffer,base,length)];
}
for(let repeat=0;repeat<3;repeat++) {
  assert.equal(instance.exports[symbol](),expected==='success'?1:0);
  assert.deepEqual(transcript('stdout'),expected==='success'?[65]:[]);
  assert.deepEqual(transcript('stderr'),expected==='success'?[65]:[]);
  assert.equal(Number(instance.exports.__spx_environment_status_v1.value),expected==='success'?0:1);
  provider.settled();
}
