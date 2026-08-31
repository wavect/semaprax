import assert from 'node:assert/strict';

// Trusted-realm fault observations of the unchanged generated facade, not a
// replacement arena or a claim about physical allocation/deallocation failure.
export async function exerciseFinalization({fixture,config,raw,input,RESULT,capture}){
  const payloadOffset=config.family==='variant'?8:0;
  const resultSize=config.family==='record'?32:config.family==='variant'?16:8;
  const frozen=config.family==='record'||config.family==='variant';

  async function run(kind,primary,secondary){
    const state={armed:false,token:0,map:null,owner:null,copied:null,gets:0,
      copyAttempts:0,copies:0,deleteAttempts:0,deletes:0,settlements:[],
      inputFills:0,resultFills:0,freezes:0,injections:0,secondaryInjections:0};
    let observing=true,observerFailed=false,observerFailure;
    function observe(action){
      try{return action()}catch(error){
        if(!observerFailed){observerFailed=true;observerFailure=error}
        throw error;
      }
    }
    function fault(at){
      if(kind===at){state.injections++;throw primary}
      if(secondary!==undefined&&at==='input'){
        state.secondaryInjections++;throw secondary;
      }
    }
    const subject=await fixture({resultHook:({name,status,memory})=>{
      if(!observing)return false;
      return observe(()=>{
      assert.equal(name,raw('case.copy'));assert.equal(status,0);
      const carrier=new DataView(memory.buffer).getBigInt64(RESULT+payloadOffset,true);
      const word=BigInt.asUintN(64,carrier);
      assert.equal(Number(word&0xffffffffn),input.length);
      assert.notEqual(word&(1n<<63n),0n);
      state.token=Number((word>>32n)&0x7fffffffn);
      assert.notEqual(state.token,0);assert.equal(state.armed,false);
      state.armed=true;
      return false; // Observe the authentic result; never mutate its storage.
      });
    }});
    const RealArray=Uint8Array,realGet=Map.prototype.get,realDelete=Map.prototype.delete;
    const sizeDescriptor=Object.getOwnPropertyDescriptor(Map.prototype,'size');
    const realFill=Uint8Array.prototype.fill,realFreeze=Object.freeze;
    let outcome;
    try{
      Map.prototype.get=function(key){
        const value=Reflect.apply(realGet,this,[key]);
        if(state.armed&&key===state.token)observe(()=>{
          assert(value instanceof RealArray);assert.equal(value.length,input.length);
          for(let i=0;i<input.length;i++)assert.equal(value[i],input[i]);
          if(state.map===null){state.map=this;state.owner=value}
          assert.equal(this,state.map);assert.equal(value,state.owner);state.gets++;
        });
        return value;
      };
      globalThis.Uint8Array=new Proxy(RealArray,{construct(target,args,newTarget){
        if(!state.armed||args[0]!==state.owner)return Reflect.construct(target,args,newTarget);
        observe(()=>{
          assert.equal(args.length,1);assert.equal(state.gets,1);
          assert.equal(state.deleteAttempts,0);state.copyAttempts++;
        });
        fault('copy');
        const copy=Reflect.construct(target,args,newTarget);
        observe(()=>{
          assert.notEqual(copy.buffer,state.owner.buffer);
          assert.equal(Object.getPrototypeOf(copy),RealArray.prototype);
          assert.deepEqual(copy,input);state.copies++;state.copied=copy;
        });
        return copy;
      }});
      Map.prototype.delete=function(key){
        if(this!==state.map||key!==state.token)return Reflect.apply(realDelete,this,[key]);
        observe(()=>{
          assert.equal(state.copies,1);assert.equal(state.deletes,0);state.deleteAttempts++;
        });
        fault('delete-before');
        const deleted=Reflect.apply(realDelete,this,[key]);
        observe(()=>{assert.equal(deleted,true);state.deletes++});
        fault('delete-after');
        return deleted;
      };
      Object.defineProperty(Map.prototype,'size',{...sizeDescriptor,get(){
        const size=Reflect.apply(sizeDescriptor.get,this,[]);
        if(state.armed&&this===state.map)state.settlements.push(size);
        return size;
      }});
      Uint8Array.prototype.fill=function(...args){
        if(!state.armed||this.buffer!==subject.memory.buffer)return Reflect.apply(realFill,this,args);
        const at=observe(()=>{
          assert.equal(this.byteOffset,0);assert.equal(args.length,3);
          if(args[0]===0){
            assert.deepEqual(args,[0,0,input.length]);state.inputFills++;return 'input';
          }
          assert.deepEqual(args,[0xa5,RESULT,RESULT+resultSize]);state.resultFills++;return 'result';
        });
        fault(at);
        return Reflect.apply(realFill,this,args);
      };
      Object.freeze=function(value){
        const matches=state.armed&&state.copied!==null&&value!==null&&typeof value==='object'&&(
          config.family==='record'
            ? Object.getPrototypeOf(value)===null&&value[config.fields[0]]===state.copied
            : config.family==='variant'&&value.ok===true&&value.value===state.copied
        );
        if(!matches)return realFreeze(value);
        observe(()=>{
          assert.equal(state.deletes,1);assert.deepEqual(state.settlements,[0]);
          assert.equal(state.inputFills,0);state.freezes++;
        });
        fault('freeze');
        return realFreeze(value);
      };
      outcome=capture(()=>subject.api.call('case.copy',input,1n));
    }finally{
      // Preserve the first invocation's ledger. Subsequent healthy/poisoned
      // calls use restored intrinsics and cannot rearm this observation hook.
      observing=false;
      globalThis.Uint8Array=RealArray;
      Map.prototype.get=realGet;Map.prototype.delete=realDelete;
      Object.defineProperty(Map.prototype,'size',sizeDescriptor);
      RealArray.prototype.fill=realFill;Object.freeze=realFreeze;
    }
    // An observer assertion must never be mistaken for an intended host fault.
    if(observerFailed)throw observerFailure;
    const counters=subject.counters();
    assert.equal(counters.entries,1);assert.equal(counters.mints,1);
    assert.equal(counters.drops,0);assert.equal(counters.resultMutations,0);
    assert.equal(state.gets,1);assert.equal(state.copyAttempts,1);
    if(kind===null){
      assert.equal(outcome.threw,false);assert.equal(state.injections,0);
      const value=config.family==='record'?outcome.value[config.fields[0]]:
        config.family==='variant'?outcome.value.value:outcome.value;
      assert.equal(value,state.copied);assert.deepEqual(value,input);
      assert.equal(state.copies,1);assert.equal(state.deleteAttempts,1);assert.equal(state.deletes,1);
      assert.deepEqual(state.settlements,[0]);assert.equal(state.inputFills,1);assert.equal(state.resultFills,1);
      assert.equal(state.freezes,frozen?1:0);
      // Calibration includes successful reuse, after all intrinsics are restored.
      assert.equal(capture(()=>subject.api.call('case.copy',input,1n)).threw,false);
      return;
    }
    assert.equal(outcome.threw,true,'finalization fault published a value');
    assert.strictEqual(outcome.value,primary,'selected thrown value changed');
    assert.equal(state.injections,1);assert.equal(state.secondaryInjections,secondary===undefined?0:1);
    assert.equal(state.copies,kind==='copy'?0:1);
    assert.equal(state.deleteAttempts,kind==='copy'?0:1);
    assert.equal(state.deletes,['copy','delete-before'].includes(kind)?0:1);
    const completed=['input','result','freeze'].includes(kind);
    // A throw after real deletion poisons before settle: empty Map != settlement.
    assert.deepEqual(state.settlements,completed?[0]:[]);
    assert.equal(state.freezes,completed&&frozen?1:0);
    assert.equal(state.inputFills,1);
    assert.equal(state.resultFills,kind==='input'||secondary!==undefined?0:1);
    const later=capture(()=>subject.api.call('case.copy',input,1n));
    assert.equal(later.threw,true);assert.equal(later.value.message,'SEMAPRAX owned-data runtime is poisoned');
    assert.deepEqual(subject.counters(),counters,'poisoned reuse reached engine or owned imports');
  }

  await run(null);
  const kinds=['copy','delete-before','delete-after','input','result'];
  if(frozen)kinds.push('freeze');
  for(const kind of kinds){
    for(const value of [new RangeError(`finalization ${kind}`),null,undefined,false,0,'']){
      await run(kind,value);
    }
  }
  // Unlike the old pre-mint primary/cleanup case, an actual owned result exists
  // before this copy failure and the distinct later scratch-cleanup exception.
  await run('copy',Object.freeze({primary:'owned copy'}),new Error('secondary input cleanup'));
}
