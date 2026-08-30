import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';

// Independent literal-byte oracle; neither metadata nor Wasm supplies it.
const unit=Uint8Array.of(239,187,191,0,228,184,150,195,169,240,159,153,130);
const encoder=new TextEncoder();
for(const [label,length] of [['minus-one',65535],['exact',65536]]){
  const directory=new URL(`./${label}/package/`,import.meta.url);
  const bindings=await import(new URL('semaprax.bindings.js',directory));
  assert.deepEqual(bindings.exportIds,['utf8.maximum']);
  const wasm=Uint8Array.from(readFileSync(new URL('app.wasm',directory)));
  const first=await bindings.instantiate(wasm),second=await bindings.instantiate(wasm);
  const expected=new Uint8Array(length);
  for(let i=0;i<5041;i++)expected.set(unit,i*13);
  expected.fill(97,65533);
  const text='\ufeff\0世é🙂'.repeat(5041)+'a'.repeat(length-65533);
  const retained=first.call('utf8.maximum');
  assert.equal(typeof retained,'string');
  assert.equal(retained,text);
  assert.deepEqual(encoder.encode(retained),expected);
  for(const instance of [first,second]){
    for(let i=0;i<20;i++){
      const value=instance.functions['utf8.maximum']();
      assert.equal(typeof value,'string');
      assert.equal(value,text);
      const bytes=encoder.encode(value);
      assert.equal(bytes.length,length);
      assert.deepEqual(bytes,expected);
      assert.equal(value.charCodeAt(0),0xfeff);
      assert.equal(value.charCodeAt(1),0);
    }
  }
  assert.equal(retained,text);
  assert.deepEqual(encoder.encode(retained),expected);
}
console.log('project-owned-utf8-capacity-ok');
