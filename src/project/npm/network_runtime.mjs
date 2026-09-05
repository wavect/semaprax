const HASH=__SPX_HASH__,COMMAND=__SPX_COMMAND__,MAX=65536,TOTAL=1048576;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true}),records=new WeakMap(),fixtures=new WeakMap();
const own=(value,label)=>{if(Object.getPrototypeOf(value)!==Uint8Array.prototype)throw new TypeError(label+' must be Uint8Array');return new Uint8Array(value)};
const exactKeys=(value,keys,label)=>{if(!value||Object.getPrototypeOf(value)!==Object.prototype)throw new TypeError(label);const actual=Object.keys(value).sort();if(actual.length!==keys.length||actual.some((key,index)=>key!==keys[index]))throw new TypeError(label+' keys')};
export function createFixture(document){
  exactKeys(document,['connections','schema'],'fixture');
  if(document.schema!=='semaprax.network-fixture.v1'||!Array.isArray(document.connections)||document.connections.length>8)throw new TypeError('fixture schema');
  const connections=document.connections.map((item,index)=>{
    const allowed=['expect_send','host','port','ready','recv'];
    if(!item||Object.getPrototypeOf(item)!==Object.prototype||Object.keys(item).some(key=>!allowed.includes(key)))throw new TypeError('fixture connection '+index);
    if(typeof item.host!=='string'||!Number.isInteger(item.port)||item.port<1||item.port>65535||item.host.length===0||item.host.includes('\0'))throw new TypeError('fixture endpoint '+index);
    if(item.recv!==undefined&&(!Array.isArray(item.recv)||item.recv.some(chunk=>typeof chunk!=='string')))throw new TypeError('fixture recv '+index);
    if(item.expect_send!==undefined&&typeof item.expect_send!=='string')throw new TypeError('fixture expect_send '+index);
    if(item.ready!==undefined&&typeof item.ready!=='boolean')throw new TypeError('fixture ready '+index);
    return {host:item.host,port:item.port,pending:(item.recv||[]).map(chunk=>encoder.encode(chunk)),expect:item.expect_send===undefined?null:encoder.encode(item.expect_send),ready:item.ready!==false,sent:[],checked:false,open:false};
  });
  const fixture=Object.freeze({});fixtures.set(fixture,connections);return fixture;
}
export function createInvocation(argv,stdin,fixture){
  if(!Array.isArray(argv)||argv.length>16)throw new RangeError('argument count');
  const args=[];let used=0;
  for(const value of argv){if(typeof value!=='string'||value.includes('\0'))throw new TypeError('argument');const bytes=encoder.encode(value);used+=bytes.length;if(used>MAX)throw new RangeError('input capacity');args.push(bytes)}
  const input=own(stdin,'stdin');used+=input.length;if(used>MAX)throw new RangeError('input capacity');
  const source=fixtures.get(fixture);if(!source)throw new TypeError('fixture provider');
  const token=Object.freeze({});records.set(token,{args,input,connections:source.map(connection=>({...connection,pending:connection.pending.map(chunk=>new Uint8Array(chunk)),sent:[]}))});return token;
}
export async function instantiate(wasm,invocation){
  const input=records.get(invocation);if(!input||!records.delete(invocation))throw new TypeError('invocation provider');
  const bytes=own(wasm,'wasm');if(!globalThis.crypto?.subtle)throw new Error('Web Crypto required');
  const digest=new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256',bytes)),actual=Array.from(digest,b=>b.toString(16).padStart(2,'0')).join('');if(actual!==HASH)throw new Error('Wasm authentication');
  let instance=null,next=1,stdinRead=false,nextConnection=0,nextHandle=1,total=0;const entries=new Map(),handles=new Map();
  const memory=()=>new Uint8Array(instance.exports.memory.buffer),view=()=>new DataView(instance.exports.memory.buffer);
  const decode=c=>{if(typeof c!=='bigint')throw Error('carrier');const w=BigInt.asUintN(64,c),n=Number(w&0xffffffffn),r=Number((w>>32n)&0xffffffffn);if(n>MAX)throw Error('carrier length');return{n,r,t:(r&0x80000000)!==0,k:r&0x7fffffff}};
  const read=c=>{const d=decode(c);if(d.t){const b=entries.get(d.k);if(!b||b.length!==d.n)throw Error('owned token');return b}if((d.r&0xc0000000)===0x40000000){const p=(d.r&0xffff)*8,k=(d.r>>>16)&0x1fff,v=view();if(p+32>v.byteLength||v.getUint32(p,true)!==k||v.getUint32(p+4,true)!==p||Number(v.getBigUint64(p+24,true))!==d.n)throw Error('descriptor');const root=v.getBigInt64(p+8,true),off=Number(v.getBigUint64(p+16,true)),all=read(root);if(off>all.length||d.n>all.length-off)throw Error('range');return all.slice(off,off+d.n)}if(d.r>memory().length-d.n)throw Error('fixed root');return memory().slice(d.r,d.r+d.n)};
  const alloc=b=>{if(entries.size>=16||next>0x7fffffff)throw Error('arena');const k=next++;entries.set(k,new Uint8Array(b));return BigInt.asIntN(64,((0x80000000n|BigInt(k))<<32n)|BigInt(b.length))};
  const out=(p,value)=>view().setBigInt64(p,BigInt(value),true),concat=parts=>{const size=parts.reduce((sum,part)=>sum+part.length,0),all=new Uint8Array(size);let at=0;for(const part of parts){all.set(part,at);at+=part.length}return all},prefix=(a,b)=>a.length<=b.length&&a.every((v,i)=>v===b[i]);
  const firstRead=connection=>{if(connection.checked)return 0;connection.checked=true;if(connection.expect){const sent=concat(connection.sent);if(sent.length!==connection.expect.length||!prefix(sent,connection.expect))return 5}return 0};
  const deliver=(handle,max)=>{const connection=handles.get(handle);if(!connection)return[3];if(max>MAX)return[4];const early=firstRead(connection);if(early)return[early];while(connection.pending.length&&connection.pending[0].length===0)connection.pending.shift();if(!connection.pending.length)return[0,new Uint8Array(0)];const head=connection.pending[0],chunk=head.length<=max?connection.pending.shift():head.subarray(0,max);if(head.length>max)connection.pending[0]=head.subarray(max);total+=chunk.length;if(total>TOTAL)return[4];return[0,chunk]};
  let offsets=[],cursor=0;for(const arg of input.args){offsets.push(cursor);cursor+=arg.length}
  const env={
    spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('contract')},
    spx_bytes_copy:c=>alloc(read(c)),spx_bytes_get:(c,i)=>{const b=read(c),n=Number(BigInt.asUintN(64,i));return n<b.length?b[n]:-1},spx_bytes_drop:c=>{const d=decode(c);if(!d.t||!entries.delete(d.k))throw Error('drop')},spx_bytes_as_slice:c=>{read(c);return c},
    spx_command_args_len_v1:()=>BigInt(input.args.length),spx_command_arg_utf8_v1:(i,p)=>{const n=Number(BigInt.asUintN(64,i));if(n>=input.args.length)return 1;memory().set(input.args[n],offsets[n]);out(p,BigInt.asIntN(64,(BigInt(offsets[n])<<32n)|BigInt(input.args[n].length)));return 0},spx_command_stdin_read_v1:p=>{if(stdinRead)return 3;stdinRead=true;out(p,alloc(input.input));return 0},spx_command_owned_bytes_validate_v1:c=>{try{const d=decode(c),b=entries.get(d.k);return d.t&&d.k!==0&&b&&b.length===d.n?0:1}catch{return 1}},
    spx_network_connect_v1:(root,len,port,p)=>{let host;try{host=decoder.decode(read(BigInt.asIntN(64,(BigInt(root>>>0)<<32n)|BigInt(len>>>0))))}catch{return 2}const connection=input.connections[nextConnection];if(!connection||connection.host!==host||connection.port!==port)return 1;if(nextHandle>8)return 4;connection.open=true;nextConnection++;const handle=nextHandle++;handles.set(handle,connection);out(p,handle);return 0},
    spx_network_send_v1:(handle,root,len,p)=>{const connection=handles.get(handle);if(!connection)return 3;const chunk=read(BigInt.asIntN(64,(BigInt(root>>>0)<<32n)|BigInt(len>>>0)));total+=chunk.length;if(total>TOTAL)return 4;connection.sent.push(new Uint8Array(chunk));if(connection.expect&&!prefix(concat(connection.sent),connection.expect))return 5;out(p,chunk.length);return 0},
    spx_network_recv_v1:(handle,max,p)=>{const [status,chunk]=deliver(handle,max);if(status)return status;out(p,alloc(chunk));return 0},
    spx_network_stream_stdout_v1:(handle,dst,max,p)=>{const [status,chunk]=deliver(handle,max);if(status)return status;memory().set(chunk,dst);out(p,chunk.length);return 0},
    spx_network_wait_v1:(handle,timeout,p)=>{const connection=handles.get(handle);if(!connection)return 3;if(timeout>30000)return 4;if(!connection.ready){connection.ready=true;out(p,0);return 0}out(p,connection.pending.some(chunk=>chunk.length)?1:2);return 0},
    spx_network_close_v1:handle=>handles.delete(handle)?0:3,spx_network_settle_v1:()=>handles.clear()
  };
  try{const result=await WebAssembly.instantiate(bytes,Object.freeze({env:Object.freeze(env)}));instance=result.instance;if(instance.exports.memory.buffer.byteLength!==393216)throw Error('memory');const raw=instance.exports[COMMAND](),status=Number(instance.exports.__spx_data_status_v1.value),networkStatus=Number(instance.exports.__spx_network_status_v1.value);if(status===0&&networkStatus!==0)throw Error('network status marker');if(status!==0){const error=Object.assign(new Error(networkStatus?'network command failure':'language command failure'),{code:status});if(networkStatus)error.domain='semaprax.network.v1';throw error}if(raw!==0&&raw!==1)throw Error('bool');if(entries.size!==0||handles.size!==0)throw Error('unsettled');const sl=Number(instance.exports.__spx_stdout_length_v1.value),el=Number(instance.exports.__spx_stderr_length_v1.value);if(sl<0||el<0||sl+el>MAX)throw Error('transcript');const mem=memory(),stdout=mem.slice(131072,131072+sl),stderr=mem.slice(196608,196608+el);mem.fill(0,131072,393216);return Object.freeze({result:raw===1,stdout,stderr})}catch(error){if(instance)memory().fill(0,131072,393216);entries.clear();handles.clear();throw error}
}
