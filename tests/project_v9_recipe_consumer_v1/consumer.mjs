import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';

// These identities and value oracles come from the source fixture, never from
// descriptor-derived field names or generated runtime internals.
const hex=value=>Buffer.from(value,'utf8').toString('hex');
const host=id=>'spx_field_id_'+hex(id);
const records=[
  {id:'left.payload',type:'left.Payload\u0008\u000c\u007f\u0085',fields:['','left.count','left.valid','left.size'],names:['bytes','count','valid','size'],kinds:['owned-bytes','i64','bool','usize']},
  {id:'right.payload',type:'right.Payload',fields:['right.size','right.valid','right.count','right.bytes'],names:['size','valid','count','bytes'],kinds:['usize','bool','i64','owned-bytes']},
];
const ids=records.map(row=>row.id);
const tsType={"owned-bytes":"Uint8Array",i64:"bigint",bool:"boolean",usize:"bigint"};
const expectedDeclarations=records.map(row=>`export interface SpxRecordId${hex(row.type)} {\n`+row.fields.map((id,index)=>`  readonly ${host(id)}: ${tsType[row.kinds[index]]};\n`).join('')+'}\n').join('')+
  'export interface SemapraxApi {\n'+records.map(row=>`  readonly "${row.id}": (arg0: Uint8Array, arg1: bigint, arg2: boolean) => SpxRecordId${hex(row.type)};\n`).join('')+'}\n'+
  'export interface SemapraxRuntime { readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id:I,...args:Parameters<SemapraxApi[I]>):ReturnType<SemapraxApi[I]>; readonly wasmSha256:string; }\nexport declare function instantiate(bytes:Uint8Array):Promise<SemapraxRuntime>;\nexport declare const exportIds:readonly(keyof SemapraxApi)[];\nexport default instantiate;\n';
const originalInstantiate=WebAssembly.instantiate;
let entries=0,instantiations=0;
WebAssembly.instantiate=async(bytes,imports)=>{
  instantiations++;
  const result=await originalInstantiate(bytes,imports);
  const exports={...result.instance.exports};
  for(const [name,fn] of Object.entries(exports))if(name.startsWith('spx_owned_v1_'))exports[name]=(...args)=>{entries++;return fn(...args)};
  return {...result,instance:{exports}};
};

function value(api,row,input,divisor,valid,throughFunctions=false){
  const result=throughFunctions?api.functions[row.id](input,divisor,valid):api.call(row.id,input,divisor,valid);
  assert(Object.isFrozen(result));assert.equal(Object.getPrototypeOf(result),null);
  assert.deepEqual(Object.keys(result).sort(),row.fields.map(host).sort());
  const byName=Object.fromEntries(row.names.map((name,index)=>[name,host(row.fields[index])]));
  assert.equal(result[byName.count],84n/divisor);
  assert.equal(result[byName.valid],valid);
  assert.equal(result[byName.size],BigInt(input.length));
  const bytes=result[byName.bytes];assert(bytes instanceof Uint8Array);
  assert.deepEqual(bytes,input);assert.notEqual(bytes.buffer,input.buffer);
  return bytes;
}

try{
  for(const label of ['original','renamed']){
    const directory=new URL(`./${label}/package/`,import.meta.url);
    const bindings=await import(new URL('semaprax.bindings.js',directory));
    const metadata=JSON.parse(readFileSync(new URL('semaprax.api.json',directory),'utf8'));
    const descriptor=JSON.parse(metadata.descriptor);
    const descriptorBytes=Buffer.from(metadata.descriptor);
    const size=Buffer.alloc(8);size.writeBigUInt64LE(BigInt(descriptorBytes.length));
    assert.equal(metadata.descriptor_digest,'sha256:'+createHash('sha256').update('semaprax.public-flat-owned-record-api.digest.v1\0').update(size).update(descriptorBytes).digest('hex'));
    assert.deepEqual(descriptor.exports.map(row=>row.stable_id),ids);
    const declarations=readFileSync(new URL('semaprax.bindings.d.ts',directory),'utf8');
    assert.equal(declarations,expectedDeclarations);
    for(const [index,row] of records.entries()){
      const actual=descriptor.exports[index].result;
      assert.equal(actual.record_id,row.type);
      assert.equal(actual.record_host_name,'SpxRecordId'+hex(row.type));
      assert.equal(actual.record_source_name,label==='renamed'&&index===1?'RenamedPayload':'Payload');
      assert.deepEqual(actual.fields.map(field=>field.stable_id),row.fields);
      assert.deepEqual(actual.fields.map(field=>field.source_name),row.names);
      assert.deepEqual(actual.fields.map(field=>field.host_name),row.fields.map(host));
      assert.deepEqual(actual.fields.map(field=>field.type),row.kinds);
      assert.deepEqual(actual.fields.map(field=>field.ordinal),[0,1,2,3]);
      assert(declarations.includes(`export interface SpxRecordId${hex(row.type)} {`));
    }
    const wasm=Uint8Array.from(readFileSync(new URL('app.wasm',directory)));
    assert.equal(metadata.wasm_sha256,'sha256:'+createHash('sha256').update(wasm).digest('hex'));
    assert.equal(bindings.wasmSha256,metadata.wasm_sha256.slice(7));
    const beforeTamper=instantiations,changed=wasm.slice();changed[changed.length-1]^=1;
    await assert.rejects(bindings.instantiate(changed),/artifact authentication failed/);
    assert.equal(instantiations,beforeTamper);
    const api=await bindings.instantiate(wasm),other=await bindings.instantiate(wasm);
    assert.deepEqual(bindings.exportIds,ids);assert(Object.isFrozen(bindings.exportIds));
    assert(Object.isFrozen(api));assert(Object.isFrozen(api.functions));
    assert.equal(Object.getPrototypeOf(api.functions),null);
    assert.deepEqual(Object.keys(api.functions),ids);
    for(const row of records){
      const input=Uint8Array.of(0,255,128,65,0);
      const maximum=Uint8Array.from({length:65536},(_,index)=>index%251);
      // Same primary corpus as the native SDK consumer, independently spelled
      // here so neither backend supplies the other's result oracle.
      for(const instance of [api,other])for(const payload of [new Uint8Array(),input,maximum]){
        for(const divisor of [-1n,1n,2n,0n])for(const valid of [false,true]){
          if(divisor===0n)assert.throws(()=>instance.call(row.id,payload,divisor,valid),error=>error.status===4);
          else value(instance,row,payload,divisor,valid);
        }
      }
      const saved=value(api,row,input,2n,true),independent=value(other,row,input,-2n,false,true);
      saved[0]=42;assert.equal(input[0],0);assert.equal(independent[0],0);
      value(api,row,input,7n,false,true);
      value(api,row,new Uint8Array(),1n,true);
      value(api,row,maximum,3n,false);
      const beforeInvalid=entries;
      assert.throws(()=>api.call(row.id,new Uint8Array(65537),1n,true),RangeError);
      assert.throws(()=>api.call(row.id,input,1,true),TypeError);
      assert.equal(entries,beforeInvalid);
      assert.throws(()=>api.call(row.id,input,0n,true),error=>error.status===4);
      for(let i=0;i<20;i++)value(api,row,input,2n,i%2===0);
      assert.equal(saved[0],42);assert.equal(independent[0],0);
    }
  }
}finally{WebAssembly.instantiate=originalInstantiate}
console.log('v9-recipe-consumer-ok');
