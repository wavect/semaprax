import fs from 'node:fs';
const [path,symbol]=process.argv.slice(2),wasm=fs.readFileSync(path);let instance,next=1;const owned=new Map(),files=new Map();
const memoryExport=()=>instance.exports.memory??instance.exports.__spx_byte_memory;
const memory=()=>new Uint8Array(memoryExport().buffer),view=()=>new DataView(memoryExport().buffer);
const carrier=(root,length)=>BigInt.asIntN(64,(BigInt(root>>>0)<<32n)|BigInt(length>>>0));
const split=value=>{const word=BigInt.asUintN(64,value);return [Number((word>>32n)&0xffffffffn),Number(word&0xffffffffn)]};
const bytes=value=>{const [root,length]=split(value);if(root&0x80000000){const value=owned.get(root&0x7fffffff);if(!value||value.length!==length)throw Error('owned');return value}return memory().slice(root,root+length)};
const allocate=value=>{const id=next++;owned.set(id,new Uint8Array(value));return carrier(0x80000000|id,value.length)};
const out=(pointer,value)=>view().setBigInt64(pointer,BigInt(value),true);
const prefix=(root,carrierLength,logicalLength)=>bytes(carrier(root,carrierLength)).slice(0,logicalLength);
const key=(root,carrierLength,logicalLength)=>Buffer.from(prefix(root,carrierLength,logicalLength)).toString('hex');
const raw=process.argv[4]==='1',directories=new Set(); let calls=0;
const seed=()=>{files.clear();directories.clear();calls=0;if(raw){files.set('61',new Uint8Array());files.set('ff',new Uint8Array([0,255]))}};
const env={
 spx_bytes_zeroed:count=>allocate(new Uint8Array(Number(count))),spx_bytes_set:(value,index,byte)=>{const data=bytes(value);if(Number(index)>=data.length)throw Error('set-bounds');data[Number(index)]=byte;return value},
 spx_filesystem_stat_v2:(r,c,l,p)=>{calls++;const k=key(r,c,l);if(k===''){out(p,2n);return 0}const data=files.get(k);if(!data)return 2;out(p,BigInt(data.length)*4n+1n);return 0},
 spx_filesystem_list_v2:(r,c,l,max,p)=>{calls++;const k=key(r,c,l);let data;if(raw&&k==='')data=new Uint8Array([97,0,255,0]);else if(k==='64'&&directories.has(k)&&files.has('642f61')){if(files.get('642f61')[0]!==255)throw Error('replace');data=new Uint8Array([97,0])}else return 2;if(data.length>max)return 4;out(p,allocate(data));return 0},
 spx_filesystem_create_dir_v2:(r,c,l,p)=>{calls++;const k=key(r,c,l);if(k!=='64')throw Error('directory');if(directories.has(k))return 3;directories.add(k);out(p,0n);return 0},
 spx_filesystem_remove_v2:(r,c,l,p)=>{calls++;const k=key(r,c,l);if(files.has(k))files.delete(k);else if(directories.has(k)&&files.size===0)directories.delete(k);else return 2;out(p,0n);return 0},
 spx_filesystem_write_atomic_v2:(r,c,l,dr,dc,dl,p)=>{calls++;const k=key(r,c,l),data=prefix(dr,dc,dl);if(k!=='642f61'||!directories.has('64')||dl!==1||data[0]!== (files.has(k)?255:65))throw Error('atomic');files.set(k,new Uint8Array(data));out(p,BigInt(dl));return 0},
 spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('contract')},
 spx_bytes_copy:value=>allocate(bytes(value)),spx_bytes_get:(value,index)=>{const data=bytes(value);return Number(index)<data.length?data[Number(index)]:-1},spx_bytes_drop:value=>{const [root]=split(value);if(!owned.delete(root&0x7fffffff))throw Error('drop')},spx_bytes_as_slice:value=>value,
 spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:()=>3,spx_command_owned_bytes_validate_v1:value=>{try{bytes(value);return 0}catch{return 1}},
 spx_filesystem_write_new_v1:(pathRoot,pathCarrier,pathLength,dataRoot,dataCarrier,dataLength,pointer)=>{const name=key(pathRoot,pathCarrier,pathLength);if(files.has(name))return 3;files.set(name,prefix(dataRoot,dataCarrier,dataLength));out(pointer,BigInt(dataLength));return 0},
 spx_filesystem_read_v1:(pathRoot,pathCarrier,pathLength,max,pointer)=>{const data=files.get(key(pathRoot,pathCarrier,pathLength));if(!data)return 2;if(data.length>max)return 4;out(pointer,allocate(data));return 0},
};
console.log(`validate ${WebAssembly.validate(wasm)?1:0}`);
const result=await WebAssembly.instantiate(wasm,{env});instance=result.instance;
for(let i=0;i<2;i++){seed();const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_data_status_v1.value} ${instance.exports.__spx_filesystem_status_v2.value} ${owned.size}`);if(calls!==(raw?2:7)||(!raw&&(files.size!==0||directories.size!==0)))throw Error('settlement')}
