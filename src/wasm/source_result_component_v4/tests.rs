use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use super::*;

const SOURCE: &str = r#"module test.component_source_result_v4;

@id("component.source")
fn source(value: i64, reject: bool) -> Result<i64, bool>
{
    if reject { Result<i64, bool>::Err { error: value > 0 } } else { Result<i64, bool>::Ok { value: value } }
}

@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

fn program() -> Program {
    crate::parse(SOURCE, Path::new("component-source-result-v4.spx")).unwrap()
}

#[test]
fn deterministic_core_is_upstream_valid_import_free_and_layout_bound() {
    let first = emit_private_source_result_core_v4(&program()).unwrap();
    let second = emit_private_source_result_core_v4(&program()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.source_revision,
        "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254"
    );
    assert_eq!(first.prelude_digest, prelude::digest_v1());
    assert_ne!(
        first.result_i64_bool_layout_digest,
        first.result_bool_bool_layout_digest
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&first.bytes)
        .expect("pinned upstream validator rejected source-result v4 core");
    assert_eq!(&first.bytes[..8], b"\0asm\x01\0\0\0");
    assert_eq!(Sha256::digest(&first.bytes), Sha256::digest(&second.bytes));
}

#[test]
fn node_executes_language_results_statuses_poison_and_invalid_closure_values() {
    let artifact = emit_test_profile(&program()).unwrap();
    let bytes = artifact
        .bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"const bytes=new Uint8Array([{bytes}]);
const instance=(await WebAssembly.instantiate(bytes,{{}})).instance;
const memory=instance.exports.memory;
const view=new DataView(memory.buffer);
const statusOut=instance.exports["{status_out}"];
const canonical=instance.exports["{canonical}"];
const validate=instance.exports["{validate}"];
const out=512, hostile=640, hostileOut=704, poison=0xa5;
const poisonBytes=(pointer,length)=>new Uint8Array(memory.buffer,pointer,length).fill(poison);
const assertPoison=(pointer,length,label)=>{{for(const byte of new Uint8Array(memory.buffer,pointer,length))if(byte!==poison)throw new Error(`${{label}} published output`);}};
const cases=[
  [83n,0,2n,0,0,1,"ok-true"],
  [-3n,0,2n,0,0,0,"ok-false"],
  [1n,1,0n,0,1,1,"err-true-skips-division"],
  [-1n,1,0n,0,1,0,"err-false-skips-division"],
  [(1n<<63n)-1n,0,1n,0x02000001,null,null,"addition-overflow"],
  [1n,0,0n,0x02000004,null,null,"division-zero"],
  [(1n<<63n)-1n,0,0n,0x02000001,null,null,"sticky-add-before-division"],
  [-99n,0,1n,0x01000001,null,null,"false-precondition"],
  [1n,0,13n,0x01000002,null,null,"false-postcondition-after-ok"],
  [1n,1,13n,0x01000002,null,null,"false-postcondition-after-err"],
];
for(let round=0;round<8;round++)for(const [value,reject,divisor,status,tag,payload,label] of cases){{
  poisonBytes(out,8);
  const actual=statusOut(value,reject,divisor,out);
  if(actual!==status)throw new Error(`${{label}} status ${{actual}}`);
  if(status===0){{
    if(view.getUint32(out,true)!==tag||view.getUint32(out+4,true)!==payload)throw new Error(`${{label}} language result`);
  }}else assertPoison(out,8,label);
}}
poisonBytes(out,8);
if(statusOut(1n,2,2n,out)!==-1)throw new Error("noncanonical raw bool admitted");
assertPoison(out,8,"noncanonical raw bool");
for(const [tag,payload,label] of [[2,0,"invalid-tag"],[0,2,"invalid-bool"],[1,0,"valid-err"]]){{
  view.setUint32(hostile,tag,true);view.setUint32(hostile+4,payload,true);poisonBytes(hostileOut,8);
  const actual=validate(hostile,hostileOut);
  if(tag>1||payload>1){{if(actual!==-1)throw new Error(`${{label}} admitted`);assertPoison(hostileOut,8,label);}}
  else if(actual!==0||view.getUint32(hostileOut,true)!==tag||view.getUint32(hostileOut+4,true)!==payload)throw new Error("valid validator copy");
}}
const canonicalCases=[
  [83n,0,2n,0,0,1,"canonical-ok"],
  [1n,1,0n,0,1,1,"canonical-language-err"],
  [1n,0,0n,1,2,4,"canonical-status"],
];
for(const [value,reject,divisor,outer,innerOrClass,payloadOrCode,label] of canonicalCases){{
  const pointer=canonical(value,reject,divisor);
  if(pointer!=={result_area}||view.getUint8(pointer)!==outer)throw new Error(`${{label}} outer`);
  if(outer===0){{if(view.getUint8(pointer+4)!==innerOrClass||view.getUint8(pointer+5)!==payloadOrCode)throw new Error(`${{label}} inner`);}}
  else if(view.getUint8(pointer+16)!==innerOrClass||view.getUint32(pointer+12,true)!==payloadOrCode||view.getUint8(pointer+17)!==1||view.getUint8(pointer+18)!==0)throw new Error(`${{label}} status`);
}}
console.log("source-result-v4-core-ok");
"#,
        status_out = STATUS_OUT_EXPORT,
        canonical = CANONICAL_EXPORT,
        validate = TEST_VALIDATE_EXPORT,
        result_area = RESULT_AREA,
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required by the established Wasm gate");
    assert!(
        output.status.success(),
        "Node source-result core failed with {:?}: {}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn excluded_signatures_nominals_and_effects_fail_before_emission() {
    let invalid_source = SOURCE.replace("value: i64", "value: bool");
    let invalid = crate::parse(&invalid_source, Path::new("invalid-source-result-v4.spx")).unwrap();
    assert_eq!(
        emit_private_source_result_core_v4(&invalid)
            .unwrap_err()
            .code,
        "SPX-T208",
        "source diagnostics must precede private profile admission"
    );

    let selected = r#"@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<bool, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<bool, bool>::Ok { value: (checked + 1) / divisor > 0 }
}"#;
    let wrong_param = r#"@id("component.evaluate")
fn evaluate(value: bool, reject: bool, divisor: i64) -> Result<bool, bool>
    ensures divisor != 13
{
    let checked = source(if value { 1 } else { 0 }, reject)?;
    Result<bool, bool>::Ok { value: checked / divisor > 0 }
}"#;
    let wrong_result = r#"@id("component.evaluate")
fn evaluate(value: i64, reject: bool, divisor: i64) -> Result<i64, bool>
    requires value != -99
    ensures divisor != 13
{
    let checked = source(value, reject)?;
    Result<i64, bool>::Ok { value: checked / divisor }
}"#;
    for source in [
            SOURCE.replace(selected, wrong_param),
            SOURCE.replace(selected, wrong_result),
            SOURCE.replace("@id(\"component.source\")", "@id(\"component.other\")"),
            SOURCE.replace(
                "module test.component_source_result_v4;",
                "module test.component_source_result_v4; @id(\"user.choice\") variant Choice { @id(\"user.choice.none\") None, }",
            ),
        ] {
            let parsed = crate::parse(&source, Path::new("excluded-source-result-v4.spx")).unwrap();
            assert_eq!(
                emit_private_source_result_core_v4(&parsed).unwrap_err().code,
                "SPX-WIT108"
            );
        }
}
