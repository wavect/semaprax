// Concatenate into the caller's fixture. All authority is passed in explicitly.
function environmentProvider(module, maxOwnedBytes = 65536) {
  if (!Number.isSafeInteger(maxOwnedBytes) || maxOwnedBytes < 0) throw Error('invalid arena capacity');
  const acceptsOwnedLength=length=>Number.isSafeInteger(length)&&length>=0&&length<=maxOwnedBytes;
  const ownedLength=value=>{
    if(typeof value==='bigint'){
      if(value<0n||value>BigInt(maxOwnedBytes))throw Error('arena capacity');
      return Number(value);
    }
    if(!acceptsOwnedLength(value))throw Error('arena capacity');
    return value;
  };
  let instance,next=1;const owned=new Map();
  let nextVec=1n;const vectors=new Map();
  const vecKey=value=>{if(typeof value!=='bigint'||value<=0n)throw Error('Vec carrier');return value.toString()};
  const vecRead=(value,tag)=>{const entry=vectors.get(vecKey(value));if(!entry||entry.tag!==tag)throw Error('stale or mistyped Vec');return entry};
  const vecAlloc=(tag,capacity,values=[])=>{const handle=nextVec++;vectors.set(vecKey(handle),{tag,capacity,values});return handle};
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
  const allocate=data=>{if(!acceptsOwnedLength(data.length)||owned.size>=4096)throw Error('arena capacity');const id=next++;owned.set(id,new Uint8Array(data));return carrier(0x80000000|id,data.length)};
  const store=(value,index,data)=>{
    const [root]=split(value);
    if(!(root&0x80000000))throw Error('write owner');
    const target=bytes(value),at=BigInt.asUintN(64,index);
    if(at>BigInt(target.length)||BigInt(data.length)>BigInt(target.length)-at)throw Error('write bounds');
    if(data.some(byte=>!Number.isInteger(byte)||byte<0||byte>255))throw Error('write byte');
    target.set(data,Number(at));return value;
  };
  const storeSource=(value,index,one,source,selector,wideWidth)=>{
    const word=BigInt.asUintN(64,selector),copy=(word&(1n<<63n))!==0n;
    const width=copy?wideWidth(word):1;
    const offset=word&((1n<<BigInt(wideWidth===width5?63:62))-1n);
    const data=copy?bytes(source):null;
    return store(value,index,Array.from({length:width},(_,i)=>copy?(offset+BigInt(i)<BigInt(data.length)?data[Number(offset+BigInt(i))]:0):one));
  };
  const width5=()=>5;
  const width6or48=word=>(word&(1n<<62n))!==0n?48:6;
  const out=(p,value)=>view().setBigInt64(p,BigInt(value),true);
  const entries=[['A','alpha'],['Z','é']];let refs=[];
  const imports={};for(const item of WebAssembly.Module.imports(module)){if(item.kind!=='function')throw Error('unexpected import kind');imports[item.module]??={};imports[item.module][item.name]=()=>{throw Error(`unexpected import ${item.name}`)}}
  imports.env??={};
  Object.assign(imports.env,{
    spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
    spx_environment_len_v1:p=>{view().setInt32(p,2,true);return 0},
    spx_environment_name_utf8_v1:(i,p)=>{if(i<0n||i>=2n)return 1;out(p,refs[Number(i)][0]);return 0},
    spx_environment_value_utf8_v1:(i,p)=>{if(i<0n||i>=2n)return 1;out(p,refs[Number(i)][1]);return 0},
    spx_command_args_len_v1:()=>0n,spx_command_arg_utf8_v1:()=>1,spx_command_stdin_read_v1:()=>3,
    spx_command_owned_bytes_validate_v1:value=>{try{bytes(value);return 0}catch{return 1}},
    spx_bytes_copy:value=>allocate(bytes(value)),spx_bytes_zeroed:size=>allocate(new Uint8Array(ownedLength(size))),
    spx_bytes_get:(value,index)=>{const data=bytes(value);return index<0n||index>=BigInt(data.length)?-1:data[Number(index)]},
    spx_bytes_set:(value,index,byte)=>{const data=bytes(value);if(index<0n||index>=BigInt(data.length))throw Error('write bounds');data[Number(index)]=byte;return value},
    spx_bytes_set5:(value,index,...data)=>store(value,index,data),
    spx_bytes_set1_or5:(value,index,one,source,selector)=>storeSource(value,index,one,source,selector,width5),
    spx_bytes_set1_or6_or48:(value,index,one,source,selector)=>storeSource(value,index,one,source,selector,width6or48),
    spx_bytes_drop:value=>{bytes(value);const [root]=split(value);if(!(root&0x80000000)||!owned.delete(root&0x7fffffff))throw Error('double drop')},
    spx_bytes_as_slice:value=>{bytes(value);return value},
    spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?vecAlloc(tag,n):0n},
    spx_vec_push:(value,tag,bits)=>{const old=vecRead(value,tag);if(old.values.length>=old.capacity)return 0n;vectors.delete(vecKey(value));return vecAlloc(tag,old.capacity,old.values.concat([bits]))},
    spx_vec_len:(value,tag)=>BigInt(vecRead(value,tag).values.length),
    spx_vec_capacity:(value,tag)=>BigInt(vecRead(value,tag).capacity),
    spx_vec_get:(value,tag,index)=>{const old=vecRead(value,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=old.values.length)throw Error('Vec index');return old.values[n]},
    spx_vec_drop:value=>{if(!vectors.delete(vecKey(value)))throw Error('double Vec drop')},
  });
  return {imports,acceptsOwnedLength,attach(value){instance=value;let cursor=512;refs=entries.map(entry=>entry.map(text=>{const data=new TextEncoder().encode(text);new Uint8Array(memory().buffer).set(data,cursor);const result=carrier(cursor,data.length);cursor+=data.length+1;return result}))},settled(){if(owned.size||vectors.size)throw Error('unsettled owners')},live(){return owned.size+vectors.size}};
}
