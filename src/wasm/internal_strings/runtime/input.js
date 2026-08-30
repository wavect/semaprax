// This module assumes trusted realm intrinsics, not a hostile co-resident realm.
const Bytes=Uint8Array,View=DataView,apply=Reflect.apply;
const bytePrototype=Bytes.prototype,bufferPrototype=ArrayBuffer.prototype;
const typedPrototype=Object.getPrototypeOf(bytePrototype),property=Object.getOwnPropertyDescriptor;
const getBuffer=property(typedPrototype,"buffer").get;
const getOffset=property(typedPrototype,"byteOffset").get;
const getLength=property(typedPrototype,"byteLength").get;
const getTag=property(typedPrototype,Symbol.toStringTag).get;
const getBufferLength=property(bufferPrototype,"byteLength").get;
const getResizable=property(bufferPrototype,"resizable")?.get;
const byteSet=typedPrototype.set;
function snapshotModule(input){
  let buffer,offset,length;
  try{
    buffer=apply(getBuffer,input,[]);
    if(Object.getPrototypeOf(input)!==bytePrototype||apply(getTag,input,[])!=="Uint8Array"||Object.getPrototypeOf(buffer)!==bufferPrototype)throw 0;
    if(property(input,"constructor")!==undefined||property(buffer,"constructor")!==undefined)throw 0;
    apply(getBufferLength,buffer,[]);
    if(getResizable!==undefined&&apply(getResizable,buffer,[]))throw 0;
    offset=apply(getOffset,input,[]);length=apply(getLength,input,[]);
    new View(buffer,offset,0); // Reject detached views, including empty ones.
  }catch{throw new TypeError("SEMAPRAX module must be an ordinary attached fixed Uint8Array")}
  if(length>16777216||length!==EXPECTED_BYTES)throw new RangeError("SEMAPRAX module length disagrees");
  const bytes=new Bytes(length);
  apply(byteSet,bytes,[input,0]);
  return bytes;
}
const IMPORT_NAMES=Object.freeze(["literal","clone","concat","from_char","byte_len","char_len","eq","starts_with","contains","drop"]);
function validateModule(module){
  const imports=WebAssembly.Module.imports(module);
  if(imports.length!==IMPORT_NAMES.length)throw new Error("SEMAPRAX import inventory disagrees");
  for(let i=0;i<imports.length;i++){
    const item=imports[i];
    if(item.module!=="semaprax.internal-strings.v1"||item.name!==IMPORT_NAMES[i]||item.kind!=="function")throw new Error("SEMAPRAX import inventory disagrees");
  }
  const expected=new Map([["memory","memory"],["__spx_stack_pointer","global"]]);
  for(const fact of DESCRIPTOR.exports)expected.set(fact.wasm_export,"function");
  const exports=WebAssembly.Module.exports(module);
  if(exports.length!==expected.size)throw new Error("SEMAPRAX export inventory disagrees");
  for(const item of exports){
    if(expected.get(item.name)!==item.kind)throw new Error("SEMAPRAX export inventory disagrees");
    expected.delete(item.name);
  }
}
