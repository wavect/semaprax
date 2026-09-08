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
const env={
 spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{throw Error('contract')},
 spx_bytes_copy:value=>allocate(bytes(value)),spx_bytes_get:(value,index)=>{const data=bytes(value);return Number(index)<data.length?data[Number(index)]:-1},spx_bytes_drop:value=>{const [root]=split(value);if(!owned.delete(root&0x7fffffff))throw Error('drop')},spx_bytes_as_slice:value=>value,
 spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:()=>3,spx_command_owned_bytes_validate_v1:value=>{try{bytes(value);return 0}catch{return 1}},
 spx_filesystem_write_new_v1:(pathRoot,pathCarrier,pathLength,dataRoot,dataCarrier,dataLength,pointer)=>{const name=key(pathRoot,pathCarrier,pathLength);if(files.has(name))return 3;files.set(name,prefix(dataRoot,dataCarrier,dataLength));out(pointer,BigInt(dataLength));return 0},
 spx_filesystem_read_v1:(pathRoot,pathCarrier,pathLength,max,pointer)=>{const data=files.get(key(pathRoot,pathCarrier,pathLength));if(!data)return 2;if(data.length>max)return 4;out(pointer,allocate(data));return 0},
};
console.log(`validate ${WebAssembly.validate(wasm)?1:0}`);
const result=await WebAssembly.instantiate(wasm,{env});instance=result.instance;
for(let i=0;i<2;i++){const value=instance.exports[symbol]();console.log(`run ${value} ${instance.exports.__spx_data_status_v1.value} ${instance.exports.__spx_filesystem_status_v1.value} ${owned.size}`);files.clear()}
