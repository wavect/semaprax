use super::*;

#[test]
fn raw_two_owned_result_rejects_invalid_tag_before_payload_authority() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.wasm_two_owned_result_hostile;
@id("wasm.result-hostile.consume")
fn consume(value: own Result<Bytes, Bytes>) -> i64 {
  match own value {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { 2 } else { 0 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 3 } else { 0 },
  }
}
@id("app.main") fn main() -> i64 { 42 }
"#;
    let resolved =
        hir::resolve(&parse(source, Path::new("wasm-two-owned-result-hostile.spx")).unwrap())
            .unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-two-owned-result-hostile-wasm-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let consume = format!(
        "__spx_test_{}",
        hex_identity(&DeclarationId::new("wasm.result-hostile.consume"))
    );
    let script = format!(
        r#"import {{readFile}} from "node:fs/promises";
const bytes=await readFile(process.argv[2]);
let drops=0;
const fail=name=>()=>{{throw new Error(`unexpected host import ${{name}}`)}};
const {{instance}}=await WebAssembly.instantiate(bytes,{{env:{{
  spx_add:fail("spx_add"),spx_sub:fail("spx_sub"),spx_mul:fail("spx_mul"),
  spx_div:fail("spx_div"),spx_rem:fail("spx_rem"),spx_neg:fail("spx_neg"),
  spx_contract_fail:fail("spx_contract_fail"),spx_bytes_copy:fail("spx_bytes_copy"),
  spx_bytes_get:fail("spx_bytes_get"),spx_bytes_as_slice:value=>value,
  spx_bytes_drop:()=>{{drops+=1;}},
}}}});
const memory=instance.exports.__spx_test_memory;
const stack=instance.exports.__spx_test_shadow_stack;
const view=new DataView(memory.buffer),input=1024,output=2048,top=stack.value;
const poison=()=>new Uint8Array(memory.buffer,output,8).fill(0xa5);
const unchanged=()=>{{for(const byte of new Uint8Array(memory.buffer,output,8))if(byte!==0xa5)throw Error("hostile Result tag published output")}};
for(const [tag,carrier,expected] of [[0,2n,2n],[1,3n,3n]]){{
  view.setUint32(input,tag,true);view.setBigUint64(input+8,carrier,true);poison();
  if(instance.exports["{consume}"](input,output)!==0||view.getBigUint64(output,true)!==expected)throw Error("valid selected Result case");
  if(stack.value!==top)throw Error("valid Result stack restore");
}}
if(drops!==2)throw Error("valid Result cases did not settle exactly one selected owner");
view.setUint32(input,0xffffffff,true);view.setBigUint64(input+8,99n,true);poison();
let trapped=false;
try{{instance.exports["{consume}"](input,output);}}catch{{trapped=true;}}
if(!trapped)throw Error("invalid Result tag did not trap");
unchanged();
if(drops!==2)throw Error("invalid Result tag granted payload cleanup authority");
"#
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
