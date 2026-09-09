// Concatenate into the caller's fixture. All authority is passed in explicitly.
function environmentProvider(module) {
  let instance,next=1;const owned=new Map();
  const memory=()=>instance.exports.memory??instance.exports.__spx_byte_memory;
  const view=()=>new DataView(memory().buffer);
  const carrier=(root,length)=>BigInt.asIntN(64,(BigInt(root>>>0)<<32n)|BigInt(length>>>0));
  const split=value=>{const word=BigInt.asUintN(64,value);return [Number(word>>32n),Number(word&0xffffffffn)]};
  const bytes=value=>{
    const [root,length]=split(value);
    if((root&0xc0000000)===0x40000000){
      const pointer=(root&0xffff)*8,identity=(root>>>16)&0x1fff,v=view();
      if(!identity||pointer+32>v.byteLength||v.getUint32(pointer,true)!==identity||v.getUint32(pointer+4,true)!==pointer||v.getBigUint64(pointer+24,true)!==BigInt(length))throw Error('range descriptor');
      const ultimate=v.getBigInt64(pointer+8,true),offset=v.getBigUint64(pointer+16,true);
      if((split(ultimate)[0]&0xc0000000)===0x40000000)throw Error('nested range descriptor');
      const data=bytes(ultimate);
      if(offset>BigInt(data.length)||BigInt(length)>BigInt(data.length)-offset)throw Error('range extent');
      return data.subarray(Number(offset),Number(offset)+length);
    }
    if(root&0x80000000){const data=owned.get(root&0x7fffffff);if(!data||data.length!==length)throw Error('owned carrier');return data}
    if(root+length>memory().buffer.byteLength)throw Error('input range');
    return new Uint8Array(memory().buffer,root,length);
  };
  const allocate=data=>{if(data.length>65536||owned.size>=4096)throw Error('arena capacity');const id=next++;owned.set(id,new Uint8Array(data));return carrier(0x80000000|id,data.length)};
  const out=(p,value)=>view().setBigInt64(p,BigInt(value),true);
  const entries=[['A','alpha'],['Z','é']];let refs=[];
  const imports={};for(const item of WebAssembly.Module.imports(module)){if(item.kind!=='function')throw Error('unexpected import kind');imports[item.module]??={};imports[item.module][item.name]=()=>{throw Error(`unexpected import ${item.name}`)}}
  Object.assign(imports.env,{
    spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
    spx_environment_len_v1:p=>{view().setInt32(p,2,true);return 0},
    spx_environment_name_utf8_v1:(i,p)=>{if(i<0n||i>=2n)return 1;out(p,refs[Number(i)][0]);return 0},
    spx_environment_value_utf8_v1:(i,p)=>{if(i<0n||i>=2n)return 1;out(p,refs[Number(i)][1]);return 0},
    spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:()=>3,
    spx_command_owned_bytes_validate_v1:value=>{try{bytes(value);return 0}catch{return 1}},
    spx_bytes_copy:value=>allocate(bytes(value)),spx_bytes_zeroed:size=>allocate(new Uint8Array(Number(size))),
    spx_bytes_get:(value,index)=>{const data=bytes(value);return index<0n||index>=BigInt(data.length)?-1:data[Number(index)]},
    spx_bytes_set:(value,index,byte)=>{const data=bytes(value);if(index<0n||index>=BigInt(data.length))throw Error('write bounds');data[Number(index)]=byte;return value},
    spx_bytes_drop:value=>{bytes(value);const [root]=split(value);if(!(root&0x80000000)||!owned.delete(root&0x7fffffff))throw Error('double drop')},
    spx_bytes_as_slice:value=>{bytes(value);return value},
  });
  return {imports,attach(value){instance=value;let cursor=512;refs=entries.map(entry=>entry.map(text=>{const data=new TextEncoder().encode(text);new Uint8Array(memory().buffer).set(data,cursor);const result=carrier(cursor,data.length);cursor+=data.length+1;return result}))},settled(){if(owned.size)throw Error('unsettled byte owners')},live(){return owned.size}};
}
