const FACTS=new Map([__SPX_FACTS__]),IDS=Object.freeze(Array.from(FACTS.keys())),RESULT=65536,POISON=0xa5;
function facade(linked){
  const invoke=createNestedOwnedInvocation(linked,FACTS,__SPX_MEMORY_BYTES__),functions=Object.create(null);
  for(const id of IDS)Object.defineProperty(functions,id,{value:(...values)=>invoke(id,values),enumerable:true});
  return Object.freeze({functions:Object.freeze(functions),call:(id,...values)=>invoke(id,values),wasmSha256});
}
export async function instantiate(bytes){return facade(await instantiateCore(bytes))}
export const exportIds=IDS;
export default instantiate;
