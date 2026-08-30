import assert from 'node:assert/strict';

// Actual package result decoding: no replacement arena, decoder or runtime
// source splice. The engine wrapper first runs the authentic selected export.
export async function exerciseResults({fixture,config,raw,input,RESULT,capture}){
  const realGet=DataView.prototype.getBigInt64;
  const realMapGet=Map.prototype.get,realDelete=Map.prototype.delete;
  const apply=Reflect.apply;
  const payloadOffset=config.family==='variant'?8:0;
  const messages={
    token:'SEMAPRAX owned carrier token invariant',
    stale:'SEMAPRAX stale or wrong-length owned carrier',
    length:'SEMAPRAX owned carrier length invariant',
    tag:'SEMAPRAX owned variant tag invariant',
    unsettled:'SEMAPRAX arena did not settle',
    bool:'SEMAPRAX bool result invariant',
    failure:'SEMAPRAX failure modified result slot',
    poisoned:'SEMAPRAX owned-data runtime is poisoned',
  };
  const token=carrier=>Number((BigInt.asUintN(64,carrier)>>32n)&0x7fffffffn);
  const word=(root,length)=>BigInt.asIntN(64,(BigInt(root)<<32n)|BigInt(length));
  const lengthWord=(carrier,length)=>BigInt.asIntN(64,(BigInt.asUintN(64,carrier)&0xffffffff00000000n)|BigInt(length));
  const defaultInvoke=api=>api.call('case.copy',input,1n);
  function bytes(value){
    if(config.family==='variant'){assert.equal(value.ok,true);return value.value}
    if(config.family==='record')return value[config.fields[0]];
    return value;
  }
  function errorIs(outcome,message){
    assert.equal(outcome.threw,true,`published a result instead of ${message}`);
    assert(outcome.value instanceof Error);
    assert.equal(outcome.value.message,message);
  }
  function stillPoisoned(subject,expectedEntries=1){
    const before=subject.counters();
    assert.equal(before.entries,expectedEntries,'faulted call engine entries');
    assert.equal(before.resultMutations,1,'exactly one result fault was injected');
    errorIs(capture(()=>defaultInvoke(subject.api)),messages.poisoned);
    assert.deepEqual(subject.counters(),before,'poisoned call reached engine/imports');
  }

  async function subjectFor({id='case.copy',offset=payloadOffset,active=true,status=0,mutate=()=>false}={}){
    const state={enabled:true,armed:false,memory:null,offset:RESULT+offset,token:null,reads:0,gets:0,deletes:0,hooks:0,carrier:null};
    const subject=await fixture({resultHook:({name,status:actual,memory,env})=>{
      if(!state.enabled)return false;
      assert.equal(name,raw(id));assert.equal(actual,status);
      state.hooks++;
      const view=new DataView(memory.buffer);
      // Read only known active results; inactive Option storage is never used
      // as ownership authority, even by the observer's setup.
      const carrier=active?apply(realGet,view,[state.offset,true]):null;
      state.carrier=carrier;state.token=carrier===null?null:token(carrier);
      const changed=mutate({view,memory,env,carrier,state});
      state.memory=memory;state.armed=true;
      return changed;
    }});
    return{subject,state};
  }
  function observed(subject,state,invoke=defaultInvoke){
    state.armed=false;state.reads=0;state.gets=0;state.deletes=0;
    // These observers return the original intrinsic result and never throw to
    // manufacture rejection. Scope begins after the real raw call/mutation,
    // ends after facade settlement, and is restored even on assertion failure.
    DataView.prototype.getBigInt64=function(...args){
      const value=apply(realGet,this,args);
      if(state.armed&&this.buffer===state.memory.buffer&&args[0]===state.offset)state.reads++;
      return value;
    };
    Map.prototype.get=function(key){
      const value=apply(realMapGet,this,[key]);
      if(state.armed&&key===state.token)state.gets++;
      return value;
    };
    Map.prototype.delete=function(key){
      const entry=apply(realMapGet,this,[key]);
      const deleted=apply(realDelete,this,[key]);
      if(state.armed&&key===state.token&&entry instanceof Uint8Array&&deleted)state.deletes++;
      return deleted;
    };
    try{return capture(()=>invoke(subject.api))}
    finally{
      state.armed=false;DataView.prototype.getBigInt64=realGet;
      Map.prototype.get=realMapGet;Map.prototype.delete=realDelete;
    }
  }
  function counts(state,reads,gets,deletes){
    assert.equal(state.reads,reads,'production payload reads');
    assert.equal(state.gets,gets,'production carrier lookups');
    assert.equal(state.deletes,deletes,'successful production owner consumption');
  }

  // Independently calibrate the observers on both nonempty and empty owners.
  // A zero-length result is valid when authenticated, not a forbidden length.
  for(const value of [input,new Uint8Array()]){
    const {subject,state}=await subjectFor();
    const outcome=observed(subject,state,api=>api.call('case.copy',value,1n));
    assert.equal(outcome.threw,false);assert.deepEqual(bytes(outcome.value),value);
    assert.notEqual(bytes(outcome.value).buffer,value.buffer);
    assert.equal(state.hooks,1);counts(state,1,1,1);
    assert.deepEqual(subject.counters(),{entries:1,mints:1,drops:0,importsAfterReentry:0,blockedImport:false,validatorReturned:0,resultMutations:0});
  }
  if(config.utf8){
    const {subject,state}=await subjectFor({id:'case.text',offset:0});
    const outcome=observed(subject,state,api=>api.call('case.text'));
    assert.equal(outcome.threw,false);assert.equal(outcome.value,'\ufeffA\0λ');
    counts(state,1,1,1);assert.equal(state.hooks,1);
    assert.equal(subject.counters().mints,1);assert.equal(subject.counters().drops,0);
  }

  const faults=[
    ['untagged-zero',()=>0n,'token',0],
    ['tagged-zero-token',()=>word(0x80000000,4),'token',0],
    ['unissued-token',()=>word(0xffffffff,4),'stale',1],
    ['wrong-zero-length',carrier=>lengthWord(carrier,0),'stale',1],
    ['wrong-plus-one-length',carrier=>lengthWord(carrier,Number(BigInt.asUintN(64,carrier)&0xffffffffn)+1),'stale',1],
    ['over-capacity-length',carrier=>lengthWord(carrier,65537),'length',0],
  ];
  for(const id of config.utf8?['case.copy','case.text']:['case.copy'])for(const [name,forge,message,gets] of faults){
    const {subject,state}=await subjectFor({id,offset:id==='case.text'?0:payloadOffset,mutate:({view,carrier,state})=>{
      const forged=forge(carrier);view.setBigInt64(state.offset,forged,true);
      state.token=token(forged);return true;
    }});
    errorIs(observed(subject,state,id==='case.text'?api=>api.call(id):defaultInvoke),messages[message]);
    assert.equal(state.hooks,1,name);counts(state,1,gets,0);
    assert.equal(subject.counters().entries,1);assert.equal(subject.counters().mints,1);
    assert.equal(subject.counters().drops,0);assert.equal(subject.counters().resultMutations,1);
    stillPoisoned(subject);
  }

  // Stale means genuinely consumed in this same arena, not a guessed integer.
  {
    let stale=null;
    const {subject,state}=await subjectFor({mutate:({view,carrier,state})=>{
      if(stale===null){stale=carrier;return false}
      assert.notEqual(carrier,stale,'successful call reused a consumed token');
      view.setBigInt64(state.offset,stale,true);state.token=token(stale);return true;
    }});
    const first=observed(subject,state);assert.equal(first.threw,false);
    assert.deepEqual(bytes(first.value),input);counts(state,1,1,1);
    errorIs(observed(subject,state),messages.stale);counts(state,1,1,0);
    assert.equal(state.hooks,2);assert.equal(subject.counters().entries,2);
    assert.equal(subject.counters().mints,2);assert.equal(subject.counters().resultMutations,1);
    stillPoisoned(subject,2);
  }
  {
    const {subject,state}=await subjectFor({mutate:({env,carrier})=>{env.spx_bytes_drop(carrier);return true}});
    errorIs(observed(subject,state),messages.stale);counts(state,1,1,0);
    assert.equal(subject.counters().drops,1);assert.equal(subject.counters().resultMutations,1);
    stillPoisoned(subject);
  }
  {
    const {subject,state}=await subjectFor({mutate:({env})=>{env.spx_bytes_copy(4n);return true}});
    errorIs(observed(subject,state),messages.unsettled);counts(state,1,1,1);
    assert.equal(subject.counters().mints,2);assert.equal(subject.counters().drops,0);
    assert.equal(subject.counters().resultMutations,1);stillPoisoned(subject);
  }

  if(config.family==='variant'){
    for(const id of ['case.copy','case.none'])for(const tag of [2,0xffffffff]){
      const {subject,state}=await subjectFor({id,offset:8,active:id==='case.copy',mutate:({view})=>{view.setUint32(RESULT,tag,true);return true}});
      errorIs(observed(subject,state,id==='case.none'?api=>api.call(id):defaultInvoke),messages.tag);
      counts(state,0,0,0);assert.equal(state.hooks,1);assert.equal(subject.counters().resultMutations,1);
      stillPoisoned(subject);
    }
    {
      const {subject,state}=await subjectFor({id:'case.none',offset:8,active:false,mutate:({view})=>{view.setBigInt64(RESULT+8,-1n,true);return true}});
      for(let round=0;round<2;round++){
        const outcome=observed(subject,state,api=>api.call('case.none'));
        assert.equal(outcome.threw,false);assert.equal(outcome.value,null);counts(state,0,0,0);
      }
      assert.equal(subject.counters().entries,2);assert.equal(subject.counters().mints,0);
      assert.equal(subject.counters().drops,0);assert.equal(subject.counters().resultMutations,2);
      state.enabled=false;
      assert.deepEqual(bytes(defaultInvoke(subject.api)),input);
      assert.equal(subject.counters().entries,3);assert.equal(subject.counters().mints,1);
    }
    {
      const {subject,state}=await subjectFor({mutate:({view})=>{view.setUint32(RESULT,1,true);return true}});
      errorIs(observed(subject,state),messages.unsettled);counts(state,1,0,0);
      assert.equal(subject.counters().mints,1);assert.equal(subject.counters().drops,0);
      assert.equal(subject.counters().resultMutations,1);stillPoisoned(subject);
    }
  }

  if(config.family==='record')for(const value of [2n,1n<<32n]){
    const {subject,state}=await subjectFor({mutate:({view})=>{view.setBigUint64(RESULT+16,value,true);return true}});
    errorIs(observed(subject,state),messages.bool);counts(state,1,0,0);
    assert.equal(subject.counters().mints,1);assert.equal(subject.counters().resultMutations,1);
    stillPoisoned(subject);
  }
  if(config.family==='mixed'){
    const {subject,state}=await subjectFor({id:'case.flag',offset:0,active:false,mutate:({view})=>{view.setUint32(RESULT,2,true);return true}});
    errorIs(observed(subject,state,api=>api.call('case.flag',true)),messages.bool);counts(state,0,0,0);
    assert.equal(subject.counters().mints,0);assert.equal(subject.counters().resultMutations,1);
    stillPoisoned(subject);
  }

  for(const id of ['case.before','case.copy']){
    const {subject,state}=await subjectFor({id,active:false,status:4,mutate:({memory})=>{new Uint8Array(memory.buffer)[RESULT]^=1;return true}});
    errorIs(observed(subject,state,api=>api.call(id,input,0n)),messages.failure);counts(state,0,0,0);
    assert.equal(subject.counters().entries,1);assert.equal(subject.counters().resultMutations,1);
    assert.equal(subject.counters().mints,id==='case.copy'?1:0);
    assert.equal(subject.counters().drops,id==='case.copy'?1:0);stillPoisoned(subject);
  }
}
