// Entries own exactly one UTF-8 buffer. No bulk clear can conceal a missing drop.
function createArena(fail,isPoisoned){
  const entries=new Map(),limits=DESCRIPTOR.limits;
  let nextToken=1,liveBytes=0,cumulative=0,cause=null,memory=null,buffer=null,active=false;
  function checkedMemory(){
    if(isPoisoned()||memory===null||memory.buffer!==buffer||buffer.byteLength!==262144)fail();
    return buffer;
  }
  function requireActive(){if(isPoisoned()||!active)fail()}
  function authenticate(carrier){
    requireActive();
    if(typeof carrier!=="bigint"||carrier<=0n||carrier>0x7fffffffffffffffn)fail();
    const token=Number(carrier>>32n),length=Number(carrier&0xffffffffn),bytes=entries.get(token);
    if(token===0||bytes===undefined||bytes.length!==length)fail();
    return {token,bytes};
  }
  function reserve(length){
    requireActive();
    if(!Number.isInteger(length)||length<0)fail();
    // A mint after the compiler has observed a refusal violates the immediate branch contract.
    if(cause!==null)fail();
    const refusal=entries.size>=limits.max_live_owners?"owners":
      length>limits.max_string_bytes?"value_bytes":
      liveBytes+length>limits.max_live_bytes?"live_bytes":
      cumulative+length>limits.max_cumulative_bytes?"cumulative_bytes":
      nextToken>0x7fffffff?"tokens":null;
    if(refusal!==null){cause=refusal;return false}
    return true;
  }
  function mint(length,fill){
    if(!reserve(length))return 0n;
    const bytes=new Bytes(length);fill(bytes);
    const token=nextToken++;
    entries.set(token,bytes);liveBytes+=length;cumulative+=length;
    return (BigInt(token)<<32n)|BigInt(length);
  }
  // Reject overlong encodings, surrogates, truncation and values above U+10FFFF.
  function scalarCount(bytes){
    let count=0;
    for(let i=0;i<bytes.length;count++){
      const first=bytes[i++];let extra,min,scalar;
      if(first<128)continue;
      if(first>=194&&first<=223){extra=1;min=128;scalar=first&31}
      else if(first>=224&&first<=239){extra=2;min=2048;scalar=first&15}
      else if(first>=240&&first<=244){extra=3;min=65536;scalar=first&7}
      else fail();
      if(i+extra>bytes.length)fail();
      for(let j=0;j<extra;j++){
        const byte=bytes[i++];if((byte&192)!==128)fail();scalar=(scalar<<6)|(byte&63);
      }
      if(scalar<min||scalar>1114111||(scalar>=55296&&scalar<=57343))fail();
    }
    return count;
  }
  function prefix(a,b,offset){
    if(offset+b.length>a.length)return false;
    for(let i=0;i<b.length;i++)if(a[offset+i]!==b[i])return false;
    return true;
  }
  const operations={
    literal(pointer,length){
      requireActive();
      if(!Number.isInteger(pointer)||!Number.isInteger(length)||pointer<196608||length<0||pointer>262144-length)fail();
      const source=new Bytes(checkedMemory(),pointer,length); // View only, no payload allocation.
      scalarCount(source);
      return mint(length,out=>apply(byteSet,out,[source,0]));
    },
    clone(carrier){const {bytes}=authenticate(carrier);return mint(bytes.length,out=>apply(byteSet,out,[bytes,0]))},
    concat(left,right){
      const a=authenticate(left).bytes,b=authenticate(right).bytes;
      return mint(a.length+b.length,out=>{apply(byteSet,out,[a,0]);apply(byteSet,out,[b,a.length])});
    },
    from_char(scalar){
      requireActive();
      if(!Number.isInteger(scalar)||scalar<0||scalar>1114111||(scalar>=55296&&scalar<=57343))fail();
      const length=scalar<128?1:scalar<2048?2:scalar<65536?3:4;
      return mint(length,out=>{
        if(length===1)out[0]=scalar;
        else if(length===2){out[0]=192|(scalar>>6);out[1]=128|(scalar&63)}
        else if(length===3){out[0]=224|(scalar>>12);out[1]=128|((scalar>>6)&63);out[2]=128|(scalar&63)}
        else{out[0]=240|(scalar>>18);out[1]=128|((scalar>>12)&63);out[2]=128|((scalar>>6)&63);out[3]=128|(scalar&63)}
      });
    },
    byte_len(carrier){return BigInt(authenticate(carrier).bytes.length)},
    char_len(carrier){return BigInt(scalarCount(authenticate(carrier).bytes))},
    eq(left,right){const a=authenticate(left).bytes,b=authenticate(right).bytes;return a.length===b.length&&prefix(a,b,0)?1:0},
    starts_with(value,prefixHandle){return prefix(authenticate(value).bytes,authenticate(prefixHandle).bytes,0)?1:0},
    contains(value,needle){
      const a=authenticate(value).bytes,b=authenticate(needle).bytes;
      // Constant workspace; bounded buffers, not a wall-time or fuel guarantee.
      for(let i=0;i<=a.length-b.length;i++)if(prefix(a,b,i))return 1;
      return 0;
    },
    drop(carrier){
      const {token,bytes}=authenticate(carrier);
      if(!entries.delete(token)||liveBytes<bytes.length)fail();
      liveBytes-=bytes.length;
    }
  };
  const imports=Object.create(null);
  for(const name of IMPORT_NAMES)imports[name]=(...args)=>{try{return operations[name](...args)}catch{fail()}};
  return Object.freeze({
    imports:Object.freeze(imports),
    bind(value){if(isPoisoned()||memory!==null||!(value instanceof WebAssembly.Memory))fail();memory=value;buffer=value.buffer;checkedMemory()},
    begin(){checkedMemory();if(active||entries.size!==0||liveBytes!==0)fail();cumulative=0;cause=null;active=true},
    settle(status){
      checkedMemory();
      if(!active||entries.size!==0||liveBytes!==0||(status===11)!==(cause!==null))fail();
      active=false;return cause;
    }
  });
}
