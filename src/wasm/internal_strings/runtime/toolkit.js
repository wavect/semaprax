// This additive profile keeps String tokens in the authenticated owner arena.
// Borrowed str values are compiler-proved views of those same tokens.
function createToolkitOperations({authenticate,mint,checkedMemory,fail,options}){
  function output(offset,size=8){
    if(!Number.isInteger(offset)||offset<0||offset%8!==0||offset>65536-size)fail();
    return new View(checkedMemory(),offset,size);
  }
  const bytes=carrier=>authenticate(carrier).bytes;
  const range=(index,length)=>typeof index==="bigint"&&index>=0n&&index<=BigInt(length);
  const boundary=(value,index)=>index===value.length||(value[index]&192)!==128;
  const whitespace=value=>value===32||(value>=9&&value<=13);
  function copy(value){return mint(value.length,out=>apply(byteSet,out,[value,0]))}
  function integer(value,unsigned){
    if(typeof value!=="bigint"||BigInt.asIntN(64,value)!==value)fail();
    const text=(unsigned?BigInt.asUintN(64,value):value).toString();
    return mint(text.length,out=>{for(let i=0;i<text.length;i++)out[i]=text.charCodeAt(i)});
  }
  function utf8(value){
    for(let i=0;i<value.length;){
      const first=value[i++];let extra,min,scalar;
      if(first<128)continue;
      if(first>=194&&first<=223){extra=1;min=128;scalar=first&31}
      else if(first>=224&&first<=239){extra=2;min=2048;scalar=first&15}
      else if(first>=240&&first<=244){extra=3;min=65536;scalar=first&7}
      else return false;
      if(i+extra>value.length)return false;
      for(let j=0;j<extra;j++){const byte=value[i++];if((byte&192)!==128)return false;scalar=(scalar<<6)|(byte&63)}
      if(scalar<min||scalar>1114111||(scalar>=55296&&scalar<=57343))return false;
    }
    return true;
  }
  let fileOperations=0,fileReservedBytes=0;
  const operations={
    from_i64:value=>integer(value,false),
    from_usize:value=>integer(value,true),
    compare(left,right){
      const a=bytes(left),b=bytes(right),bound=Math.min(a.length,b.length);
      for(let i=0;i<bound;i++)if(a[i]!==b[i])return a[i]<b[i]?-1n:1n;
      return a.length===b.length?0n:a.length<b.length?-1n:1n;
    },
    spx_string_slice_v2(carrier,start,end,offset){
      const value=bytes(carrier),out=output(offset);
      if(!range(start,value.length)||!range(end,value.length)||start>end)return 23;
      if(!boundary(value,Number(start))||!boundary(value,Number(end)))return 24;
      out.setBigInt64(0,copy(value.subarray(Number(start),Number(end))),true);return 0;
    },
    spx_string_find_v2(carrier,needleCarrier,from,offset){
      const value=bytes(carrier),needle=bytes(needleCarrier),out=output(offset);
      if(!range(from,value.length))return 23;
      let found=-1n;
      outer:for(let i=Number(from);i<=value.length-needle.length;i++){
        for(let j=0;j<needle.length;j++)if(value[i+j]!==needle[j])continue outer;
        found=BigInt(i);break;
      }
      out.setBigInt64(0,found,true);return 0;
    },
    spx_string_to_i64_v2(carrier,offset){
      const value=bytes(carrier),out=output(offset,16);
      let i=0,negative=false,valid=true,result=0n;
      if(value[0]===45){negative=true;i++}
      if(i===value.length)valid=false;
      for(;valid&&i<value.length;i++){
        const digit=value[i];if(digit<48||digit>57){valid=false;break}
        result=result*10n+BigInt(digit-48);
        if(result>(negative?9223372036854775808n:9223372036854775807n))valid=false;
      }
      out.setBigInt64(0,valid?1n:0n,true);
      out.setBigInt64(8,valid?(negative?-result:result):0n,true);return 0;
    },
    spx_string_trim_v2(carrier,offset){
      const value=bytes(carrier),out=output(offset);let start=0,end=value.length;
      while(start<end&&whitespace(value[start]))start++;
      while(end>start&&whitespace(value[end-1]))end--;
      out.setBigInt64(0,copy(value.subarray(start,end)),true);return 0;
    },
    spx_string_byte_at_v2(carrier,index,offset){
      const value=bytes(carrier),out=output(offset);
      if(!range(index,value.length)||index===BigInt(value.length))return 23;
      out.setBigInt64(0,BigInt(value[Number(index)]),true);return 0;
    },
    spx_string_from_str_v2(carrier,offset){
      const value=bytes(carrier),out=output(offset);
      out.setBigInt64(0,copy(value),true);return 0;
    },
    spx_file_read_text_v2(carrier,offset){
      const path=bytes(carrier),out=output(offset),provider=options.fileReadText;
      if(provider===undefined)return 70;
      if(provider===null||typeof provider.read!=="function")fail();
      if(fileOperations>=64||fileReservedBytes>1048576-65536)return 68;
      fileOperations++;fileReservedBytes+=65536;
      if(path.length===0||path.length>4096||path.includes(0)||path.includes(92)||path.includes(58))return 65;
      let start=0;
      for(let i=0;i<=path.length;i++)if(i===path.length||path[i]===47){
        const length=i-start;
        if(length===0||(length===1&&path[start]===46)||(length===2&&path[start]===46&&path[start+1]===46))return 65;
        start=i+1;
      }
      const result=provider.read(path.slice(),65536);
      if(result===null||typeof result!=="object"||typeof result.ok!=="boolean")fail();
      if(!result.ok){if(!Number.isInteger(result.code)||result.code<1||result.code>7)fail();return 64+result.code}
      const supplied=result.bytes;
      let backing,length;
      try{
        if(Object.getPrototypeOf(supplied)!==bytePrototype||apply(getTag,supplied,[])!=="Uint8Array")fail();
        backing=apply(getBuffer,supplied,[]);length=apply(getLength,supplied,[]);
        if(Object.getPrototypeOf(backing)!==bufferPrototype||(getResizable!==undefined&&apply(getResizable,backing,[])))fail();
        new View(backing,apply(getOffset,supplied,[]),0);
      }catch{fail()}
      if(length>65536)return 68;
      const snapshot=new Bytes(length);apply(byteSet,snapshot,[supplied,0]);
      if(!utf8(snapshot))return 25;
      out.setBigInt64(0,copy(snapshot),true);return 0;
    }
  };
  return {operations,begin(){fileOperations=0;fileReservedBytes=0}};
}
