'use strict';
// Strict private Core-Wasm host for the additive owned-leaf Vec profile.
// Every handle is a one-generation authority; an operation that returns zero
// has consumed nothing. The compiler's status/cleanup path owns that failure.
const fs = require('fs');
const moduleBytes = fs.readFileSync(process.argv[2]);
const expectedStatus = Number(process.argv[3]);
const expectedValue = BigInt(process.argv[4]);
const refusal = process.argv[5] || '';
// Dedicated private scalar String/Bytes witness, not a public Web adapter.
const scalarView = refusal === 'scalar-view';
const expectedDomain = scalarView ? process.argv[6] : null;
if (scalarView && !['ok', 'semaprax.convert.v1'].includes(expectedDomain))
  throw Error('scalar-view requires an exact admitted normalized status domain');
const codecPushFailure = /^codec-push-([1-4])$/.exec(refusal);
let codecPushAttempts = 0;
let instance, fieldOutput, nextPayload = 1, nextVec = 1n, nextIter = 1n << 62n, copies = 0, drops = 0;
const payloads = new Map(), vectors = new Map(), iterators = new Map();
// Fixture authority encoding v2: old mint/move keeps its generation-zero
// low-word handle. Additive sort renews the existing authority slot in place.
// Import signatures, legacy tags and generated legacy modules are unchanged.
const AUTHORITY_ENCODING = 'semaprax.test.owned-leaf-authority.v2';
const EMPTY_BYTES = new Uint8Array(0);
let sorting = false, fieldReading = false, fieldReads = 0;
const allocationOutsideSort = name => {
  if(sorting||fieldReading)throw Error(`infallible read/sort reached allocating/settling helper: ${name}`);
};
// Test-only guards make a future slice/sort/map or authority re-mint fail the
// same executable corpus, rather than quietly reintroducing a failure lane.
for(const name of ['slice','sort','map','filter']){
  const original=Array.prototype[name];
  Array.prototype[name]=function(...args){
    allocationOutsideSort(name);return Reflect.apply(original,this,args);
  };
}
const authoritySet=vectors.set;
vectors.set=function(key,value){
  allocationOutsideSort('authority insertion');return authoritySet.call(this,key,value);
};
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
  allocationOutsideSort('payload view');
  const {length, origin} = split(carrier);
  if (origin & 0x80000000) {
    const bytes = payloads.get(origin & 0x7fffffff);
    if (!bytes || bytes.length !== length) throw Error('stale payload');
    return bytes;
  }
  if ((origin & 0xc0000000) === 0x40000000) {
    const pointer=(origin & 0xffff)*8;
    if(pointer>131072-32)throw Error('range descriptor bounds');
    const descriptor=view(pointer,32),identity=descriptor.getUint32(0,true);
    if(!identity||identity!==((origin>>>16)&0x1fff)||descriptor.getUint32(4,true)!==pointer)
      throw Error('range descriptor identity');
    const carrier=descriptor.getBigInt64(8,true),ultimate=split(carrier);
    const start=descriptor.getBigUint64(16,true),extent=descriptor.getBigUint64(24,true);
    if((ultimate.origin & 0xc0000000)===0x40000000||extent!==BigInt(length)||
       start>BigInt(ultimate.length)||extent>BigInt(ultimate.length)-start)
      throw Error('range descriptor extent');
    return read(carrier).subarray(Number(start),Number(start+extent));
  }
  if (origin > memory().buffer.byteLength - length) throw Error('borrowed payload range');
  return new Uint8Array(memory().buffer, origin, length);
};
const alloc = carrier => {
  allocationOutsideSort('payload clone');
  const bytes = new Uint8Array(read(carrier));
  const id = nextPayload++;
  payloads.set(id, bytes);
  copies++;
  return BigInt.asIntN(64, ((0x80000000n | BigInt(id)) << 32n) | BigInt(bytes.length));
};
const ownBytes = bytes => {
  allocationOutsideSort('owned payload');
  const id = nextPayload++;
  payloads.set(id, new Uint8Array(bytes));
  copies++;
  return BigInt.asIntN(64, ((0x80000000n | BigInt(id)) << 32n) | BigInt(bytes.length));
};
const drop = carrier => {
  allocationOutsideSort('payload drop');
  if (carrier === 0n) return;
  read(carrier);
  const {origin} = split(carrier);
  if (!(origin & 0x80000000) || !payloads.delete(origin & 0x7fffffff))
    throw Error('double or borrowed payload drop');
  drops++;
};
const validUtf8 = bytes => {
    for (let index = 0; index < bytes.length;) {
      const first = bytes[index++];
      let extra, minimum, scalar;
      if (first < 128) continue;
      if (first >= 194 && first <= 223) { extra = 1; minimum = 128; scalar = first & 31; }
      else if (first >= 224 && first <= 239) { extra = 2; minimum = 2048; scalar = first & 15; }
      else if (first >= 240 && first <= 244) { extra = 3; minimum = 65536; scalar = first & 7; }
      else return false;
      if (index + extra > bytes.length) return false;
      for (let count = 0; count < extra; count++) {
        const byte = bytes[index++];
        if ((byte & 192) !== 128) return false;
        scalar = (scalar << 6) | (byte & 63);
      }
      if (scalar < minimum || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) return false;
    }
    return true;
  };

const fields = shape => {
  allocationOutsideSort('descriptor array');
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
const vectorValue = handle => {
  const raw=BigInt.asUintN(64,handle),slot=raw&0xffffffffn;
  const value=vectors.get(slot);
  return value&&value.generation===(raw>>32n)?value:undefined;
};
const releaseVector = handle => {
  allocationOutsideSort('authority release');
  const value=vectorValue(handle);
  if(!value||!vectors.delete(value.authority_slot))throw Error('stale Vec release');
};
const vec = (handle, tag) => {
  const value = vectorValue(handle);
  if (!value || value.tag !== tag) throw Error('stale or mistyped Vec');
  return value;
};
const bind = (handle, identity, shape) => {
  const value = vectorValue(handle);
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
  allocationOutsideSort('authority mint');
  if(nextVec>0xffffffffn)throw Error('Vec authority identity exhausted');
  const handle = nextVec++;
  value.authority_slot=handle;value.generation=0n;
  // Predeclare even an unbound legacy record's descriptor fields. Its first
  // additive sort only writes existing metadata; no owner object is grown.
  if(value.identity===undefined)value.identity=undefined;
  if(value.shape===undefined)value.shape=undefined;
  vectors.set(handle, value);
  return handle;
};
const move = (handle, value) => {
  releaseVector(handle);
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
const ownedSortBytes = word => {
  if(word===0n)return EMPTY_BYTES;
  const raw=BigInt.asUintN(64,word),origin=Number(raw>>32n),length=Number(raw&0xffffffffn);
  if(!(origin&0x80000000))throw Error('borrowed payload in owned sort');
  const bytes=payloads.get(origin&0x7fffffff);
  if(!bytes||bytes.length!==length)throw Error('stale sort payload');
  return bytes;
};
const compareOwnedBytes = (a,b) => {
  const left=ownedSortBytes(a),right=ownedSortBytes(b),bound=Math.min(left.length,right.length);
  for(let i=0;i<bound;i++)if(left[i]!==right[i])return left[i]-right[i];
  return left.length-right.length;
};
const compareRows = (left, right, shape) => {
  // Read the packed descriptor without building arrays, decoded strings or
  // borrowed views. Both String and Bytes compare their retained owned bytes.
  for(let raw=BigInt.asUintN(64,shape);raw;raw>>=8n) {
    const field=Number(raw&255n),code=field>>4,slot=field&15;
    const a=left[slot],b=right[slot];
    const cmp=code>=9?compareOwnedBytes(a,b):
      (totalKey(a,code)<totalKey(b,code)?-1:totalKey(a,code)>totalKey(b,code)?1:0);
    if (cmp) return cmp;
  }
  return 0;
};
const bindSort = (handle,identity,shape) => {
  const value=vectorValue(handle);
  if(!value||(value.tag!==10&&value.tag!==11))throw Error('stale owned sort vector');
  let count=0,owned=0,seen=0,bytes=0,scalars=0;
  for(let raw=BigInt.asUintN(64,shape);raw;raw>>=8n) {
    const field=Number(raw&255n),code=field>>4,slot=field&15;
    if(!field||code<1||code>10||slot>=8||(seen&(1<<slot)))throw Error('invalid sort shape');
    count++;seen|=1<<slot;
    if(code>=9)owned++;
    if(value.tag===10){
      if(code===9&&slot<=1)bytes++;
      else if(code<=8&&slot===2)scalars++;
      else throw Error('legacy sort shape mismatch');
    }
  }
  if(!count||owned<1||owned>2||seen!==((1<<count)-1))throw Error('invalid owned sort shape');
  if(value.tag===10&&(count!==3||bytes!==2||scalars!==1))throw Error('legacy sort shape mismatch');
  if(value.shape!==undefined&&(value.shape!==shape||value.identity!==identity))
    throw Error('sort declaration identity or shape mismatch');
  if(value.generation===0xffffffffn)throw Error('Vec generation exhausted');
  return value;
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
  spx_string_from_utf8_v1:(carrier,offset)=>{
    const bytes=read(carrier),output=view(offset,8);
    if(!validUtf8(bytes))return 21;
    output.setBigInt64(0,ownBytes(bytes),true);return 0;
  },
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
    const value=vectorValue(handle);if(!value)throw Error('stale Vec drop');
    if(value.tag===9)for(const word of value.values)drop(word);
    else if(value.shape===undefined&&value.tag===10){for(const row of value.values){drop(row[0]);drop(row[1])}}
    else if(value.shape!==undefined)for(const row of value.values)dropRow(row,value.shape);
    releaseVector(handle);
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
    const spec=fields(shape);
    const legacy=spec.length===3&&spec.filter(field=>field.code===9).length===2
      &&spec.filter(field=>field.code<=8).length===1
      &&spec.filter(field=>field.code===9).every(field=>field.slot<=1)
      &&spec.find(field=>field.code<=8).slot===2;
    try{return mint({tag:legacy?10:11,identity,shape,capacity,values:[]})}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
  },
  spx_vec_leaf_field_read_v1:(handle,identity,shape,index,position,out)=>{
    // No descriptor construction or binding here. Construction/push already
    // validated each field and payload; a read requires exact existing facts.
    const value=vectorValue(handle);
    if(!value||(value.tag!==10&&value.tag!==11)||value.identity!==identity
       ||value.shape!==shape)throw Error('unbound, stale or mismatched field-read authority');
    if(position<0n||position>=8n)throw Error('invalid field-read position');
    const byte=Number((BigInt.asUintN(64,shape)>>(position*8n))&255n);
    const code=byte>>4,slot=byte&15;
    if(code<1||code>10||slot>=8)
      throw Error('invalid field-read descriptor');
    if(index<0n||index>=BigInt(value.values.length))return 2;
    const row=value.values[Number(index)],word=row[slot];
    const beforeCopies=copies,beforeDrops=drops,beforePayload=nextPayload,beforeVec=nextVec;
    fieldReading=true;
    try{
      if(code>=9&&word!==0n){
        const raw=BigInt.asUintN(64,word),origin=raw>>32n,length=Number(raw&0xffffffffn);
        if(!(origin&0x80000000n))throw Error('borrowed field-read leaf');
        const payload=payloads.get(Number(origin&0x7fffffffn));
        if(!payload||payload.length!==length)throw Error('stale field-read leaf');
      }else if(code<9)checkWord(word,code);
      if(copies!==beforeCopies||drops!==beforeDrops||nextPayload!==beforePayload
         ||nextVec!==beforeVec||vectorValue(handle)!==value||value.values[Number(index)]!==row)
        throw Error('field-read changed authority or allocation inventory');
      fieldReads++;
      const result=refusal==='field-bad-word'?(code>=9?1n:0x100000000n):word;
      if(!Number.isInteger(out)||out<0||!fieldOutput||fieldOutput.buffer!==memory().buffer
         ||out>fieldOutput.byteLength-8)throw Error('field-read output range');
      fieldOutput.setBigInt64(out,BigInt.asIntN(64,result),true);
      return 0;
    }finally{fieldReading=false}
  },
  spx_vec_leaf_push_v1:(handle,identity,shape,...words)=>{
    const value=bind(handle,identity,shape);
    let row;
    try{row=rowFromDeclared(shape,words)}
    catch(error){if(error instanceof RangeError)return 0n;throw error}
    if(BigInt(value.values.length)>=value.capacity)return 0n;
    if(refusal==='push-allocation')return 0n;
    codecPushAttempts++;
    if(codecPushFailure&&codecPushAttempts===Number(codecPushFailure[1]))return 0n;
    try{value.values.push(row)}catch(error){if(error instanceof RangeError)return 0n;throw error}
    value.identity=identity;value.shape=shape;
    return move(handle,value);
  },
  spx_vec_leaf_clone_at_v1:(handle,identity,shape,index,out)=>{
    const value=bind(handle,identity,shape),spec=fields(shape);
    if(index<0n||index>=BigInt(value.values.length))return 2;
    const target=view(out,64),row=value.values[Number(index)],clones=[0n,0n];
    let cloneCount=0;
    try {
      const words=[];
      for(const field of spec){
        let word=row[field.slot];
        if(field.code>=9&&word!==0n){
          if(refusal==='string-clone-allocation'&&field.code===10)
            throw new RangeError('injected String clone allocation');
          if(refusal==='bytes-clone-allocation'&&field.code===9)
            throw new RangeError('injected Bytes clone allocation');
          if(refusal==='second-clone'&&cloneCount===1)
            throw new RangeError('injected second clone allocation');
          word=alloc(word);clones[cloneCount++]=word;
        }
        words.push(word);
      }
      for(let i=0;i<8;i++)target.setBigInt64(i*8,BigInt.asIntN(64,words[i]??0n),true);
      value.identity=identity;value.shape=shape;
      return 0;
    } catch(error) {
      for(let i=0;i<cloneCount;i++)drop(clones[i]);
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
    if(sorting)throw Error('reentrant owned sort');
    sorting=true;
    try {
      const value=bindSort(handle,identity,shape),rows=value.values;
      const copied=copies,dropped=drops,authorities=vectors.size;
      for(let i=1;i<rows.length;i++){
        const row=rows[i];let j=i;
        while(j>0&&compareRows(rows[j-1],row,shape)>0){rows[j]=rows[j-1];j--}
        rows[j]=row;
      }
      value.identity=identity;value.shape=shape;value.generation++;
      if(copies!==copied||drops!==dropped||value.values!==rows||vectors.size!==authorities)
        throw Error('sort allocated, copied or settled an owner');
      return BigInt.asIntN(64,(value.generation<<32n)|value.authority_slot);
    } finally {sorting=false}
  },
  spx_vec_leaf_into_iter_v1:(handle,identity,shape,out)=>{
    const value=bind(handle,identity,shape),target=view(out,16);
    if(refusal==='iterator-allocation')return 3;
    const next=nextIter++;
    try{iterators.set(next,{identity,shape,rows:value.values,cursor:0n})}
    catch(error){if(error instanceof RangeError)return 3;throw error}
    releaseVector(handle);
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
    releaseVector(handle);
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
  if(scalarView){
    const imports=WebAssembly.Module.imports(new WebAssembly.Module(moduleBytes));
    if(imports.some(entry=>/^spx_(vec|iter)_/.test(entry.name)) ||
       !imports.some(entry=>/^spx_(string|bytes)_/.test(entry.name)))
      throw Error('scalar-view fixture has a collection boundary or no owned payload import');
  }else{
  const missing={...env};delete missing.spx_vec_leaf_clone_at_v1;
  let refused=false;
  try{await WebAssembly.instantiate(moduleBytes,{env:missing})}
  catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;refused=true}
  if(!refused)throw Error('private boundary linked without clone import');
  }
  if(refusal==='field-no-allocation'){
    const withoutRead={...env};delete withoutRead.spx_vec_leaf_field_read_v1;
    let absent=false;
    try{await WebAssembly.instantiate(moduleBytes,{env:withoutRead})}
    catch(error){if(!(error instanceof WebAssembly.LinkError))throw error;absent=true}
    if(!absent)throw Error('read module linked without its conditional private import');
  }
  if(refusal==='field-bad-word'||refusal==='field-forged-shape'||refusal==='field-unknown-status'){
    let calls=0,trapped=false;
    const actual=env.spx_vec_leaf_field_read_v1;
    const hostile={...env,spx_vec_leaf_field_read_v1:(...args)=>{
      calls++;
      if(refusal==='field-unknown-status')return 1;
      if(refusal==='field-forged-shape')args[2]^=1n;
      return actual(...args);
    }};
    ({instance}=await WebAssembly.instantiate(moduleBytes,{env:hostile}));
    fieldOutput=new DataView(memory().buffer);
    try{instance.exports.semaprax_main()}
    catch(error){
      if(refusal!=='field-forged-shape'&&!(error instanceof WebAssembly.RuntimeError))throw error;
      if(refusal==='field-forged-shape'&&error.message!=='unbound, stale or mismatched field-read authority')throw error;
      trapped=true;
    }
    if(!trapped||calls!==1)throw Error('forged field-read result/descriptor did not trap');
    for(const owner of vectors.values())
      env.spx_vec_drop_v2(BigInt.asIntN(64,(owner.generation<<32n)|owner.authority_slot));
    if(payloads.size||vectors.size||iterators.size||copies!==drops)throw Error('field trap teardown leaked');
    return;
  }
  if(refusal==='sort-null'){
    let calls=0,trapped=false;
    const hostile={...env,spx_vec_leaf_sort_v1:()=>{calls++;return 0n}};
    ({instance}=await WebAssembly.instantiate(moduleBytes,{env:hostile}));
    fieldOutput=new DataView(memory().buffer);
    try{instance.exports.semaprax_main()}
    catch(error){if(!(error instanceof WebAssembly.RuntimeError))throw error;trapped=true}
    if(!trapped||calls!==1)throw Error('null infallible sort result was not a host invariant trap');
    // A corrupted host is fail-stop, not a recoverable source failure. Release
    // the host's retained authority explicitly after checking the trap type.
    for(const owner of vectors.values())
      env.spx_vec_drop_v2(BigInt.asIntN(64,(owner.generation<<32n)|owner.authority_slot));
    if(payloads.size||vectors.size||iterators.size||copies!==drops)throw Error('host trap teardown leaked');
    return;
  }
  ({instance}=await WebAssembly.instantiate(moduleBytes,{env}));
  fieldOutput=new DataView(memory().buffer);
  for(let run=0;run<3;run++){
    codecPushAttempts=0;fieldReads=0;
    let selected=0,returned;
    try{returned=instance.exports.semaprax_main()}
    catch(error){if(!error.message.startsWith('status:'))throw error;selected=Number(error.message.slice(7))}
    if(scalarView){
      const domain=selected===0?'ok':selected===21||selected===22?'semaprax.convert.v1':null;
      const code=selected===21||selected===22?selected-20:selected;
      if(domain!==expectedDomain||code!==expectedStatus)
        throw Error(`normalized status ${domain}|${code}, expected ${expectedDomain}|${expectedStatus}`);
      if(selected&&returned!==undefined)throw Error('failed scalar view published a result');
    }else if(selected!==expectedStatus)throw Error(`status ${selected}, expected ${expectedStatus}`);
    if(!selected&&returned!==expectedValue)throw Error(`value ${returned}, expected ${expectedValue}`);
    if(vectors.size||iterators.size||payloads.size||copies!==drops)
      throw Error(`leaks vec=${vectors.size} iter=${iterators.size} payload=${payloads.size} copies=${copies} drops=${drops}`);
    if(refusal==='field-index-failure'&&fieldReads!==0)throw Error('index failure accessed row storage');
    if(refusal==='field-no-allocation'&&fieldReads<64)throw Error('repeated read guard was not exercised');
    if(codecPushFailure&&codecPushAttempts!==Number(codecPushFailure[1]))
      throw Error('partial codec allocation failure did not reach its exact owning commit');
  }
  if(refusal==='none'||refusal==='sort-no-allocation'){
    if(AUTHORITY_ENCODING!=='semaprax.test.owned-leaf-authority.v2')throw Error('fixture authority version');
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
    let tied=env.spx_vec_leaf_new_v1(17n,0xa0n,2n);
    const a=ownBytes(new TextEncoder().encode('same'));
    const b=ownBytes(new TextEncoder().encode('same'));
    tied=env.spx_vec_leaf_push_v1(tied,17n,0xa0n,a,0n,0n,0n,0n,0n,0n,0n);
    tied=env.spx_vec_leaf_push_v1(tied,17n,0xa0n,b,0n,0n,0n,0n,0n,0n,0n);
    const owner=vectorValue(tied),rows=owner.values,first=rows[0],second=rows[1];
    const after=env.spx_vec_leaf_sort_v1(tied,17n,0xa0n);
    const again=env.spx_vec_leaf_sort_v1(after,17n,0xa0n);
    if(vectorValue(tied)||vectorValue(after)||vectorValue(again)!==owner||owner.values!==rows
       ||rows[0]!==first||rows[1]!==second||first[0]!==a||second[0]!==b)
      throw Error('stable sort changed row identity or retained a stale epoch');
    env.spx_vec_drop_v2(again);
    let inspected=env.spx_vec_leaf_new_v1(23n,0xa180n,1n);
    const inspectedText=ownBytes(new TextEncoder().encode('a\u0000é'));
    inspected=env.spx_vec_leaf_push_v1(inspected,23n,0xa180n,1n,inspectedText,0n,0n,0n,0n,0n,0n);
    const beforeCopies=copies,beforeDrops=drops,beforePayload=nextPayload,beforeVec=nextVec;
    const fieldOut=0;
    for(let i=0;i<64;i++){
      if(env.spx_vec_leaf_field_read_v1(inspected,23n,0xa180n,0n,0n,fieldOut)!==0
         ||view(fieldOut,8).getBigInt64(0,true)!==1n
         ||env.spx_vec_leaf_field_read_v1(inspected,23n,0xa180n,0n,1n,fieldOut)!==0
         ||view(fieldOut,8).getBigInt64(0,true)!==inspectedText)
        throw Error('field-read lost exact scalar/payload identity');
    }
    if(copies!==beforeCopies||drops!==beforeDrops||nextPayload!==beforePayload||nextVec!==beforeVec)
      throw Error('read allocated, cloned, dropped or renewed');
    for(const [handle,identity,shape,index,position] of [
      [inspected,24n,0xa180n,0n,0n],[inspected,23n,0xa180n^1n,0n,0n],
      [inspected,23n,0xa180n,0n,2n],
      [inspected,23n,0xa180n,0n,-1n],
    ]){
      let rejected=false;
      try{env.spx_vec_leaf_field_read_v1(handle,identity,shape,index,position,fieldOut)}catch(error){rejected=true}
      if(!rejected)throw Error('forged field-read facts were accepted');
    }
    for(const index of [1n,-1n,0xffffffffffffffffn]){
      view(fieldOut,8).setBigInt64(0,123n,true);
      if(env.spx_vec_leaf_field_read_v1(inspected,23n,0xa180n,index,0n,fieldOut)!==2
         ||view(fieldOut,8).getBigInt64(0,true)!==123n)
        throw Error('index refusal mutated output or lost Vec/2 status');
    }
    const consumed=inspected;
    inspected=env.spx_vec_leaf_sort_v1(inspected,23n,0xa180n);
    let staleRead=false;
    try{env.spx_vec_leaf_field_read_v1(consumed,23n,0xa180n,0n,0n,fieldOut)}catch(error){staleRead=true}
    if(!staleRead)throw Error('stale generation permitted a field read');
    env.spx_vec_drop_v2(inspected);
    for(const bits of [-0x8000000000000000n,0x7ff8000000001234n]){
      let floating=env.spx_vec_leaf_new_v1(29n,0xa170n,1n);
      floating=env.spx_vec_leaf_push_v1(floating,29n,0xa170n,bits,0n,0n,0n,0n,0n,0n,0n);
      if(env.spx_vec_leaf_field_read_v1(floating,29n,0xa170n,0n,0n,fieldOut)!==0
         ||view(fieldOut,8).getBigInt64(0,true)!==bits)
        throw Error('signed zero or NaN payload bits changed during field read');
      env.spx_vec_drop_v2(floating);
    }
    const unbound=env.spx_vec_with_capacity_v2(10,1n);
    let unboundRead=false;
    try{env.spx_vec_leaf_field_read_v1(unbound,1n,0x929110n,0n,0n,fieldOut)}catch(error){unboundRead=true}
    if(!unboundRead||vectorValue(unbound).shape!==undefined)
      throw Error('read lazily repaired an unbound legacy authority');
    env.spx_vec_drop_v2(unbound);
    if(vectors.size||iterators.size||payloads.size)throw Error('host self-check leaked an owner');
  }
})().catch(error=>{console.error(error);process.exitCode=2});
