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
@id("wasm.result-hostile.propagate")
fn propagate(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> {
  let payload = value?;
  Result<Bytes, Bytes>::Ok { value: payload }
}
@id("app.main") fn main() -> i64 { 42 }
"#;
    let resolved =
        hir::resolve(&parse(source, Path::new("wasm-two-owned-result-hostile.spx")).unwrap())
            .unwrap();
    let mut forged = resolved.clone();
    let propagate = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "wasm.result-hostile.propagate")
        .unwrap();
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut propagate.body.kind else {
        panic!("owned Result propagation must remain a block")
    };
    let crate::hir::ResolvedStatement::Let {
        value: try_expr, ..
    } = &mut statements[0]
    else {
        panic!("owned Result propagation must begin with its Try binding")
    };
    assert!(matches!(
        try_expr.kind,
        crate::hir::ResolvedExprKind::Try { .. }
    ));
    try_expr.ownership = crate::hir::OwnershipMode::Borrow;
    let forged_result = emit_profile(&forged, true, false);
    if let Err(diagnostic) = forged_result {
        assert_eq!(diagnostic.code, "SPX-H006");
        assert!(
            diagnostic
                .message
                .contains("missing Wasm32 layout for concrete variant `bytes`"),
            "{diagnostic:?}"
        );
    }
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
    let propagate = format!(
        "__spx_test_{}",
        hex_identity(&DeclarationId::new("wasm.result-hostile.propagate"))
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
for(const [tag,carrier] of [[0,12n],[1,13n]]){{
  view.setUint32(input,tag,true);view.setBigUint64(input+8,carrier,true);
  new Uint8Array(memory.buffer,output,16).fill(0xa5);
  if(instance.exports["{propagate}"](input,output)!==0)throw Error("valid owned Try status");
  if(view.getUint32(output,true)!==tag||view.getBigUint64(output+8,true)!==carrier)throw Error("owned Try selected branch transfer");
  if(stack.value!==top)throw Error("owned Try stack restore");
}}
if(drops!==2)throw Error("owned Try return was incorrectly finalized");
view.setUint32(input,0xffffffff,true);view.setBigUint64(input+8,99n,true);poison();
let trapped=false;
try{{instance.exports["{consume}"](input,output);}}catch{{trapped=true;}}
if(!trapped)throw Error("invalid Result tag did not trap");
unchanged();
if(drops!==2)throw Error("invalid Result tag granted payload cleanup authority");
view.setUint32(input,0xffffffff,true);view.setBigUint64(input+8,100n,true);
new Uint8Array(memory.buffer,output,16).fill(0xa5);trapped=false;
try{{instance.exports["{propagate}"](input,output);}}catch{{trapped=true;}}
if(!trapped)throw Error("owned Try invalid Result tag did not trap");
for(const byte of new Uint8Array(memory.buffer,output,16))if(byte!==0xa5)throw Error("owned Try invalid tag published output");
if(drops!==2)throw Error("owned Try invalid tag granted cleanup authority");
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
