use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use semaprax::hir;
use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, ProjectNpmBuild, PublicApiSubject,
    PROJECT_NPM_BUILD_SCHEMA_V7, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"module owned.api;
@id("frame.empty")
fn empty(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }
@id("frame.fail-after")
fn fail_after(input: borrow Slice<u8>, zero: i64) -> Bytes {
    let staged = bytes_copy(input);
    let ignored = 1 / zero;
    staged
}
@id("frame.fail-before")
fn fail_before(input: borrow Slice<u8>, zero: i64) -> Bytes {
    let ignored = 1 / zero;
    bytes_copy(input)
}
@id("frame.mixed")
fn mixed(flag: bool, text: borrow str, input: borrow Slice<u8>) -> Bytes {
    if flag { bytes_copy(input) } else { bytes_copy(str_as_bytes(text)) }
}
@id("frame.payload")
fn payload(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }
@id("app.main")
fn main() -> i64 { 0 }
"#;

const VARIANT_SOURCE: &str = r#"module owned.variants;
@id("variant.option")
fn option(input: borrow Slice<u8>, present: bool) -> Option<Bytes> {
    if present {
        Option<Bytes>::Some { value: bytes_copy(input) }
    } else {
        Option<Bytes>::None {}
    }
}
@id("variant.option-fail-after")
fn option_fail_after(input: borrow Slice<u8>, zero: i64) -> Option<Bytes> {
    let staged = bytes_copy(input);
    let ignored = 1 / zero;
    Option<Bytes>::Some { value: staged }
}
@id("variant.result")
fn result_value(input: borrow Slice<u8>, error: i64, ok: bool) -> Result<Bytes, i64> {
    if ok {
        Result<Bytes, i64>::Ok { value: bytes_copy(input) }
    } else {
        Result<Bytes, i64>::Err { error: error }
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        workspace_revision:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        project_graph_digest:
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    }
}

fn resolved() -> hir::ResolvedProgram {
    hir::resolve(&semaprax::check(SOURCE, "owned-api.spx").unwrap()).unwrap()
}

fn selected() -> Vec<String> {
    [
        "frame.empty",
        "frame.fail-after",
        "frame.fail-before",
        "frame.mixed",
        "frame.payload",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn variant_resolved() -> hir::ResolvedProgram {
    hir::resolve(&semaprax::check(VARIANT_SOURCE, "owned-variants.spx").unwrap()).unwrap()
}

fn variant_selected() -> Vec<String> {
    [
        "variant.option",
        "variant.option-fail-after",
        "variant.result",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

#[test]
fn exact_descriptor_driven_package_and_v7_carrier_replay() {
    let program = resolved();
    let descriptor = derive_public_api_descriptor(&program, &selected(), subject()).unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "frame-owned",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    build.verify().unwrap();
    ProjectNpmBuild::inspect_envelope(build.envelope(), build.max_bytes()).unwrap();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(value["schema"], PROJECT_NPM_BUILD_SCHEMA_V7);
    assert_eq!(
        artifacts(&build)
            .iter()
            .map(|row| row.0.as_str())
            .collect::<Vec<_>>(),
        [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.api.json",
            "package.json",
        ]
    );
    let declarations = String::from_utf8(
        artifacts(&build)
            .into_iter()
            .find(|row| row.0 == "semaprax.bindings.d.ts")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(declarations.contains("readonly \"frame.payload\": (arg0: Uint8Array) => Uint8Array;"));
    let runtime = String::from_utf8(
        artifacts(&build)
            .into_iter()
            .find(|row| row.0 == "semaprax.js")
            .unwrap()
            .1,
    )
    .unwrap();
    assert!(!runtime.contains("export async function instantiateCore"));
    assert!(!runtime.contains("export function snapshotUint8"));
    let bindings = String::from_utf8(
        artifacts(&build)
            .into_iter()
            .find(|row| row.0 == "semaprax.bindings.js")
            .unwrap()
            .1,
    )
    .unwrap();
    assert_eq!(
        bindings,
        "export { instantiate, exportIds, wasmSha256, default } from \"./semaprax.js\";\n"
    );
    assert!(!build.envelope().contains("option-owned-bytes"));
    assert!(!build.envelope().contains("result-owned-bytes-i64"));
    assert!(prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "frame-owned",
        "0.1.0",
        build.artifact_bytes() - 1,
    )
    .is_err());

    let mut tampered = build.envelope().to_owned();
    let at = tampered.find("0061736d").unwrap();
    tampered.replace_range(at..at + 2, "01");
    assert!(ProjectNpmBuild::inspect_envelope(&tampered, build.max_bytes()).is_err());

    let mut redigested: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let row = redigested["artifacts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["path"] == "semaprax.api.json")
        .unwrap();
    let hex = row["hex"].as_str().unwrap();
    let mut bytes = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    let schema = b"semaprax.owned-data-api.v1";
    let position = bytes
        .windows(schema.len())
        .position(|window| window == schema)
        .unwrap();
    bytes[position + schema.len() - 1] = b'2';
    row["hex"] = serde_json::Value::String(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    row["sha256"] = serde_json::Value::String(format!(
        "sha256:{}",
        Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ));
    assert!(ProjectNpmBuild::inspect_envelope(
        &serde_json::to_string(&redigested).unwrap(),
        build.max_bytes()
    )
    .is_err());
}

#[test]
fn node_boundary_copies_consumes_settles_and_rejects_hostiles() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = resolved();
    let descriptor = derive_public_api_descriptor(&program, &selected(), subject()).unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "frame-owned",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-owned-data-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for (path, bytes) in artifacts(&build) {
        fs::write(directory.join(path), bytes).unwrap();
    }
    fs::write(
        directory.join("contract.mjs"),
        r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));
async function instantiateRaw(){let instance=null,next=1;const entries=new Map(),decode=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn),token=root&0x7fffffff;if((root&0x80000000)===0||token===0||length>65536)throw Error('carrier invariant');return{length,root,token}},resolve=v=>{const b=entries.get(v.token);if(!(b instanceof Uint8Array)||b.length!==v.length)throw Error('stale or wrong length');return b},read=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn);if((root&0x80000000)!==0)return resolve(decode(c));if(!instance||root>instance.exports.memory.buffer.byteLength-length)throw Error('range');return new Uint8Array(instance.exports.memory.buffer,root,length)},allocate=b=>{const token=next++,copy=new Uint8Array(b);entries.set(token,copy);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(copy.length))},semantic=(code)=>{throw Object.assign(Error(`SEMAPRAX semantic failure ${code}`),{semapraxSemantic:true})};const arena={begin(){if(entries.size)throw Error('entered unsettled')},consume(c){const v=decode(c),copy=new Uint8Array(resolve(v));entries.delete(v.token);return copy},settle(){if(entries.size)throw Error('unsettled')}};const imports={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>b===0n?semantic(4):a/b,spx_rem:(a,b)=>b===0n?semantic(6):a%b,spx_neg:a=>-a,spx_contract_fail:semantic,spx_bytes_copy:c=>allocate(read(c)),spx_bytes_get:(c,i)=>{const b=read(c);return i<0n||i>=BigInt(b.length)?-1:b[Number(i)]},spx_bytes_drop:c=>{const v=decode(c);resolve(v);entries.delete(v.token)},spx_bytes_as_slice:c=>{read(c);return c},spx_owned_utf8_validate_v1:(o,l)=>{try{new TextDecoder('utf-8',{fatal:true}).decode(read((BigInt(o)<<32n)|BigInt(l)));return 1}catch{return 0}}};instance=(await WebAssembly.instantiate(wasm,{env:imports})).instance;return{instance,arena}}
const api=await instantiate(wasm);
const cases=[new Uint8Array(),new Uint8Array([0,0xff,0xc3,0x28]),new Uint8Array(65536).fill(0xff)];
for(const input of cases){const out=api.functions['frame.payload'](input);if(!(out instanceof Uint8Array)||out===input||out.length!==input.length)throw Error('fresh output');for(let i=0;i<out.length;i++)if(out[i]!==input[i])throw Error('byte mismatch')}
for(let i=0;i<8;i++){const out=api.functions['frame.payload'](new Uint8Array([i,0,255]));if(out[0]!==i)throw Error('repeat')}
const mixed=api.functions['frame.mixed'](false,'A\0€',new Uint8Array([9]));if(new TextDecoder().decode(mixed)!=='A\0€')throw Error('utf8');
for(const [args,label] of [[[new Uint8Array(65537)],'oversize'],[[new Uint16Array(2)],'typed']]){let ok=false;try{api.functions['frame.payload'](...args)}catch{ok=true}if(!ok)throw Error(label)}
if(typeof SharedArrayBuffer!=='undefined'){let ok=false;try{api.functions['frame.payload'](new Uint8Array(new SharedArrayBuffer(1)))}catch{ok=true}if(!ok)throw Error('shared')}
const beforeApi=await instantiate(wasm);let beforeError;try{beforeApi.functions['frame.fail-before'](new Uint8Array([1,2]),0n)}catch(error){beforeError=error}if(!beforeError?.message.includes('semantic failure 4'))throw Error('failure before publication identity');if(beforeApi.functions['frame.payload'](new Uint8Array([7]))[0]!==7)throw Error('settled semantic failure poisoned runtime');
const afterApi=await instantiate(wasm);let afterError;try{afterApi.functions['frame.fail-after'](new Uint8Array([1,2]),0n)}catch(error){afterError=error}if(!afterError?.message.includes('semantic failure 4'))throw Error('first failure replaced by settlement');if(afterApi.functions['frame.payload'](new Uint8Array([8]))[0]!==8)throw Error('post-staging cleanup did not settle');
const savedSet=Uint8Array.prototype.set;let intercepted=false;Uint8Array.prototype.set=function(){intercepted=true;throw Error('caller mutation hook')};const isolated=api.functions['frame.payload'](new Uint8Array([4,5]));Uint8Array.prototype.set=savedSet;if(intercepted||isolated[1]!==5)throw Error('snapshot intrinsic isolation');
const linked=await instantiateRaw(),e=linked.instance.exports,u=new Uint8Array(e.memory.buffer),name='spx_owned_v1_'+Array.from(new TextEncoder().encode('frame.payload'),b=>b.toString(16).padStart(2,'0')).join('');
for(const pointer of [1,131068]){u.fill(0x3c,pointer,Math.min(pointer+8,u.length));const before=u.slice(pointer,Math.min(pointer+8,u.length));const status=e[name](0,0,pointer);if(status!==11)throw Error('pointer status');if(u.slice(pointer,Math.min(pointer+8,u.length)).some((b,i)=>b!==before[i]))throw Error('pointer write')}
u.set([0xc3,0x28],0);if(e['spx_owned_v1_'+Array.from(new TextEncoder().encode('frame.mixed'),b=>b.toString(16).padStart(2,'0')).join('')](0,0,2,0,0,65536)!==11)throw Error('raw utf8');
if(e['spx_owned_v1_'+Array.from(new TextEncoder().encode('frame.mixed'),b=>b.toString(16).padStart(2,'0')).join('')](2,0,0,0,0,65536)!==11)throw Error('raw bool');
async function rawCarrier(){const x=await instantiateRaw(),ex=x.instance.exports,mem=new DataView(ex.memory.buffer);x.arena.begin();if(ex[name](0,0,65536)!==0)throw Error('raw status');return{x,carrier:mem.getBigInt64(65536,true)}}
{const {x,carrier}=await rawCarrier();const copy=x.arena.consume(carrier);if(!(copy instanceof Uint8Array)||copy.length!==0)throw Error('empty carrier');x.arena.settle();let ok=false;try{x.arena.consume(carrier)}catch{ok=true}if(!ok)throw Error('double consume')}
{const {x,carrier}=await rawCarrier();let ok=false;try{x.arena.consume(carrier+1n)}catch{ok=true}if(!ok)throw Error('wrong length');x.arena.consume(carrier);x.arena.settle()}
{const {x,carrier}=await rawCarrier();for(const forged of [0n,BigInt.asIntN(64,BigInt.asUintN(64,carrier)+(1n<<32n))]){let ok=false;try{x.arena.consume(forged)}catch{ok=true}if(!ok)throw Error('zero or stale token')}x.arena.consume(carrier);x.arena.settle()}
{const {x}=await rawCarrier();let ok=false;try{x.arena.settle()}catch{ok=true}if(!ok)throw Error('unsettled arena')}
for(const id of ['frame.fail-before','frame.fail-after']){const x=await instantiateRaw(),ex=x.instance.exports,mem=new Uint8Array(ex.memory.buffer),raw='spx_owned_v1_'+Array.from(new TextEncoder().encode(id),b=>b.toString(16).padStart(2,'0')).join('');mem.fill(0x3c,131064,131072);if(ex[raw](0,0,0n,131064)!==11)throw Error('alias was not rejected');if(mem.slice(131064,131072).some(b=>b!==0x3c))throw Error('aliased failure modified public out')}
{const detached=new Uint8Array([1]);structuredClone(detached.buffer,{transfer:[detached.buffer]});let ok=false;try{api.functions['frame.payload'](detached)}catch{ok=true}if(!ok)throw Error('detached')}
try{const buffer=new ArrayBuffer(1,{maxByteLength:2});if(buffer.resizable){let ok=false;try{api.functions['frame.payload'](new Uint8Array(buffer))}catch{ok=true}if(!ok)throw Error('resizable')}}catch{}
console.log('owned-data-contract-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("contract.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn v8_admits_the_complete_input_tuple_before_payload_allocation() {
    // This focused physical gate requires Node; absence is not passing evidence.
    for (label, result, value, scalar) in [
        ("direct", "Bytes", "bytes_copy(input)", false),
        (
            "option",
            "Option<Bytes>",
            "Option<Bytes>::Some { value: bytes_copy(input) }",
            false,
        ),
        (
            "result",
            "Result<Bytes, i64>",
            "Result<Bytes, i64>::Ok { value: bytes_copy(input) }",
            false,
        ),
        ("mixed", "Bytes", "bytes_copy(input)", true),
    ] {
        let extra = if scalar {
            "@id(\"probe.scalar\") fn scalar() -> i64 { 7 }\n@id(\"probe.mixed\") fn mixed(input: borrow Slice<u8>, text: borrow str, value: i64, flag: bool) -> Bytes { if flag { bytes_copy(input) } else { bytes_from_str(text) } }"
        } else {
            ""
        };
        let source = format!("module probe.input;\n@id(\"probe.bytes\") fn copy(input: borrow Slice<u8>, text: borrow str, other: borrow Slice<u8>) -> {result} {{ {value} }}\n{extra}\n@id(\"probe.main\") fn main() -> i64 {{ 0 }}\n");
        let program = hir::resolve(&semaprax::check(&source, "input.spx").unwrap()).unwrap();
        let mut selected = vec!["probe.bytes".to_owned()];
        if scalar {
            selected.push("probe.scalar".to_owned());
            selected.push("probe.mixed".to_owned());
        }
        let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
        let build = prepare_owned_data_npm_build(
            &program,
            &descriptor,
            "input-probe",
            "0.1.0",
            40 * 1024 * 1024,
        )
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "semaprax-input-v8-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        for (path, bytes) in artifacts(&build) {
            fs::write(directory.join(path), bytes).unwrap();
        }
        fs::write(
            directory.join("admission.mjs"),
            include_str!("fixtures/owned_data_input_admission_v8.mjs"),
        )
        .unwrap();
        let output = Command::new("node")
            .arg("admission.mjs")
            .current_dir(&directory)
            .output()
            .expect("Node is required by the explicit input-admission gate");
        assert!(
            output.status.success(),
            "{label}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "owned-input-admission-v8-ok"
        );
    }
}

#[test]
fn option_result_package_is_exact_and_project_v8_stays_inactive() {
    assert!(
        semaprax::project::ProjectManifest::parse("schema = \"semaprax.project.v8\"\n").is_err()
    );
    let program = variant_resolved();
    let descriptor =
        derive_public_api_descriptor(&program, &variant_selected(), subject()).unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "owned-variants",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    build.verify().unwrap();
    let package = artifacts(&build);
    let declarations = String::from_utf8(
        package
            .iter()
            .find(|row| row.0 == "semaprax.bindings.d.ts")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(declarations.contains("export type OptionalBytes = Uint8Array | null;"));
    assert!(declarations.contains(
        "readonly \"variant.option\": (arg0: Uint8Array, arg1: boolean) => OptionalBytes;"
    ));
    assert!(declarations.contains("export type SemapraxResult<T, E> ="));
    assert!(declarations.contains("readonly \"variant.result\": (arg0: Uint8Array, arg1: bigint, arg2: boolean) => SemapraxResult<Uint8Array, bigint>;"));
    let metadata = String::from_utf8(
        package
            .iter()
            .find(|row| row.0 == "semaprax.api.json")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    assert!(metadata.contains("\"result\":\"option-owned-bytes\""));
    assert!(metadata.contains("\"result\":\"result-owned-bytes-i64\""));
    let runtime = String::from_utf8(
        package
            .iter()
            .find(|row| row.0 == "semaprax.js")
            .unwrap()
            .1
            .clone(),
    )
    .unwrap();
    let tag_check = runtime.find("if(tag>1)").unwrap();
    let payload_read = runtime.find("view.getBigInt64(RESULT+8,true)").unwrap();
    assert!(tag_check < payload_read);
    assert!(!runtime.contains("export async function instantiateCore"));
    assert!(!runtime.contains("export function createArena"));
}

#[test]
fn node_option_result_carriers_preserve_tags_liveness_and_first_failure() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = variant_resolved();
    let descriptor =
        derive_public_api_descriptor(&program, &variant_selected(), subject()).unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "owned-variants",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-owned-variants-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    for (path, bytes) in artifacts(&build) {
        fs::write(directory.join(path), bytes).unwrap();
    }
    fs::write(
        directory.join("contract.mjs"),
        r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm=new Uint8Array(fs.readFileSync(new URL('./app.wasm',import.meta.url)));
const api=await instantiate(wasm),option=api.functions['variant.option'],result=api.functions['variant.result'];
if(option(new Uint8Array([9]),false)!==null)throw Error('None mapping');
for(const input of [new Uint8Array(),new Uint8Array(65536).fill(0xff)]){const value=option(input,true);if(!(value instanceof Uint8Array)||value===input||value.length!==input.length)throw Error('Some carrier');for(let i=0;i<value.length;i++)if(value[i]!==input[i])throw Error('Some bytes')}
for(let i=0;i<8;i++){const value=option(new Uint8Array([i]),true);if(value[0]!==i)throw Error('Some token rotation')}
const ok=result(new Uint8Array([0,255]),7n,true);if(ok.ok!==true||!(ok.value instanceof Uint8Array)||ok.value[1]!==255)throw Error('Ok mapping');
for(const error of [0n,-(1n<<63n),(1n<<63n)-1n]){const value=result(new Uint8Array([1]),error,false);if(value.ok!==false||value.error!==error||Object.keys(value).join(',')!=='ok,error')throw Error('Err mapping')}
let primary;try{api.functions['variant.option-fail-after'](new Uint8Array([1,2]),0n)}catch(error){primary=error}if(!primary?.message.includes('semantic failure 4'))throw Error('first failure replaced');if(option(new Uint8Array([7]),true)[0]!==7)throw Error('settled semantic failure poisoned runtime');
async function raw(){let instance=null,next=1;const entries=new Map(),decode=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn),token=root&0x7fffffff;if((root&0x80000000)===0||token===0||length>65536)throw Error('carrier invariant');return{length,token}},resolve=v=>{const b=entries.get(v.token);if(!(b instanceof Uint8Array)||b.length!==v.length)throw Error('stale carrier');return b},read=c=>{const w=BigInt.asUintN(64,c),length=Number(w&0xffffffffn),root=Number((w>>32n)&0xffffffffn);if((root&0x80000000)!==0)return resolve(decode(c));if(!instance||root>instance.exports.memory.buffer.byteLength-length)throw Error('range');return new Uint8Array(instance.exports.memory.buffer,root,length)},allocate=b=>{const token=next++,copy=new Uint8Array(b);entries.set(token,copy);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(copy.length))},semantic=code=>{throw Object.assign(Error(`semantic ${code}`),{semapraxSemantic:true})},arena={begin(){if(entries.size)throw Error('entered unsettled')},consume(c){const v=decode(c),copy=new Uint8Array(resolve(v));entries.delete(v.token);return copy},settle(){if(entries.size)throw Error('unsettled')}};const imports={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>b===0n?semantic(4):a/b,spx_rem:(a,b)=>b===0n?semantic(6):a%b,spx_neg:a=>-a,spx_contract_fail:semantic,spx_bytes_copy:c=>allocate(read(c)),spx_bytes_get:(c,i)=>{const b=read(c);return i<0n||i>=BigInt(b.length)?-1:b[Number(i)]},spx_bytes_drop:c=>{const v=decode(c);resolve(v);entries.delete(v.token)},spx_bytes_as_slice:c=>{read(c);return c},spx_owned_utf8_validate_v1:()=>1};instance=(await WebAssembly.instantiate(wasm,{env:imports})).instance;return{instance,arena}}
const symbol=id=>'spx_owned_v1_'+Array.from(new TextEncoder().encode(id),b=>b.toString(16).padStart(2,'0')).join('');
{const x=await raw(),e=x.instance.exports,u=new Uint8Array(e.memory.buffer),v=new DataView(e.memory.buffer),out=65536;u.set([3,4],0);u.fill(0x3c,out,out+16);x.arena.begin();if(e[symbol('variant.option')](0,2,0,out)!==0||v.getUint32(out,true)!==0)throw Error('raw None tag');if(u.slice(out+8,out+16).some(b=>b!==0x3c))throw Error('inactive None payload accessed');x.arena.settle();v.setUint32(out,2,true);let payloadReads=0,failed=false;try{const tag=v.getUint32(out,true);if(tag>1)throw Error('invalid tag');payloadReads++;v.getBigInt64(out+8,true)}catch{failed=true}if(!failed||payloadReads!==0)throw Error('invalid tag did not fail before payload')}
{const x=await raw(),e=x.instance.exports,v=new DataView(e.memory.buffer),out=65536;x.arena.begin();if(e[symbol('variant.option')](0,0,1,out)!==0||v.getUint32(out,true)!==1)throw Error('raw Some tag');const carrier=v.getBigInt64(out+8,true);x.arena.consume(carrier);x.arena.settle();let failed=false;try{x.arena.consume(carrier)}catch{failed=true}if(!failed)throw Error('Some double consume')}
{const x=await raw(),e=x.instance.exports,v=new DataView(e.memory.buffer),out=65536;x.arena.begin();if(e[symbol('variant.result')](0,0,-9n,0,out)!==0||v.getUint32(out,true)!==1||v.getBigInt64(out+8,true)!==-9n)throw Error('raw Err payload');x.arena.settle()}
{const x=await raw(),e=x.instance.exports,u=new Uint8Array(e.memory.buffer),v=new DataView(e.memory.buffer),out=65536;x.arena.begin();if(e[symbol('variant.option')](0,0,1,out)!==0)throw Error('raw liveness setup');v.setUint32(out,0,true);let failed=false;try{x.arena.settle()}catch{failed=true}if(!failed)throw Error('tag/liveness mismatch')}
{const x=await raw(),e=x.instance.exports,u=new Uint8Array(e.memory.buffer);for(const out of [1,131064,131056]){u.fill(0x3c,out,Math.min(out+16,u.length));const before=u.slice(out,Math.min(out+16,u.length));if(e[symbol('variant.option')](0,0,0,out)!==11)throw Error('variant pointer status');if(u.slice(out,Math.min(out+16,u.length)).some((b,i)=>b!==before[i]))throw Error('variant pointer modified')}}
console.log('owned-variant-contract-ok');
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("contract.mjs")
        .current_dir(&directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn target_replays_descriptor_against_held_hir_before_lowering() {
    let slice_program = resolved();
    let descriptor =
        derive_public_api_descriptor(&slice_program, &["frame.payload".to_owned()], subject())
            .unwrap();
    let foreign = r#"module foreign.api;
@id("frame.payload") fn payload(input: borrow str) -> Bytes { bytes_copy(str_as_bytes(input)) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let foreign_program =
        hir::resolve(&semaprax::check(foreign, "foreign-api.spx").unwrap()).unwrap();
    assert!(
        semaprax::wasm::emit_resolved_module_with_owned_data_exports(&foreign_program, &descriptor)
            .is_err()
    );

    let option_program = variant_resolved();
    let option_descriptor =
        derive_public_api_descriptor(&option_program, &["variant.option".to_owned()], subject())
            .unwrap();
    let result_with_same_id = r#"module foreign.variant;
@id("variant.option")
fn value(input: borrow Slice<u8>, present: bool) -> Result<Bytes, i64> {
    if present {
        Result<Bytes, i64>::Ok { value: bytes_copy(input) }
    } else {
        Result<Bytes, i64>::Err { error: 0 }
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let result_program =
        hir::resolve(&semaprax::check(result_with_same_id, "foreign-variant.spx").unwrap())
            .unwrap();
    assert!(
        semaprax::wasm::emit_resolved_module_with_owned_data_exports(
            &result_program,
            &option_descriptor
        )
        .is_err()
    );
}

#[test]
fn legacy_project_v3_wasm_projection_remains_byte_pinned() {
    let source = r#"module legacy.data;
@id("legacy.length") fn length(input: borrow Slice<u8>) -> usize { byte_len(input) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(&semaprax::check(source, "legacy-data.spx").unwrap()).unwrap();
    let bytes = semaprax::wasm::emit_resolved_module_with_byte_exports(
        &program,
        &["legacy.length".to_owned()],
    )
    .unwrap();
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "774a256a968f7ed80e611ea6866ffadd79d9da39a7a71a04f23fc7dcbdfbd049"
    );
}
