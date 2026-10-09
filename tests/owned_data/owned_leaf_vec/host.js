'use strict';
// Strict private Core-Wasm host for the additive owned-leaf Vec profile.
// Every handle is a one-generation authority; an operation that returns zero
// has consumed nothing. The compiler's status/cleanup path owns that failure.
const fs = require('fs');
const moduleBytes = fs.readFileSync(process.argv[2]);
const expectedStatus = Number(process.argv[3]);
const expectedValue = BigInt(process.argv[4]);
const refusal = process.argv[5] || '';
let instance, nextPayload = 1, nextVec = 1n, nextIter = 1n << 62n, copies = 0, drops = 0;
const payloads = new Map(), vectors = new Map(), iterators = new Map();
const utf8 = new TextDecoder('utf-8',{fatal:true});
const memory = () => instance.exports.__spx_byte_memory || instance.exports.memory;
const view = (address, length) => {
  if (!Number.isInteger(address) || address < 0 || address + length > memory().buffer.byteLength)
    throw Error('Wasm output range');
  return new DataView(memory().buffer, address, length);
};
const split = carrier => {
  const word = BigInt.asUintN(64, carrier);
  return {length: Number(word & 0xffffffffn), origin: Number(word >> 32n)};
};
const read = carrier => {
  const {length, origin} = split(carrier);
  if (origin & 0x80000000) {
    const bytes = payloads.get(origin & 0x7fffffff);
    if (!bytes || bytes.length !== length) throw Error('stale payload');
    return bytes;
  }
  if (origin > memory().buffer.byteLength - length) throw Error('borrowed payload range');
  return new Uint8Array(memory().buffer, origin, length);
};
const alloc = carrier => {
  const bytes = new Uint8Array(read(carrier));
  const id = nextPayload++;
  payloads.set(id, bytes);
  copies++;
  return BigInt.asIntN(64, ((0x80000000n | BigInt(id)) << 32n) | BigInt(bytes.length));
};
const ownBytes = bytes => {
  const id = nextPayload++;
  payloads.set(id, new Uint8Array(bytes));
  copies++;
  return BigInt.asIntN(64, ((0x80000000n | BigInt(id)) << 32n) | BigInt(bytes.length));
};
const drop = carrier => {
  if (carrier === 0n) return;
  read(carrier);
  const {origin} = split(carrier);
  if (!(origin & 0x80000000) || !payloads.delete(origin & 0x7fffffff))
    throw Error('double or borrowed payload drop');
  drops++;
};
const fields = shape => {
  const raw = BigInt.asUintN(64, shape), result = [];
  for (let i = 0; i < 8; i++) {
    const byte = Number((raw >> BigInt(i * 8)) & 255n);
    if (byte === 0) {
      if ((raw >> BigInt(i * 8)) !== 0n) throw Error('noncanonical shape');
      break;
    }
    const code = byte >> 4, slot = byte & 15;
    if (code < 1 || code > 10 || slot >= 8 || result.some(field => field.slot === slot))
      throw Error('invalid shape field');
    result.push({code, slot});
  }
  if (!result.length || !result.some(field => field.code >= 9) ||
      result.filter(field => field.code >= 9).length > 2 ||
      result.some(field => field.slot >= result.length)) throw Error('invalid owned shape');
  return result;
};
const limit = shape => {
  const list = fields(shape), owned = list.filter(field => field.code >= 9).length;
  const scalar = list.length - owned;
  return [8192n, scalar ? 8192n / BigInt(scalar) : 8192n,
    131072n / (16n * BigInt(owned))].reduce((a, b) => a < b ? a : b);
};
const checkWord = (word, code) => {
  if (code >= 9) {
    if (word !== 0n && !(split(word).origin & 0x80000000)) throw Error('borrowed owned leaf');
    if (word !== 0n) read(word);
    if (code === 10) utf8.decode(read(word));
  } else if (code === 2 && BigInt.asIntN(32, word) !== word) throw Error('noncanonical i32');
  else if ((code === 3 || code === 8) && (word < 0n || word > (code === 8 ? 1n : 255n)))
    throw Error('noncanonical u8/bool');
  else if (code === 5 && (word < 0n || word > 0x10ffffn || (word >= 0xd800n && word <= 0xdfffn)))
    throw Error('invalid char');
  else if (code === 6 && (word < 0n || word > 0xffffffffn)) throw Error('noncanonical f32');
};
const rowFromDeclared = (shape, values) => {
  const spec = fields(shape), row = Array(spec.length).fill(0n), seen = new Set();
  for (let i = 0; i < spec.length; i++) {
    const word = values[i], field = spec[i];
    checkWord(word, field.code);
    if (field.code >= 9 && word !== 0n) {
      if (seen.has(word)) throw Error('aliased owned leaves');
      seen.add(word);
    }
    row[field.slot] = word;
  }
  if (values.slice(spec.length).some(word => word !== 0n)) throw Error('nonzero spare word');
  return row;
};
const dropRow = (row, shape) => {
  for (const field of fields(shape)) if (field.code >= 9) drop(row[field.slot]);
};
const vec = (handle, tag) => {
  const value = vectors.get(handle);
  if (!value || value.tag !== tag) throw Error('stale or mistyped Vec');
  return value;
};
const bind = (handle, identity, shape) => {
  const value = vectors.get(handle);
  if (!value || (value.tag !== 10 && value.tag !== 11)) throw Error('stale owned Vec');
  const spec = fields(shape);
  if (value.tag === 10 && !(spec.length === 3 &&
      spec.filter(field => field.code === 9).length === 2 &&
      spec.filter(field => field.code <= 8).length === 1 &&
      spec.filter(field => field.code === 9).every(field => field.slot <= 1) &&
      spec.find(field => field.code <= 8).slot === 2)) throw Error('legacy shape mismatch');
  if (value.shape !== undefined && (value.shape !== shape || value.identity !== identity))
    throw Error('declaration identity or shape mismatch');
  return value;
};
const mint = value => {
  const handle = nextVec++;
  vectors.set(handle, value);
  return handle;
};
const move = (handle, value) => {
  vectors.delete(handle);
  return mint(value);
};
const iterator = (handle, cursor, identity, shape) => {
  const value = iterators.get(handle);
  if (!value || value.cursor !== cursor || value.identity !== identity || value.shape !== shape)
    throw Error('stale or mistyped iterator epoch');
  return value;
};
const totalKey = (word, code) => {
  const bits = BigInt.asUintN(code === 6 || code === 2 ? 32 : 64, word);
  if (code === 1 || code === 2) return bits ^ (code === 2 ? 0x80000000n : 0x8000000000000000n);
  if (code === 6 || code === 7) {
    const mask = code === 6 ? 0xffffffffn : 0xffffffffffffffffn;
    const sign = code === 6 ? 0x80000000n : 0x8000000000000000n;
    return bits & sign ? bits ^ mask : bits ^ sign;
  }
  return bits;
};
const compareBytes = (left, right) => {
  const a = read(left), b = read(right), bound = Math.min(a.length, b.length);
  for (let i = 0; i < bound; i++) if (a[i] !== b[i]) return a[i] - b[i];
  return a.length - b.length;
};
const compareRows = (left, right, shape) => {
  for (const field of fields(shape)) {
    const a = left[field.slot], b = right[field.slot];
    const cmp = field.code >= 9 ? compareBytes(a, b) :
      (totalKey(a, field.code) < totalKey(b, field.code) ? -1 :
       totalKey(a, field.code) > totalKey(b, field.code) ? 1 : 0);
    if (cmp) return cmp;
  }
  return 0;
};
const env = {
  spx_add:(a,b)=>a+b, spx_sub:(a,b)=>a-b, spx_mul:(a,b)=>a*b,
  spx_div:(a,b)=>a/b, spx_rem:(a,b)=>a%b, spx_neg:a=>-a,
  spx_contract_fail:status=>{throw Error(`status:${status}`)},
  spx_bytes_copy:alloc, spx_bytes_drop:drop, spx_bytes_as_slice:carrier=>{read(carrier);return carrier},
  spx_bytes_get:(carrier,index)=>read(carrier)[Number(index)]??-1,
  spx_bytes_zeroed:length=>{
    if(length<0n||length>131072n)return 0n;
    return ownBytes(new Uint8Array(Number(length)));
  },
  spx_bytes_set:(carrier,index,byte)=>{
    const bytes=read(carrier);if(index<0n||index>=BigInt(bytes.length)||byte<0||byte>255)
      throw Error('byte set bounds');
    bytes[Number(index)]=byte;return carrier;
  },
  spx_bytes_set5:()=>{throw Error('unexpected five-byte write')},
  spx_bytes_set1_or5:()=>{throw Error('unexpected one-or-five write')},
  spx_bytes_set1_or6_or48:()=>{throw Error('unexpected one-or-six-or-forty-eight write')},
  spx_string_concat_v1:(left,right)=>ownBytes(new Uint8Array([...read(left),...read(right)])),
  spx_string_from_char_v1:value=>ownBytes(new TextEncoder().encode(String.fromCodePoint(value))),
  spx_string_len_chars_v1:carrier=>BigInt([...new TextDecoder('utf-8',{fatal:true}).decode(read(carrier))].length),
  spx_string_from_i64_v1:value=>ownBytes(new TextEncoder().encode(value.toString())),
  spx_string_from_usize_v1:value=>ownBytes(new TextEncoder().encode(BigInt.asUintN(64,value).toString())),
  spx_string_starts_with_v1:(left,right)=>{
    const a=read(left),b=read(right);
    return b.length<=a.length&&b.every((byte,i)=>a[i]===byte)?1:0;
  },
  spx_string_contains_v1:(left,right)=>{
    const a=read(left),b=read(right);
    for(let i=0;i<=a.length-b.length;i++)if(b.every((byte,j)=>a[i+j]===byte))return 1;
    return 0;
  },
  spx_string_compare_v2:(left,right)=>BigInt(Math.sign(compareBytes(left,right))),
  spx_vec_with_capacity_v2:(tag,capacity)=>{
    if(tag<1||tag>10||capacity<0n||capacity>(tag===10?4096n:8192n))return 0n;
    return mint({tag,capacity,values:[]});
  },
  spx_vec_push_v2:(handle,tag,word)=>{
    if(tag===10)throw Error('legacy record requires exact push');
    const value=vec(handle,tag);
    if(tag===9)read(word);
    if(BigInt(value.values.length)>=value.capacity)return 0n;
    value.values.push(word);return move(handle,value);
  },
  spx_vec_record_push_v2:(handle,tag,b0,b1,scalar)=>{
    const value=vec(handle,tag);
    if(tag!==10)throw Error('legacy push tag');
    read(b0);read(b1);
    if(b0!==0n&&b0===b1)throw Error('aliased legacy owned leaves');
    if(BigInt(value.values.length)>=value.capacity)return 0n;
    value.values.push([b0,b1,scalar]);return move(handle,value);
  },
  spx_vec_len_v2:(handle,tag)=>BigInt(vec(handle,tag).values.length),
  spx_vec_capacity_v2:(handle,tag)=>vec(handle,tag).capacity,
  spx_vec_get_v2:(handle,tag,index)=>{
    const value=vec(handle,tag);
    if(tag>=9||index<0n||index>=BigInt(value.values.length))throw Error('invalid Vec get');
    return value.values[Number(index)];
  },
  spx_vec_drop_v2:handle=>{
    const value=vectors.get(handle);if(!value)throw Error('stale Vec drop');
    if(value.tag===9)for(const word of value.values)drop(word);
    else if(value.shape===undefined&&value.tag===10){for(const row of value.values){drop(row[0]);drop(row[1])}}
    else if(value.shape!==undefined)for(const row of value.values)dropRow(row,value.shape);
    vectors.delete(handle);
  },
  spx_vec_reserve_exact_v2:(handle,tag,additional)=>{
    const value=vec(handle,tag),target=BigInt(value.values.length)+additional;
    if(tag===10||additional<0n||target>8192n)return 0n;
    value.values=value.values.slice();
    if(target>value.capacity)value.capacity=target;
    return move(handle,value);
  },
  spx_vec_set_v2:(handle,tag,index,word)=>{
    const value=vec(handle,tag);
    if(tag===10||index<0n||index>=BigInt(value.values.length))return 0n;
    if(tag===9){read(word);drop(value.values[Number(index)])}
    value.values[Number(index)]=word;
    return move(handle,value);
  },
  spx_vec_clear_v2:(handle,tag)=>{
    const value=vec(handle,tag);
    if(tag===9)for(const word of value.values)drop(word);
    else if(value.shape===undefined&&tag===10){for(const row of value.values){drop(row[0]);drop(row[1])}}
    else if(value.shape!==undefined)for(const row of value.values)dropRow(row,value.shape);
    value.values=[];return move(handle,value);
  },
  spx_vec_sort_v3:(handle,tag)=>{
    const value=vec(handle,tag);
    if(tag>=9)throw Error('old sort cannot reorder owned payloads');
    value.values=value.values.slice().sort((a,b)=>{
      const left=totalKey(a,tag),right=totalKey(b,tag);
      return left<right?-1:left>right?1:0;
    });
    return move(handle,value);
  },
  spx_vec_leaf_new_v1:(identity,shape,capacity)=>{
    if(capacity<0n||capacity>limit(shape)||refusal==='allocation')return 0n;
    try{return mint({tag:11,identity,shape,capacity,values:[]})}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
  },
  spx_vec_leaf_push_v1:(handle,identity,shape,...words)=>{
    const value=bind(handle,identity,shape);
    let row;
    try{row=rowFromDeclared(shape,words)}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
    if(BigInt(value.values.length)>=value.capacity)return 0n;
    if(refusal==='push-allocation')return 0n;
    try{value.values.push(row)}catch(error){if(error instanceof RangeError)return 0n;throw error}
    value.identity=identity;value.shape=shape;
    return move(handle,value);
  },
  spx_vec_leaf_clone_at_v1:(handle,identity,shape,index,out)=>{
    const value=bind(handle,identity,shape),spec=fields(shape);
    if(index<0n||index>=BigInt(value.values.length))return 2;
    const target=view(out,64),row=value.values[Number(index)],clones=[];
    try {
      const words=[];
      for(const field of spec){
        let word=row[field.slot];
        if(field.code>=9&&word!==0n){
          if(refusal==='string-clone-allocation'&&field.code===10)
            throw new RangeError('injected String clone allocation');
          if(refusal==='bytes-clone-allocation'&&field.code===9)
            throw new RangeError('injected Bytes clone allocation');
          if(refusal==='second-clone'&&clones.length===1)
            throw new RangeError('injected second clone allocation');
          word=alloc(word);clones.push(word);
        }
        words.push(word);
      }
      for(let i=0;i<8;i++)target.setBigInt64(i*8,BigInt.asIntN(64,words[i]??0n),true);
      value.identity=identity;value.shape=shape;
      return 0;
    } catch(error) {
      for(const word of clones)drop(word);
      if(error instanceof RangeError)return 1;
      throw error;
    }
  },
  spx_vec_leaf_replace_v1:(handle,identity,shape,index,...words)=>{
    const value=bind(handle,identity,shape);
    if(index<0n||index>=BigInt(value.values.length))return 0n;
    let row;
    try{row=rowFromDeclared(shape,words)}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
    if(refusal==='replacement-allocation')return 0n;
    const old=value.values[Number(index)];
    value.values[Number(index)]=row;value.identity=identity;value.shape=shape;
    dropRow(old,shape);
    return move(handle,value);
  },
  spx_vec_leaf_reserve_v1:(handle,identity,shape,additional)=>{
    const value=bind(handle,identity,shape);
    const target=BigInt(value.values.length)+additional;
    if(additional<0n||target>limit(shape))return 0n;
    if(target>value.capacity){
      if(refusal==='reserve-allocation')return 0n;
      let replacement;
      try{replacement=value.values.slice()}catch(error){if(error instanceof RangeError)return 0n;throw error}
      value.values=replacement;value.capacity=target;
    }
    value.identity=identity;value.shape=shape;
    return move(handle,value);
  },
  spx_vec_leaf_sort_v1:(handle,identity,shape)=>{
    const value=bind(handle,identity,shape);
    if(value.values.length<2){value.identity=identity;value.shape=shape;return move(handle,value)}
    if(refusal==='sort-allocation')return 0n;
    let replacement;
    try{replacement=value.values.slice().sort((a,b)=>compareRows(a,b,shape))}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
    value.values=replacement;
    value.identity=identity;value.shape=shape;
    return move(handle,value);
  },
  spx_vec_leaf_into_iter_v1:(handle,identity,shape,out)=>{
    const value=bind(handle,identity,shape),target=view(out,16);
    if(refusal==='iterator-allocation')return 3;
    const next=nextIter++;
    try{iterators.set(next,{identity,shape,rows:value.values,cursor:0n})}
    catch(error){if(error instanceof RangeError)return 3;throw error}
    vectors.delete(handle);
    target.setBigUint64(0,next,true);target.setBigUint64(8,0n,true);
    return 0;
  },
  spx_vec_leaf_iter_next_v1:(handle,cursor,identity,shape,out)=>{
    const value=iterator(handle,cursor,identity,shape),target=view(out,88);
    if(refusal==='iterator-next-allocation')return 3;
    const words=Array(11).fill(0n);
    if(cursor<BigInt(value.rows.length)){
      const row=value.rows[Number(cursor)],spec=fields(shape),next=nextIter++;
      words[0]=1n;
      for(let i=0;i<spec.length;i++)words[i+1]=row[spec[i].slot];
      words[9]=next;words[10]=cursor+1n;
      try{iterators.set(next,{...value,cursor:cursor+1n})}
      catch(error){if(error instanceof RangeError)return 3;throw error}
    }
    iterators.delete(handle);
    for(let i=0;i<11;i++)target.setBigUint64(i*8,BigInt.asUintN(64,words[i]),true);
    return 0;
  },
  spx_vec_leaf_iter_drop_v1:(handle,cursor,identity,shape)=>{
    const value=iterator(handle,cursor,identity,shape);
    for(let i=Number(cursor);i<value.rows.length;i++)dropRow(value.rows[i],shape);
    iterators.delete(handle);
  },
  spx_iter_record_into_v3:(handle,out)=>{
    const value=vec(handle,10),target=view(out,16),next=nextIter++;
    vectors.delete(handle);
    iterators.set(next,{legacy:true,rows:value.values,cursor:0n,shape:value.shape});
    target.setBigUint64(0,next,true);target.setBigUint64(8,0n,true);
    return 0;
  },
  spx_iter_record_next_v3:(handle,cursor,out,b0,b1,s,rest)=>{
    const value=iterators.get(handle);
    if(!value||!value.legacy||value.cursor!==cursor)throw Error('stale legacy iterator');
    const target=view(out,Math.max(b0+8,b1+8,s+8,rest+16));
    target.setUint32(0,0,true);target.setUint32(4,0,true);
    for(const offset of [b0,b1,s,rest,rest+8])target.setBigUint64(offset,0n,true);
    if(cursor===BigInt(value.rows.length)){iterators.delete(handle);return 0}
    const row=value.rows[Number(cursor)],next=nextIter++;
    target.setUint32(0,1,true);
    target.setBigInt64(b0,row[0],true);target.setBigInt64(b1,row[1],true);
    target.setBigInt64(s,row[2],true);
    target.setBigUint64(rest,next,true);target.setBigUint64(rest+8,cursor+1n,true);
    iterators.set(next,{...value,cursor:cursor+1n});iterators.delete(handle);
    return 0;
  },
  spx_iter_record_drop_v3:(handle,cursor)=>{
    const value=iterators.get(handle);
    if(!value||!value.legacy||value.cursor!==cursor)throw Error('stale legacy iterator drop');
    for(let i=Number(cursor);i<value.rows.length;i++){
      if(value.shape===undefined){drop(value.rows[i][0]);drop(value.rows[i][1])}
      else dropRow(value.rows[i],value.shape);
    }
    iterators.delete(handle);
  },
};
(async()=>{
  const missing={...env};delete missing.spx_vec_leaf_clone_at_v1;
  let refused=false;
  try{await WebAssembly.instantiate(moduleBytes,{env:missing})}
  catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;refused=true}
  if(!refused)throw Error('private boundary linked without clone import');
  ({instance}=await WebAssembly.instantiate(moduleBytes,{env}));
  for(let run=0;run<3;run++){
    let selected=0,returned;
    try{returned=instance.exports.semaprax_main()}
    catch(error){if(!error.message.startsWith('status:'))throw error;selected=Number(error.message.slice(7))}
    if(selected!==expectedStatus)throw Error(`status ${selected}, expected ${expectedStatus}`);
    if(!selected&&returned!==expectedValue)throw Error(`value ${returned}, expected ${expectedValue}`);
    if(vectors.size||iterators.size||payloads.size||copies!==drops)
      throw Error(`leaks vec=${vectors.size} iter=${iterators.size} payload=${payloads.size} copies=${copies} drops=${drops}`);
  }
  if(refusal==='none'){
    const old=env.spx_vec_leaf_new_v1(11n,0xa0n,0n);
    if(!old)throw Error('empty private vector allocation refused');
    for(const [identity,shape] of [[12n,0xa0n],[11n,0x90n]]){
      let rejected=false;
      try{env.spx_vec_leaf_sort_v1(old,identity,shape)}catch(error){rejected=true}
      if(!rejected)throw Error('wrong nominal or shape entered private carrier');
    }
    const fresh=env.spx_vec_leaf_sort_v1(old,11n,0xa0n);
    let stale=false;
    try{env.spx_vec_len_v2(old,11)}catch(error){stale=true}
    if(!stale)throw Error('consumed Vec generation remained live');
    env.spx_vec_drop_v2(fresh);
    const iter=nextIter++;
    iterators.set(iter,{identity:11n,shape:0xa0n,rows:[],cursor:0n});
    for(const [cursor,identity] of [[1n,11n],[0n,12n]]){
      let rejected=false;
      try{env.spx_vec_leaf_iter_drop_v1(iter,cursor,identity,0xa0n)}catch(error){rejected=true}
      if(!rejected)throw Error('wrong iterator epoch or nominal was accepted');
    }
    env.spx_vec_leaf_iter_drop_v1(iter,0n,11n,0xa0n);
    const typed=env.spx_vec_leaf_new_v1(13n,0xa180n,1n);
    const text=ownBytes(new TextEncoder().encode('x'));
    let badBool=false;
    try{env.spx_vec_leaf_push_v1(typed,13n,0xa180n,2n,text,0n,0n,0n,0n,0n,0n)}
    catch(error){badBool=true}
    if(!badBool)throw Error('noncanonical Bool entered owned-leaf row');
    drop(text);env.spx_vec_drop_v2(typed);
    if(vectors.size||iterators.size||payloads.size)throw Error('host self-check leaked an owner');
  }
})().catch(error=>{console.error(error);process.exitCode=2});
