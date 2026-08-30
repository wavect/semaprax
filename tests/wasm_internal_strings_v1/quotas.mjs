import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
for(const {name,id,cause} of JSON.parse(readFileSync('quotas.json','utf8'))){
  const {instantiate}=await import(`./${name}.mjs`);
  const api=await instantiate(Uint8Array.from(readFileSync(`${name}.wasm`)));
  for(let repeat=0;repeat<3;repeat++){
    assert.deepEqual(api.call(id),{kind:'capacity',cause});
    assert.deepEqual(api.call('case.scalar',41n),{kind:'success',value:42n});
  }
}
process.stdout.write('quotas settled\n');
