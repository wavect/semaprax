async function instantiateCore(input){
  const bytes=snapshotUint8(input,"SEMAPRAX module bytes");
  if(globalThis.crypto?.subtle===undefined)throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  const hash=new Uint8Array(await crypto.subtle.digest("SHA-256",bytes)),actual=Array.from(hash,v=>v.toString(16).padStart(2,"0")).join("");
  if(actual!==EXPECTED_WASM_SHA256)throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  const arena=createArena();
  // Imported exceptions never mint the facade's private recoverable identity.
  const fail=(code,domain)=>{throw Object.assign(new Error(`SEMAPRAX semantic failure ${code}`),{code,domain,semapraxSemantic:true})};
  const checked=(value,code)=>{if(value<-(1n<<63n)||value>(1n<<63n)-1n)fail(code,"semaprax.arithmetic.v1");return value};
  const scalar={
    spx_add:(a,b)=>checked(a+b,1),spx_sub:(a,b)=>checked(a-b,2),spx_mul:(a,b)=>checked(a*b,3),
    spx_div:(a,b)=>b===0n?fail(4,"semaprax.arithmetic.v1"):a/b,
    spx_rem:(a,b)=>b===0n?fail(6,"semaprax.arithmetic.v1"):a%b,
    spx_neg:a=>checked(-a,8),spx_contract_fail:code=>fail(code,"semaprax.contract.v1"),
  };
  const env=Object.create(null);
  for(const [name,operation] of Object.entries(scalar))env[name]=(...args)=>{
    arena.check();
    try{const result=operation(...args);arena.check();return result}
    catch(error){arena.poison();throw error}
  };
  Object.assign(env,arena.imports);
  const result=await WebAssembly.instantiate(bytes,Object.freeze({env:Object.freeze(env)}));
  arena.bind(result.instance);
  return Object.freeze({instance:result.instance,arena,copyInto:(target,source,offset)=>{
    arena.check();reflectApply(typedSet,target,[source,offset]);arena.check();
  }});
}
export const wasmSha256=EXPECTED_WASM_SHA256;
