// Project v8 only: admit the complete tuple before allocating payload copies.
const InputUint8Array=Uint8Array,InputDataView=DataView;
const inputArrayPrototype=Uint8Array.prototype,inputBufferPrototype=ArrayBuffer.prototype;
const TypedArrayPrototype=Object.getPrototypeOf(inputArrayPrototype);
const reflectApply=Reflect.apply,objectGetPrototypeOf=Object.getPrototypeOf,inputDescriptor=Object.getOwnPropertyDescriptor;
const typedSet=TypedArrayPrototype.set,typedTag=inputDescriptor(TypedArrayPrototype,Symbol.toStringTag).get,typedBuffer=inputDescriptor(TypedArrayPrototype,"buffer").get,typedOffset=inputDescriptor(TypedArrayPrototype,"byteOffset").get,typedLength=inputDescriptor(TypedArrayPrototype,"byteLength").get;
const inputBufferLength=inputDescriptor(inputBufferPrototype,"byteLength").get,inputResizable=inputDescriptor(inputBufferPrototype,"resizable")?.get,inputCharCode=String.prototype.charCodeAt;
const inputEncoder=new TextEncoder(),inputEncode=TextEncoder.prototype.encode;
function inputCapacity(){throw new RangeError("SEMAPRAX borrowed input capacity exceeded")}
function admitUint8(value,label,remaining){
  let buffer,offset,length;
  try{
    buffer=reflectApply(typedBuffer,value,[]);
    if(objectGetPrototypeOf(value)!==inputArrayPrototype||reflectApply(typedTag,value,[])!=="Uint8Array"||objectGetPrototypeOf(buffer)!==inputBufferPrototype)throw 0;
    // Inspect descriptors without invoking caller getters or species constructors.
    if(inputDescriptor(buffer,"constructor")!==undefined||inputDescriptor(value,"constructor")!==undefined)throw 0;
    reflectApply(inputBufferLength,buffer,[]);
    if(inputResizable!==undefined&&reflectApply(inputResizable,buffer,[]))throw 0;
    offset=reflectApply(typedOffset,value,[]);length=reflectApply(typedLength,value,[]);
    // This intrinsic view construction rejects even a detached zero-length view.
    // It allocates no backing bytes and does not consult Symbol.species.
    new InputDataView(buffer,offset,0);
  }catch{throw new TypeError(`${label} must be an ordinary attached fixed Uint8Array`)}
  if(length>remaining)inputCapacity();
  return {value,buffer,offset,length};
}
function admitString(value,label,remaining){
  if(typeof value!=="string")throw new TypeError(`${label} must be a string`);
  // Every UTF-16 unit contributes at least one UTF-8 byte. Bound scanning too.
  if(value.length>remaining)inputCapacity();
  let length=0;
  for(let index=0;index<value.length;index++){
    const unit=reflectApply(inputCharCode,value,[index]);
    if(unit>=0xd800&&unit<=0xdbff){
      const low=reflectApply(inputCharCode,value,[++index]);
      if(index>=value.length||low<0xdc00||low>0xdfff)throw new TypeError(`${label} must contain Unicode scalar values`);
      length+=4;
    }else if(unit>=0xdc00&&unit<=0xdfff){throw new TypeError(`${label} must contain Unicode scalar values`)}
    else length+=unit<0x80?1:unit<0x800?2:3;
    if(length>remaining)inputCapacity();
  }
  return {value,length};
}
function copyAdmittedUint8(plan,label){
  const observed=admitUint8(plan.value,label,plan.length);
  if(observed.buffer!==plan.buffer||observed.offset!==plan.offset||observed.length!==plan.length)throw new TypeError(`${label} changed during snapshot`);
  const copy=new InputUint8Array(plan.length);
  reflectApply(typedSet,copy,[plan.value,0]);
  return copy;
}
function snapshotUint8(value,label){
  // Module authentication has a separate fixed bound, not the call-input limit.
  return copyAdmittedUint8(admitUint8(value,label,16777216),label);
}
function snapshotArguments(values,types){
  const plans=[],snapshots=[];let used=0;
  for(let index=0;index<types.length;index++){
    const value=values[index],type=types[index],label=`argument ${index}`;let plan;
    if(type==="borrow-slice-u8")plan=admitUint8(value,label,65536-used);
    else if(type==="borrow-str")plan=admitString(value,label,65536-used);
    else if(type==="i64"){
      if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError(`${label} must be signed i64 bigint`);
      plan={value,length:0};
    }else if(type==="bool"){
      if(typeof value!=="boolean")throw new TypeError(`${label} must be boolean`);
      plan={value:value?1:0,length:0};
    }else throw new Error("unknown descriptor parameter type");
    used+=plan.length;plans[index]=plan;
  }
  for(let index=0;index<types.length;index++){
    const plan=plans[index],type=types[index];let copy;
    if(type==="borrow-slice-u8")copy=copyAdmittedUint8(plan,`argument ${index}`);
    else if(type==="borrow-str"){
      copy=reflectApply(inputEncode,inputEncoder,[plan.value]);
      if(reflectApply(typedLength,copy,[])!==plan.length)throw new Error("SEMAPRAX UTF-8 snapshot length invariant");
    }else copy=plan.value;
    snapshots[index]=copy;
  }
  return {snapshots,used};
}
