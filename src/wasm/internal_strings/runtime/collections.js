// Private typed collection owners; String atoms snapshot authenticated UTF-8.
// The scalar facade exposes neither handles nor imports. Every finalizer is
// compiler-selected, and settlement verifies the arena instead of clearing it.
function createCollectionOperations({authenticate,mint,checkedMemory,requireActive,refuseCapacity,fail,options}){
  const collections=new Map();let next=1n,liveBytes=0;
  const limit=(value,fallback,maximum)=>{
    if(value===undefined)return Math.min(fallback,maximum);
    if(!Number.isInteger(value)||value<1||value>maximum)fail();return value;
  };
  const maxOwners=limit(options.maxOwnedCollections,16,DESCRIPTOR.derived_owner_capacity);
  const maxBytes=limit(options.maxOwnedCollectionBytes,8388608,8388608);
  const atom=(tag,word)=>{
    if(typeof word!=="bigint")fail();
    if(tag===1)return authenticate(word).bytes;
    if(tag===2)return BigInt.asIntN(64,word);
    if(tag===3){if(word!==0n&&word!==1n)fail();return word}
    if(tag===4){const value=BigInt.asIntN(32,word);if(value!==word)fail();return value}
    if(tag===5&&(word<0n||word>255n))fail();
    if(tag===7&&(word<0n||word>1114111n||(word>=55296n&&word<=57343n)))fail();
    if(tag===8&&BigInt.asUintN(32,word)!==word)fail();
    if(tag<4||tag>9)fail();return BigInt.asUintN(64,word);
  };
  const compare=(tag,a,b)=>{
    if(tag!==1)return a<b?-1:a>b?1:0;
    const n=Math.min(a.length,b.length);
    for(let i=0;i<n;i++)if(a[i]!==b[i])return a[i]<b[i]?-1:1;
    return a.length<b.length?-1:a.length>b.length?1:0;
  };
  const find=(map,key)=>{
    let low=0,high=map.entries.length;
    while(low<high){const mid=low+Math.floor((high-low)/2),order=compare(map.keyTag,map.entries[mid][0],key);
      if(order===0)return {index:mid,found:true};if(order<0)low=mid+1;else high=mid;
    }
    return {index:low,found:false};
  };
  const size=entry=>(entry[0] instanceof Bytes?entry[0].length:0)+(entry[1] instanceof Bytes?entry[1].length:0);
  const snapshot=value=>{const out=new Bytes(value.length);apply(byteSet,out,[value,0]);return out};
  const output=offset=>{
    if(!Number.isInteger(offset)||offset<0||offset%8!==0||offset>65536-8)fail();
    return new View(checkedMemory(),offset,8);
  };
  const write=(out,tag,value)=>out.setBigInt64(0,tag===1?mint(value.length,copy=>apply(byteSet,copy,[value,0])):BigInt.asIntN(64,value),true);
  const operations={
    spx_collection_checked_v2(op,kind,token,keyWord,valueWord,offset){
      requireActive();
      if(!Number.isInteger(op)||op<1||op>9||!Number.isInteger(kind)||(kind&~0x1ffff)!==0)fail();
      const keyTag=kind&255,valueTag=(kind>>>8)&255,legacy=(kind&65536)!==0;
      if(keyTag<1||keyTag>3||valueTag<1||valueTag>9||(legacy&&(keyTag!==1||valueTag!==2)))fail();
      const out=output(offset),failure=code=>(legacy?25:29)+code;
      if(op===1){
        if(typeof keyWord!=="bigint")fail();
        if(keyWord<0n||keyWord>65536n)return failure(3);
        if(token!==0n||valueWord!==0n)fail();
        if(collections.size>=maxOwners)return refuseCapacity("collection_owners");
        if(next>0xffffffffn)return refuseCapacity("collection_tokens");
        const handle=0x6000000000000000n|next++;
        collections.set(handle,{keyTag,valueTag,legacy,capacity:Number(keyWord),entries:[]});out.setBigInt64(0,handle,true);return 0;
      }
      const map=collections.get(token);
      if(map===undefined||map.keyTag!==keyTag||map.valueTag!==valueTag||map.legacy!==legacy)fail();
      if(op===7){out.setBigInt64(0,BigInt(map.entries.length),true);return 0}
      if(op===8||op===9){
        if(typeof keyWord!=="bigint")fail();
        if(keyWord<0n||keyWord>=BigInt(map.entries.length))return failure(2);
        write(out,op===8?keyTag:valueTag,map.entries[Number(keyWord)][op===8?0:1]);return 0;
      }
      const key=atom(keyTag,keyWord),position=find(map,key);
      if(op===6){out.setBigInt64(0,position.found?1n:0n,true);return 0}
      if(op===5){write(out,valueTag,position.found?map.entries[position.index][1]:atom(valueTag,valueWord));return 0}
      if(op===4){if(position.found)liveBytes-=size(map.entries.splice(position.index,1)[0]);out.setBigInt64(0,token,true);return 0}
      let value=atom(valueTag,valueWord);
      if(op===3){
        if(valueTag!==2)fail();
        if(position.found){value+=map.entries[position.index][1];if(value<-(1n<<63n)||value>(1n<<63n)-1n)return failure(4)}
      }
      if(!position.found&&map.entries.length===map.capacity)return failure(1);
      const old=position.found?map.entries[position.index]:null;
      const bytes=(position.found?size(old)-(old[1] instanceof Bytes?old[1].length:0):(keyTag===1?key.length:0))+(valueTag===1?value.length:0);
      const delta=bytes-(old===null?0:size(old));
      // Authenticate and reserve before any payload copies or publication.
      if(liveBytes+delta>maxBytes)return refuseCapacity("collection_bytes");
      const entry=[position.found?old[0]:keyTag===1?snapshot(key):key,valueTag===1?snapshot(value):value];
      if(position.found)map.entries[position.index]=entry;else map.entries.splice(position.index,0,entry);
      liveBytes+=delta;out.setBigInt64(0,token,true);return 0;
    },
    spx_collection_drop_v2(token){
      requireActive();const map=collections.get(token);if(map===undefined)fail();
      for(const entry of map.entries)liveBytes-=size(entry);
      if(liveBytes<0||!collections.delete(token))fail();
    }
  };
  return {operations,begin(){if(collections.size!==0||liveBytes!==0)fail()},settle(){if(collections.size!==0||liveBytes!==0)fail()}};
}
