function ownedResultSize(fact){
  if(fact.fields!==undefined)return fact.size;
  if(fact.result==="bool")return 4;
  return fact.result==="option-owned-bytes"||fact.result==="result-owned-bytes-i64"?16:8;
}
function stageOwnedResult(linked,fact,view){
  if(fact.fields!==undefined){
    const values=Object.create(null);let ownedCarrier=null;
    for(const field of fact.fields){
      if(field.kind==="owned-bytes")ownedCarrier=view.getBigInt64(RESULT+field.offset,true);
      else if(field.kind==="i64")values[field.name]=view.getBigInt64(RESULT+field.offset,true);
      else if(field.kind==="usize")values[field.name]=view.getBigUint64(RESULT+field.offset,true);
      else{
        const value=view.getBigUint64(RESULT+field.offset,true);
        if(value>1n)throw new Error("SEMAPRAX bool result invariant");values[field.name]=value===1n;
      }
    }
    linked.arena.check();const owned=linked.arena.consume(ownedCarrier);
    return ()=>{for(const field of fact.fields)if(field.kind==="owned-bytes")values[field.name]=owned;return Object.freeze(values)};
  }
  let answer;
  switch(fact.result??"owned-bytes"){
    case "i64":answer=view.getBigInt64(RESULT,true);break;
    case "usize":answer=view.getBigUint64(RESULT,true);break;
    case "bool":{
      const value=view.getUint32(RESULT,true);if(value>1)throw new Error("SEMAPRAX bool result invariant");answer=value===1;break;
    }
    case "owned-bytes":answer=linked.arena.consume(view.getBigInt64(RESULT,true));break;
    case "owned-utf8":{
      const bytes=linked.arena.consume(view.getBigInt64(RESULT,true));
      linked.arena.check();answer=new TextDecoder("utf-8",{fatal:true,ignoreBOM:true}).decode(bytes);break;
    }
    case "option-owned-bytes":{
      const tag=view.getUint32(RESULT,true);if(tag>1)throw new Error("SEMAPRAX owned variant tag invariant");
      answer=tag===0?null:linked.arena.consume(view.getBigInt64(RESULT+8,true));break;
    }
    case "result-owned-bytes-i64":{
      const tag=view.getUint32(RESULT,true);if(tag>1)throw new Error("SEMAPRAX owned variant tag invariant");
      if(tag===0){const value=linked.arena.consume(view.getBigInt64(RESULT+8,true));return ()=>Object.freeze({ok:true,value})}
      const error=view.getBigInt64(RESULT+8,true);return ()=>Object.freeze({ok:false,error});
    }
    default:throw new Error("unknown descriptor result type");
  }
  return ()=>answer;
}
