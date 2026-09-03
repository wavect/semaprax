function nestedOwnedResultSize(fact){return fact.size}
function stageNestedOwnedResult(linked,fact,view){
  const staged=[];
  // Authenticate every scalar and carrier before consuming any owner. The
  // generated facts are closed, but keep this boundary self-defending.
  for(const leaf of fact.leaves){
    if(!Array.isArray(leaf.path)||leaf.path.length===0||leaf.path.length>64)throw new Error("SEMAPRAX nested result path invariant");
    for(const part of leaf.path)if(typeof part!=="string"||part.length===0)throw new Error("SEMAPRAX nested result field invariant");
    let value;
    if(leaf.kind==="owned-bytes")value=view.getBigInt64(RESULT+leaf.offset,true);
    else if(leaf.kind==="i64")value=view.getBigInt64(RESULT+leaf.offset,true);
    else if(leaf.kind==="usize")value=view.getBigUint64(RESULT+leaf.offset,true);
    else if(leaf.kind==="bool"){
      const raw=view.getBigUint64(RESULT+leaf.offset,true);
      if(raw>1n)throw new Error("SEMAPRAX bool result invariant");value=raw===1n;
    }else throw new Error("SEMAPRAX nested result type invariant");
    staged.push({path:leaf.path,value,owned:leaf.kind==="owned-bytes"});
  }
  linked.arena.check();
  // Copy every opaque owner before settlement. If any carrier is stale or
  // foreign the enclosing invocation poisons and no result object exists.
  const owned=staged.filter(leaf=>leaf.owned),copies=linked.arena.consumeMany(owned.map(leaf=>leaf.value));
  for(let index=0;index<owned.length;index++)owned[index].value=copies[index];
  linked.arena.check();
  return ()=>{
    const root=Object.create(null),nodes=[root],seen=new Set();
    for(const leaf of staged){
      let node=root,key="";
      for(let index=0;index<leaf.path.length;index++){
        const part=leaf.path[index];key+=`${part.length}:${part}`;
        if(index+1===leaf.path.length){
          if(seen.has(key)||Object.prototype.hasOwnProperty.call(node,part))throw new Error("SEMAPRAX nested result path collision");
          seen.add(key);Object.defineProperty(node,part,{value:leaf.value,enumerable:true});
        }else{
          if(Object.prototype.hasOwnProperty.call(node,part)){
            node=node[part];if(node===null||typeof node!=="object")throw new Error("SEMAPRAX nested result path prefix invariant");
          }else{
            const child=Object.create(null);Object.defineProperty(node,part,{value:child,enumerable:true});nodes.push(child);node=child;
          }
          key+="/";
        }
      }
    }
    for(let index=nodes.length-1;index>=0;index--)Object.freeze(nodes[index]);
    return root;
  };
}
