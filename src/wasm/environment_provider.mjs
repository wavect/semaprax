/** Explicit Environment I/O v1 provider. No ambient environment or process reads.
 * Supply {environment: [[name,value],... ] | null, arguments: [...], stdin: Uint8Array}.
 * Text accepts strings or UTF-8 Uint8Array values. Construction validates and
 * privately copies the complete snapshot; caller mutations cannot alter it.
 * attach(memory,{allocateOwned,validateOwned}) binds the reserved first 64 KiB.
 * The host must keep that input arena read-only during an invocation. Arbitrary
 * replacement imports or writes to exported memory are outside this adapter.
 */
export function createEnvironmentProvider({environment = null, arguments: argv = [], stdin = new Uint8Array()} = {}) {
  const limit = 65536, encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', {fatal:true, ignoreBOM:true});
  const fail = message => { throw new TypeError(`Environment I/O v1: ${message}`); };
  const text = (value, name) => {
    let bytes;
    if (typeof value === 'string') {
      if(value.length>limit)fail('text exceeds input bound');
      // TextEncoder replaces isolated UTF-16 surrogates; reject instead.
      for (let i=0;i<value.length;i++) {
        const c=value.charCodeAt(i);
        if(c>=0xd800&&c<=0xdbff) { const next=value.charCodeAt(++i); if(!(next>=0xdc00&&next<=0xdfff))fail('invalid UTF-16'); }
        else if(c>=0xdc00&&c<=0xdfff)fail('invalid UTF-16');
      }
      bytes=encoder.encode(value);
    } else if(value instanceof Uint8Array) {
      if(value.length>limit)fail('text exceeds input bound');
      bytes=new Uint8Array(value);
      try {decoder.decode(bytes);} catch {fail('invalid UTF-8');}
    } else fail('text must be a string or UTF-8 bytes');
    if(bytes.length>limit || bytes.includes(0) || (name && (!bytes.length || bytes.includes(61))))fail('invalid text bytes');
    return bytes;
  };
  if(!Array.isArray(argv)||argv.length>16)fail('argument count exceeds 16');
  if(!(stdin instanceof Uint8Array))fail('stdin must be bytes');
  let total=stdin.length;
  if(total>limit)fail('combined input exceeds 65536 bytes');
  const debit=bytes=>{total+=bytes.length;if(total>limit)fail('combined input exceeds 65536 bytes');return bytes;};
  const input=new Uint8Array(stdin);
  const argumentsCopy=argv.map(value=>debit(text(value,false)));
  let entries=null;
  if(environment!==null) {
    if(!Array.isArray(environment)||environment.length>256)fail('entry count exceeds 256');
    entries=environment.map(entry=>{
      if(!Array.isArray(entry)||entry.length!==2)fail('entry must contain name and value');
      return [debit(text(entry[0],true)),debit(text(entry[1],false))];
    });
    const compare=(a,b)=>{for(let i=0;i<Math.min(a.length,b.length);i++)if(a[i]!==b[i])return a[i]-b[i];return a.length-b.length;};
    entries.sort((a,b)=>compare(a[0],b[0]));
    for(let i=1;i<entries.length;i++)if(compare(entries[i-1][0],entries[i][0])===0)fail('duplicate name');
  }
  let memory,owned,usedStdin=false,argumentViews=[],entryViews=[];
  const carrier=(root,length)=>BigInt.asIntN(64,(BigInt(root)<<32n)|BigInt(length));
  const output=(pointer,size)=>{
    if(!memory || !Number.isInteger(pointer))throw new Error('environment memory is not attached');
    const offset=pointer>>>0;
    if(offset<limit || offset+size>memory.buffer.byteLength)throw new Error('environment output slot is outside writable scratch');
    return [new DataView(memory.buffer),offset];
  };
  const put64=(pointer,value)=>{const [view,offset]=output(pointer,8);view.setBigInt64(offset,value,true);};
  const index=(value,count)=>typeof value==='bigint' && value>=0n && value<BigInt(count);
  const lookup=(value,pointer,field)=>{
    if(entries===null)return 4;
    if(!index(value,entries.length))return 1;
    put64(pointer,entryViews[Number(value)][field]);return 0;
  };
  const imports=Object.freeze({
    spx_environment_len_v1(pointer) {if(entries===null)return 4;const [view,offset]=output(pointer,4);view.setUint32(offset,entries.length,true);return 0;},
    spx_environment_name_utf8_v1:(value,pointer)=>lookup(value,pointer,0),
    spx_environment_value_utf8_v1:(value,pointer)=>lookup(value,pointer,1),
    spx_command_args_len_v1:()=>BigInt(argumentsCopy.length),
    spx_command_arg_utf8_v1(value,pointer) {if(!index(value,argumentsCopy.length))return 1;put64(pointer,argumentViews[Number(value)]);return 0;},
    spx_command_stdin_read_v1(pointer) {
      if(usedStdin || typeof owned?.allocateOwned!=='function')return 3;
      output(pointer,8);
      usedStdin=true;
      const value=owned.allocateOwned(new Uint8Array(input));
      if(typeof value!=='bigint'||BigInt.asUintN(64,value)>>63n!==1n || Number(BigInt.asUintN(64,value)&0xffffffffn)!==input.length || !owned.validateOwned(value))throw new Error('invalid owned stdin carrier');
      put64(pointer,BigInt.asIntN(64,value));return 0;
    },
    spx_command_owned_bytes_validate_v1(value) {return typeof owned?.validateOwned==='function' && owned.validateOwned(value)?0:1;},
  });
  return Object.freeze({imports, byteLength:total,
    attach(target,allocator={}) {
      if(!(target instanceof WebAssembly.Memory)||target.buffer.byteLength<limit)fail('memory lacks input arena');
      if(allocator.allocateOwned!==undefined && (typeof allocator.allocateOwned!=='function'||typeof allocator.validateOwned!=='function'))fail('owned allocator requires validation');
      memory=target;owned=Object.freeze({allocateOwned:allocator.allocateOwned,validateOwned:allocator.validateOwned});usedStdin=false;
      let cursor=0;const arena=new Uint8Array(memory.buffer,0,limit);
      const publish=bytes=>{const root=cursor;arena.set(bytes,cursor);cursor+=bytes.length;return carrier(root,bytes.length);};
      argumentViews=argumentsCopy.map(publish);
      entryViews=entries===null?[]:entries.map(entry=>entry.map(publish));
    },
  });
}
