export async function instantiate(input){
  const bytes=snapshotModule(input);
  if(globalThis.crypto?.subtle===undefined)throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  const hash=new Bytes(await globalThis.crypto.subtle.digest("SHA-256",bytes));
  let actual="";for(const byte of hash)actual+=byte.toString(16).padStart(2,"0");
  if(actual!==EXPECTED_SHA256)throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  const module=await WebAssembly.compile(bytes);validateModule(module);
  let poisoned=false,busy=false;
  function fail(){poisoned=true;throw new Error("SEMAPRAX internal String runtime is poisoned")}
  const arena=createArena(fail,()=>poisoned);
  const instance=await WebAssembly.instantiate(module,Object.freeze({"semaprax.internal-strings.v1":arena.imports}));
  const exports=instance.exports,memory=exports.memory,stack=exports.__spx_stack_pointer;
  if(!(memory instanceof WebAssembly.Memory)||!(stack instanceof WebAssembly.Global)||memory.buffer.byteLength!==262144||stack.value!==65536)fail();
  arena.bind(memory);
  const buffer=memory.buffer,view=new View(buffer),scratch=new Bytes(buffer,65536,8);
  const facts=new Map(DESCRIPTOR.exports.map(fact=>[fact.stable_id,fact]));
  function invoke(id,args){
    if(poisoned||busy)fail();
    if(typeof id!=="string"||!facts.has(id))throw new TypeError("SEMAPRAX export identity disagrees");
    const fact=facts.get(id);
    if(args.length!==fact.parameters.length)throw new TypeError("SEMAPRAX argument count disagrees");
    const raw=[];
    for(let i=0;i<args.length;i++){
      const value=args[i];
      if(fact.parameters[i]==="i64"){
        if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError("SEMAPRAX argument must be signed i64 bigint");
        raw.push(value);
      }else if(fact.parameters[i]==="bool"){
        if(typeof value!=="boolean")throw new TypeError("SEMAPRAX argument must be Boolean");
        raw.push(value?1:0);
      }else fail();
    }
    busy=true;
    try{
      if(memory.buffer!==buffer||stack.value!==65536)fail();
      for(const byte of scratch)if(byte!==0)fail();
      arena.begin();
      const status=exports[fact.wasm_export](...raw);
      if(poisoned||!Number.isInteger(status)||status<0||status>11||memory.buffer!==buffer||stack.value!==65536)fail();
      if(status!==0)for(const byte of scratch)if(byte!==0)fail();
      const cause=arena.settle(status);
      let result;
      if(status===0){
        let value;
        if(fact.result==="i64")value=view.getBigInt64(65536,true);
        else if(fact.result==="bool"){
          const scalar=view.getUint32(65536,true);
          if(scalar>1||view.getUint32(65540,true)!==0)fail();
          value=scalar===1;
        }else fail();
        result=Object.freeze({kind:"success",value});
      }else if(status===11)result=Object.freeze({kind:"capacity",cause});
      else result=Object.freeze({kind:"failure",domain:status<=8?"semaprax.arithmetic.v1":"semaprax.contract.v1",code:status<=8?status:status-8});
      scratch.fill(0);
      if(poisoned)fail();
      return result;
    }catch{fail()}
    finally{busy=false}
  }
  return Object.freeze({call:(id,...args)=>invoke(id,args)});
}
