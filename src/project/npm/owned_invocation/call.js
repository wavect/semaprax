// One state machine for every existing v8/v9/v10 result shape.
function createOwnedInvocation(linked,facts,memoryBytes){
  let busy=false;
  function unavailable(){linked.arena.poison();throw new Error("SEMAPRAX owned-data runtime is poisoned")}
  return function invoke(id,values){
    linked.arena.check();if(busy)unavailable();
    let entered=false,began=false,settled=false,hasPrimary=false,primary;
    let hasSemantic=false,semanticError,answer,bytes=null,used=0,size=0;
    function select(error){if(!hasPrimary){hasPrimary=true;primary=error}}
    busy=true;
    try{
      const fact=facts.get(id);
      if(!fact)throw new RangeError(`unknown SEMAPRAX export: ${id}`);
      if(values.length!==fact.params.length)throw new TypeError("SEMAPRAX argument count disagrees");
      const admission=snapshotArguments(values,fact.params),snapshots=admission.snapshots;
      used=admission.used;linked.arena.check();
      entered=true;
      const e=linked.instance.exports,memory=e.memory;
      if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength!==memoryBytes)throw new Error("SEMAPRAX fixed owned-data memory invariant");
      size=ownedResultSize(fact);bytes=new Uint8Array(memory.buffer);
      const view=new DataView(memory.buffer),raw=[];let offset=0;
      for(const value of snapshots){
        if(value instanceof Uint8Array){linked.copyInto(bytes,value,offset);raw.push(offset,value.byteLength);offset+=value.byteLength}
        else raw.push(value);
      }
      bytes.fill(POISON,RESULT,RESULT+size);linked.arena.check();
      linked.arena.begin();began=true;
      const fn=e[fact.raw];if(typeof fn!=="function")throw new Error("SEMAPRAX raw adapter missing");
      linked.arena.check();const status=fn(...raw,RESULT);linked.arena.check();
      if(!Number.isInteger(status)||status<0||status>10)throw new Error("SEMAPRAX raw adapter status invariant");
      if(status!==0){
        for(let i=RESULT;i<RESULT+size;i++)if(bytes[i]!==POISON)throw new Error("SEMAPRAX failure modified result slot");
        const error=Object.assign(new Error(`SEMAPRAX semantic failure ${status}`),{status,semapraxSemantic:true});
        linked.arena.check();semanticError=error;hasSemantic=true;throw error;
      }
      const complete=stageOwnedResult(linked,fact,view);linked.arena.check();
      linked.arena.settle();settled=true;linked.arena.check();
      // In particular, the flat record's owned field and frozen object are
      // constructed only after its sole owner and whole arena settle.
      answer=complete();linked.arena.check();
    }catch(error){
      select(error);
      if(entered&&!(hasSemantic&&error===semanticError))linked.arena.poison();
    }finally{
      try{
        if(entered){
          if(began&&!settled)try{linked.arena.settle();settled=true}catch(error){linked.arena.poison();select(error)}
          try{if(bytes!==null){bytes.fill(0,0,used);bytes.fill(POISON,RESULT,RESULT+size)}}
          catch(error){linked.arena.poison();select(error)}
        }
      }finally{busy=false}
    }
    if(hasPrimary)throw primary;
    linked.arena.check();return answer;
  };
}
