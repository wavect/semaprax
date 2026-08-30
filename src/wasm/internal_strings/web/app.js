import {instantiate} from './semaprax.js';
const EXPECTED_DESCRIPTOR_SHA256='__DESCRIPTOR_DIGEST__',EXPECTED_DESCRIPTOR_BYTES=__DESCRIPTOR_BYTES__;
const status=document.querySelector('#status'),selection=document.querySelector('#export-select'),signature=document.querySelector('#signature'),argumentsBox=document.querySelector('#arguments'),button=document.querySelector('#invoke'),transcript=document.querySelector('#transcript');

async function boundedFetch(path,maximum){
  const response=await fetch(path,{mode:'same-origin',redirect:'error'});
  if(!response.ok||response.body===null)throw new Error('Cannot read package artifact: '+path);
  const reader=response.body.getReader(),chunks=[];let size=0;
  try{
    for(;;){const {done,value}=await reader.read();if(done)break;
      if(value.byteLength>maximum-size)throw new Error('Package artifact exceeds byte limit: '+path);
      chunks.push(value);size+=value.byteLength;
    }
  }catch(error){try{await reader.cancel()}catch{}throw error}
  finally{reader.releaseLock()}
  const bytes=new Uint8Array(size);let offset=0;for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength}return bytes;
}
function closed(value,keys){
  if(value===null||typeof value!=='object'||Array.isArray(value))throw new Error('Invalid package descriptor object');
  const actual=Object.keys(value);if(actual.length!==keys.length||keys.some(key=>!Object.hasOwn(value,key)))throw new Error('Invalid package descriptor fields');
}
function integer(value,min,max){if(!Number.isInteger(value)||value<min||value>max)throw new Error('Invalid package descriptor bound')}
function descriptor(bytes){
  const value=JSON.parse(new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes));
  closed(value,['schema','runtime_schema','wasm_sha256','wasm_bytes','memory_pages','result_offset','literal_offset','stack_bytes','derived_owner_capacity','limits','exports']);
  if(value.schema!=='semaprax.wasm-internal-strings.v1'||value.runtime_schema!=='semaprax.wasm-internal-strings.runtime.v1'||!/^[0-9a-f]{64}$/.test(value.wasm_sha256)||value.memory_pages!==4||value.result_offset!==65536||value.literal_offset!==196608)throw new Error('Invalid package descriptor identity');
  integer(value.wasm_bytes,1,16777216);integer(value.stack_bytes,0,65536);integer(value.derived_owner_capacity,1,65536);
  closed(value.limits,['max_string_bytes','max_live_bytes','max_cumulative_bytes','max_live_owners']);
  integer(value.limits.max_string_bytes,0,65536);integer(value.limits.max_live_bytes,0,16777216);integer(value.limits.max_cumulative_bytes,0,67108864);integer(value.limits.max_live_owners,1,value.derived_owner_capacity);
  if(!Array.isArray(value.exports)||value.exports.length<1||value.exports.length>32)throw new Error('Invalid package export count');
  const ids=new Set();
  for(const [index,fact] of value.exports.entries()){
    closed(fact,['stable_id','wasm_export','parameters','result']);
    if(typeof fact.stable_id!=='string'||ids.has(fact.stable_id)||fact.wasm_export!=='__spx_call_'+index||!Array.isArray(fact.parameters)||fact.parameters.length>8||fact.parameters.some(type=>type!=='i64'&&type!=='bool')||(fact.result!=='i64'&&fact.result!=='bool'))throw new Error('Invalid package export signature');
    ids.add(fact.stable_id);
  }
  return value;
}
function controls(fact){
  signature.textContent=fact.stable_id+'('+fact.parameters.join(', ')+') → '+fact.result;
  argumentsBox.replaceChildren();
  fact.parameters.forEach((type,index)=>{
    const label=document.createElement('label'),input=document.createElement('input');
    input.id='arg-'+index;label.htmlFor=input.id;label.textContent='Argument '+(index+1)+' · '+type;
    input.type=type==='bool'?'checkbox':'text';if(type==='i64'){input.value='0';input.required=true;input.autocomplete='off';input.spellcheck=false}
    argumentsBox.append(label,input);
  });
}
async function start(){
  const bytes=await boundedFetch('./semaprax.internal-strings.json',1048576);
  if(bytes.byteLength!==EXPECTED_DESCRIPTOR_BYTES)throw new Error('Package descriptor length disagrees');
  const hash=new Uint8Array(await crypto.subtle.digest('SHA-256',bytes));
  if(Array.from(hash,byte=>byte.toString(16).padStart(2,'0')).join('')!==EXPECTED_DESCRIPTOR_SHA256)throw new Error('Package descriptor digest disagrees');
  const facts=descriptor(bytes),runtime=await instantiate(await boundedFetch('./app.wasm',16777216));
  for(const fact of facts.exports){const option=document.createElement('option');option.value=fact.stable_id;option.textContent=fact.stable_id;selection.append(option)}
  const selected=()=>facts.exports[selection.selectedIndex];
  selection.addEventListener('change',()=>controls(selected()));controls(selected());
  selection.disabled=false;button.disabled=false;status.textContent='Ready · '+facts.exports.length+' exports';
  document.querySelector('#call-form').addEventListener('submit',event=>{
    event.preventDefault();button.disabled=true;
    try{
      const fact=selected(),args=fact.parameters.map((type,index)=>{
        const input=document.querySelector('#arg-'+index);
        if(type==='bool')return input.checked;
        if(!/^-?(0|[1-9][0-9]*)$/.test(input.value))throw new Error('Argument '+(index+1)+' must be a decimal i64');
        const value=BigInt(input.value);if(value<-(1n<<63n)||value>(1n<<63n)-1n)throw new Error('Argument '+(index+1)+' is outside i64');return value;
      });
      transcript.textContent=JSON.stringify(runtime.call(fact.stable_id,...args),(_,value)=>typeof value==='bigint'?value.toString():value,2);
    }catch(error){transcript.textContent='Error: '+String(error)}finally{button.disabled=false}
  });
}
start().catch(error=>{status.textContent='Package unavailable: '+String(error);button.disabled=true;selection.disabled=true});
