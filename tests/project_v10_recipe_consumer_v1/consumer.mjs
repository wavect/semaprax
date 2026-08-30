import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

const ids=['bytes.raw','utf8.left','utf8.right'];
const texts={'utf8.left':'\ufeffhello\0世界é','utf8.right':''};
const expectedDeclarations='export interface SemapraxApi {\n'+
  '  readonly "bytes.raw": (arg0: Uint8Array) => Uint8Array;\n'+
  '  readonly "utf8.left": (arg0: bigint) => string;\n'+
  '  readonly "utf8.right": (arg0: bigint) => string;\n'+
  '}\nexport interface SemapraxRuntime { readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id: I, ...args: Parameters<SemapraxApi[I]>): ReturnType<SemapraxApi[I]>; readonly wasmSha256: string; }\nexport declare function instantiate(bytes: Uint8Array): Promise<SemapraxRuntime>;\nexport declare const exportIds: readonly (keyof SemapraxApi)[];\nexport default instantiate;\n';
const originalInstantiate=WebAssembly.instantiate;
let entries=0,instantiations=0;
WebAssembly.instantiate=async(bytes,imports)=>{
  instantiations++;
  const result=await originalInstantiate(bytes,imports),exports={...result.instance.exports};
  for(const [name,fn] of Object.entries(exports))if(name.startsWith('spx_owned_v1_'))exports[name]=(...args)=>{entries++;return fn(...args)};
  return {...result,instance:{exports}};
};
try{
  for(const label of ['original','renamed']){
    const directory=new URL(`./${label}/package/`,import.meta.url);
    const bindings=await import(new URL('semaprax.bindings.js',directory));
    assert.equal(readFileSync(new URL('semaprax.bindings.d.ts',directory),'utf8'),expectedDeclarations);
    const metadata=JSON.parse(readFileSync(new URL('semaprax.api.json',directory),'utf8'));
    assert.equal(metadata.schema,'semaprax.owned-utf8-api.v1');
    const descriptor=JSON.parse(metadata.descriptor),descriptorBytes=Buffer.from(metadata.descriptor);
    assert.equal(descriptor.schema,'semaprax.public-owned-utf8-api.v1');
    assert.equal(descriptor.project_schema,'semaprax.project.v10');
    const length=Buffer.alloc(8);length.writeBigUInt64LE(BigInt(descriptorBytes.length));
    assert.equal(metadata.descriptor_digest,'sha256:'+createHash('sha256').update('semaprax.public-owned-utf8-api.digest.v1\0').update(length).update(descriptorBytes).digest('hex'));
    assert.deepEqual(descriptor.exports.map(row=>row.stable_id),ids);
    assert.deepEqual(descriptor.exports.map(row=>row.result),['owned-bytes','owned-utf8','owned-utf8']);
    assert.deepEqual(descriptor.exports.map(row=>row.parameters.map(parameter=>parameter.type)),[['borrow-slice-u8'],['i64'],['i64']]);
    const wasm=Uint8Array.from(readFileSync(new URL('app.wasm',directory)));
    assert.equal(metadata.wasm.path,'app.wasm');
    assert.equal(metadata.wasm.sha256,createHash('sha256').update(wasm).digest('hex'));
    assert.equal(bindings.wasmSha256,metadata.wasm.sha256);
    const corrupted=wasm.slice();corrupted[corrupted.length-1]^=1;
    const beforeTamper=instantiations;
    await assert.rejects(bindings.instantiate(corrupted),/artifact authentication failed/);
    assert.equal(instantiations,beforeTamper);
    const api=await bindings.instantiate(wasm),other=await bindings.instantiate(wasm);
    assert.deepEqual(bindings.exportIds,ids);assert(Object.isFrozen(bindings.exportIds));
    assert(Object.isFrozen(api));assert(Object.isFrozen(api.functions));
    assert.equal(Object.getPrototypeOf(api.functions),null);
    assert.deepEqual(Object.keys(api.functions),ids);
    for(const instance of [api,other]){
      for(const id of ['utf8.left','utf8.right']){
        for(const divisor of [-1n,1n,2n])assert.equal(instance.call(id,divisor),texts[id]);
        const beforeInvalid=entries;
        assert.throws(()=>instance.call(id,1),TypeError);
        assert.throws(()=>instance.call(id),TypeError);
        assert.equal(entries,beforeInvalid);
        for(let index=0;index<20;index++){
          // The first owned String argument is allocated before division fails.
          // Observe recovery; this is not a complete allocation/destruction trace.
          assert.throws(()=>instance.call(id,0n),error=>error.status===4);
          assert.equal(instance.functions[id](2n),texts[id]);
        }
      }
      for(const input of [new Uint8Array(),Uint8Array.of(0,255,195,40),Uint8Array.from({length:65536},(_,index)=>index%251)]){
        const output=instance.call('bytes.raw',input);
        assert(output instanceof Uint8Array);assert.deepEqual(output,input);
        assert.notEqual(output.buffer,input.buffer);
        const retained=instance.functions['bytes.raw'](input);
        if(output.length){output[0]^=255;assert.deepEqual(retained,input)}
      }
      const beforeInvalid=entries;
      assert.throws(()=>instance.call('bytes.raw',new Uint8Array(65537)),RangeError);
      assert.equal(entries,beforeInvalid);
      assert.equal(instance.call('utf8.left',1n),texts['utf8.left']);
    }
  }
}finally{WebAssembly.instantiate=originalInstantiate}
console.log('v10-recipe-consumer-ok');
