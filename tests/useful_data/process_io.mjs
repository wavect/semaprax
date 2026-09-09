import fs from 'node:fs';
import assert from 'node:assert/strict';
const module=new WebAssembly.Module(fs.readFileSync(process.argv[2]));
const provider=environmentProvider(module);
let instance,mode=0,runs=0,settlements=0;
const memory=()=>instance.exports.memory??instance.exports.__spx_byte_memory;
const out=(pointer,value)=>new DataView(memory().buffer).setBigInt64(pointer,BigInt.asIntN(64,value),true);
Object.assign(provider.imports.env,{
 spx_process_run_v1:(tool,argv,argc,stdin,inputLength,timeout,maxout,maxerr,pointer)=>{
  runs++;assert.equal(tool,7n);assert.equal(argc,4n);assert.equal(inputLength,0n);
  assert.equal(timeout,100n);assert.equal(maxout,2n);assert.equal(maxerr,2n);
  for(let i=0n;i<4n;i++)assert.equal(provider.imports.env.spx_bytes_get(argv,i),0);
  if(mode===2||mode===4)return 4;
  if(mode===99)return 99;
  if(mode>=10&&mode<=16)return mode-9;
  if(mode===6)return 0;
  if(mode===9){out(pointer,(0x8000007fn<<32n)|34n);return 0;}
  const bytes=new Uint8Array(mode===7?37:mode===8?35:34);
  bytes[0]=mode===5?2:1;bytes[8]=28;bytes[16]=mode===8?3:1;bytes[24]=mode===8?0:1;bytes[32]=65;bytes[33]=33;
  const owner=provider.imports.env.spx_bytes_zeroed(BigInt(bytes.length));
  bytes.forEach((value,index)=>provider.imports.env.spx_bytes_set(owner,BigInt(index),value));
  out(pointer,owner);return 0;
 },
 spx_process_settle_v1:()=>{settlements++;return mode===3||mode===4?7:0;},
});
instance=new WebAssembly.Instance(module,provider.imports);provider.attach(instance);
const run=instance.exports['spx_data_'+Buffer.from('process.run').toString('hex')];
function transcript(channel){const base=Number(instance.exports[`__spx_${channel}_base_v1`].value),length=Number(instance.exports[`__spx_${channel}_length_v1`].value);return [...new Uint8Array(memory().buffer,base,length)];}
function invoke(selected,expected){
 mode=selected;runs=0;settlements=0;
 assert.equal(run(),expected===0?1:0,`mode ${mode}`);
 assert.equal(runs,1);assert.equal(settlements,1);
 assert.equal(Number(instance.exports.__spx_data_status_v1.value),expected);
 if(expected>0)assert.equal(Number(instance.exports.__spx_process_status_v1.value),expected);
 assert.deepEqual(transcript('stdout'),expected===0?[0,0,0,0]:[]);
 assert.deepEqual(transcript('stderr'),expected===0?[0,0,0,0]:[]);
 provider.settled();
}
for(let repeat=0;repeat<2;repeat++)for(const [selected,expected] of [[0,0],[2,4],[3,7],[4,4],[5,6],[6,-1],[7,5],[8,5],[9,-1],[99,-1],[10,1],[11,2],[12,3],[13,4],[14,5],[15,6],[16,7]]){invoke(selected,expected);invoke(0,0);}
console.log('process callbacks verified');
