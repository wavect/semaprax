//! Authority-free npm projection for the closed WP-10/WP-11 owned-data results.

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::ResolvedProgram;
use crate::project::{
    replay_public_api_descriptor, PublicApiDescriptor, PublicApiParameterType, PublicApiResultType,
    PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

use super::carrier::{
    artifact, payload_digest_artifacts_v7, render_carrier_artifacts, trusted_binding, NpmArtifact,
    NpmBuildIdentity,
};
use super::{package_error, validate_carrier_limit, ProjectNpmBuild};

pub const OWNED_DATA_PACKAGE_PATHS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];
pub const OWNED_DATA_API_SCHEMA: &str = "semaprax.owned-data-api.v1";
pub(super) const MAX_WASM_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnedExport {
    stable_id: String,
    wasm_export: String,
    parameters: Vec<PublicApiParameterType>,
    result: PublicApiResultType,
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    descriptor: &PublicApiDescriptor,
    package: &str,
    version: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    validate_carrier_limit(0, max_bytes)?;
    validate_identity(package, version)?;
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = descriptor_subject(descriptor);
    let replayed = replay_public_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(package_error("owned-data descriptor replay disagrees"));
    }
    let semantic_recipe = super::render_owned_data_semantic_recipe(program)?;
    let replayed_program = super::semantic_recipe_v8::replay_against(program, &semantic_recipe)?;
    let replayed_descriptor = replay_public_api_descriptor(
        &replayed_program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed_descriptor != descriptor {
        return Err(package_error(
            "owned-data descriptor disagrees with independent semantic replay",
        ));
    }
    let exports = exports_from_descriptor(descriptor)?;
    let wasm = crate::wasm::emit_resolved_module_with_owned_data_exports(program, descriptor)?;
    let replayed_wasm = crate::wasm::emit_resolved_module_with_owned_data_exports(
        &replayed_program,
        &replayed_descriptor,
    )?;
    if replayed_wasm != wasm {
        return Err(package_error(
            "owned-data Wasm disagrees with independent semantic replay",
        ));
    }
    let artifacts = render_package(package, version, descriptor, &wasm, &exports)?;
    let artifact_bytes = artifacts.iter().try_fold(0_usize, |total, artifact| {
        total
            .checked_add(artifact.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("owned-data npm artifacts exceed the trusted limit"))
    })?;
    let identity = NpmBuildIdentity {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        package,
        version,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
        semantic_recipe: &semantic_recipe,
    };
    let payload_digest = payload_digest_artifacts_v7(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        super::PROJECT_NPM_BUILD_SCHEMA_V7,
        identity,
        &artifacts,
        artifact_bytes,
        &payload_digest,
    );
    validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 6],
) -> Result<(), Diagnostic> {
    validate_identity(identity.package, identity.version)?;
    if identity.project_schema != PUBLIC_OWNED_DATA_PROJECT_SCHEMA {
        return Err(package_error("owned-data carrier Project schema disagrees"));
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(artifact_bytes(artifacts, "semaprax.api.json")?)
            .map_err(|_| package_error("owned-data API metadata is not valid JSON"))?;
    let object = metadata
        .as_object()
        .ok_or_else(|| package_error("owned-data API metadata must be one object"))?;
    super::require_exact_keys(
        object,
        &[
            "artifacts",
            "descriptor",
            "descriptor_digest",
            "limits",
            "package",
            "schema",
            "settlement",
            "target",
            "version",
            "wasm",
        ],
    )?;
    if super::json_string(object, "schema")? != OWNED_DATA_API_SCHEMA
        || super::json_string(object, "package")? != identity.package
        || super::json_string(object, "version")? != identity.version
    {
        return Err(package_error("owned-data API metadata identity disagrees"));
    }
    let descriptor_bytes = super::json_string(object, "descriptor")?.as_bytes();
    let program = super::semantic_recipe_v8::replay(identity.semantic_recipe)?;
    let descriptor_value: serde_json::Value = serde_json::from_slice(descriptor_bytes)
        .map_err(|_| package_error("owned-data descriptor metadata is invalid"))?;
    let exports_value = descriptor_value
        .get("exports")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("owned-data descriptor exports are invalid"))?;
    let selected = exports_value
        .iter()
        .map(|row| {
            row.get("stable_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| package_error("owned-data descriptor export is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let subject = PublicApiSubject {
        project_schema: identity.project_schema,
        project_revision: identity.project_revision,
        workspace_revision: identity.workspace_revision,
        project_graph_digest: identity.project_graph_digest,
    };
    let descriptor = replay_public_api_descriptor(
        &program,
        &selected,
        subject,
        descriptor_bytes,
        super::json_string(object, "descriptor_digest")?,
    )?;
    if super::json_string(object, "descriptor_digest")? != descriptor.digest() {
        return Err(package_error("owned-data descriptor digest disagrees"));
    }
    let exports = exports_from_descriptor(&descriptor)?;
    let wasm = artifact_bytes(artifacts, "app.wasm")?;
    wasmparser::Validator::new()
        .validate_all(wasm)
        .map_err(|_| package_error("owned-data app.wasm is not structurally valid"))?;
    let expected_wasm =
        crate::wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor)?;
    if wasm != expected_wasm {
        return Err(package_error(
            "owned-data app.wasm disagrees with descriptor replay",
        ));
    }
    let expected = render_package(
        identity.package,
        identity.version,
        &descriptor,
        wasm,
        &exports,
    )?;
    if artifacts != &expected {
        return Err(package_error(
            "owned-data npm artifacts disagree with replay",
        ));
    }
    Ok(())
}

fn exports_from_descriptor(
    descriptor: &PublicApiDescriptor,
) -> Result<Vec<OwnedExport>, Diagnostic> {
    descriptor
        .exports()
        .iter()
        .map(|export| {
            Ok(OwnedExport {
                stable_id: export.stable_id().as_str().to_owned(),
                wasm_export: raw_symbol(export.stable_id().as_str()),
                parameters: export.parameters().iter().map(|value| value.ty()).collect(),
                result: export.result(),
            })
        })
        .collect()
}

fn render_package(
    package: &str,
    version: &str,
    descriptor: &PublicApiDescriptor,
    wasm: &[u8],
    exports: &[OwnedExport],
) -> Result<[NpmArtifact; 6], Diagnostic> {
    if wasm.is_empty() || wasm.len() > MAX_WASM_BYTES {
        return Err(package_error("owned-data Wasm size is outside its bound"));
    }
    let digest = hex_sha256(wasm);
    let runtime = render_runtime(&digest, exports);
    let bindings = render_bindings(exports, &digest);
    let declarations = render_declarations(exports);
    let metadata = render_metadata(package, version, descriptor, &digest, exports);
    let package_json = render_package_json(package, version);
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.api.json", metadata.as_bytes()),
        artifact("package.json", package_json.as_bytes()),
    ])
}

fn render_runtime(wasm_sha256: &str, exports: &[OwnedExport]) -> String {
    let mut runtime = render_runtime_prelude(wasm_sha256);
    let facade = if exports.iter().any(|export| {
        matches!(
            export.result,
            PublicApiResultType::I64 | PublicApiResultType::Bool | PublicApiResultType::Usize
        )
    }) {
        render_mixed_runtime_facade(exports)
    } else if exports
        .iter()
        .any(|export| export.result != PublicApiResultType::OwnedBytes)
    {
        render_variant_runtime_facade(exports)
    } else {
        render_runtime_facade(exports)
    };
    runtime.push_str(&facade);
    runtime
}

pub(super) fn render_runtime_prelude(wasm_sha256: &str) -> String {
    format!(
        r#"const EXPECTED_WASM_SHA256 = {digest};
const TypedArrayPrototype=Object.getPrototypeOf(Uint8Array.prototype),typedTag=Object.getOwnPropertyDescriptor(TypedArrayPrototype,Symbol.toStringTag).get,typedBuffer=Object.getOwnPropertyDescriptor(TypedArrayPrototype,"buffer").get,typedOffset=Object.getOwnPropertyDescriptor(TypedArrayPrototype,"byteOffset").get,typedLength=Object.getOwnPropertyDescriptor(TypedArrayPrototype,"byteLength").get,typedSet=TypedArrayPrototype.set,reflectApply=Reflect.apply,objectGetPrototypeOf=Object.getPrototypeOf;
const sharedLength=typeof SharedArrayBuffer==="undefined"?null:Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype,"byteLength").get,resizable=Object.getOwnPropertyDescriptor(ArrayBuffer.prototype,"resizable")?.get,arrayBufferSlice=ArrayBuffer.prototype.slice;
function snapshotUint8(value,label){{let buffer;try{{buffer=reflectApply(typedBuffer,value,[])}}catch{{throw new TypeError(`${{label}} must be an ordinary attached Uint8Array`)}}if(objectGetPrototypeOf(value)!==Uint8Array.prototype||reflectApply(typedTag,value,[])!=="Uint8Array")throw new TypeError(`${{label}} must be an ordinary Uint8Array`);if(sharedLength!==null){{let shared=false;try{{reflectApply(sharedLength,buffer,[]);shared=true}}catch{{}}if(shared)throw new TypeError(`${{label}} must not use SharedArrayBuffer`)}}if(resizable!==undefined&&reflectApply(resizable,buffer,[]))throw new TypeError(`${{label}} must not use a resizable ArrayBuffer`);reflectApply(arrayBufferSlice,buffer,[0,0]);const offset=reflectApply(typedOffset,value,[]),length=reflectApply(typedLength,value,[]);if(!Number.isSafeInteger(offset)||!Number.isSafeInteger(length))throw new TypeError(`${{label}} is detached or out of bounds`);const copy=new Uint8Array(length);reflectApply(typedSet,copy,[value,0]);if(copy.byteLength!==length)throw new TypeError(`${{label}} changed during snapshot`);return copy}}
function createArena(){{const entries=new Map();let nextToken=1,instance=null,poisoned=false;const decode=carrier=>{{if(typeof carrier!=="bigint")throw new Error("SEMAPRAX owned carrier is not i64");const word=BigInt.asUintN(64,carrier),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn),token=root&0x7fffffff;if((root&0x80000000)===0||token===0)throw new Error("SEMAPRAX owned carrier token invariant");if(length>65536)throw new Error("SEMAPRAX owned carrier length invariant");return{{token,length}}}},resolve=value=>{{const bytes=entries.get(value.token);if(!(bytes instanceof Uint8Array)||bytes.byteLength!==value.length)throw new Error("SEMAPRAX stale or wrong-length owned carrier");return bytes}},allocate=bytes=>{{if(entries.size>=16||nextToken>0x7fffffff)throw new Error("SEMAPRAX owned arena exhausted");const token=nextToken++,owned=new Uint8Array(bytes);entries.set(token,owned);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(owned.byteLength))}},read=carrier=>{{const word=BigInt.asUintN(64,carrier),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn);if((root&0x80000000)!==0)return resolve(decode(carrier));const memory=instance?.exports.memory;if(!(memory instanceof WebAssembly.Memory)||root>memory.buffer.byteLength-length)throw new Error("SEMAPRAX borrowed carrier range invariant");return new Uint8Array(memory.buffer,root,length)}},utf8=(offset,length)=>{{try{{new TextDecoder("utf-8",{{fatal:true}}).decode(read((BigInt(offset)<<32n)|BigInt(length)));return 1}}catch{{return 0}}}};return Object.freeze({{imports:Object.freeze({{spx_bytes_copy:c=>allocate(read(c)),spx_bytes_get:(c,i)=>{{const b=read(c),n=BigInt.asUintN(64,i);return n>=BigInt(b.byteLength)?-1:b[Number(n)]}},spx_bytes_drop:c=>{{const v=decode(c);resolve(v);entries.delete(v.token)}},spx_bytes_as_slice:c=>{{read(c);return BigInt.asIntN(64,c)}},spx_owned_utf8_validate_v1:utf8}}),bind(v){{if(instance!==null)throw new Error("SEMAPRAX arena already bound");instance=v}},begin(){{if(poisoned||entries.size!==0){{poisoned=true;throw new Error("SEMAPRAX arena entered unsettled")}}}},consume(c){{const v=decode(c),copy=new Uint8Array(resolve(v));entries.delete(v.token);return copy}},settle(){{if(entries.size!==0){{poisoned=true;throw new Error("SEMAPRAX arena did not settle")}}}},poison(){{poisoned=true}}}})}}
async function instantiateCore(input){{const bytes=snapshotUint8(input,"SEMAPRAX module bytes");if(globalThis.crypto?.subtle===undefined)throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");const hash=new Uint8Array(await crypto.subtle.digest("SHA-256",bytes)),actual=Array.from(hash,v=>v.toString(16).padStart(2,"0")).join("");if(actual!==EXPECTED_WASM_SHA256)throw new Error("SEMAPRAX WebAssembly artifact authentication failed");const arena=createArena(),fail=(code,domain)=>{{throw Object.assign(new Error(`SEMAPRAX semantic failure ${{code}}`),{{code,domain,semapraxSemantic:true}})}},checked=(v,c)=>{{if(v<-(1n<<63n)||v>(1n<<63n)-1n)fail(c,"semaprax.arithmetic.v1");return v}},env=Object.freeze({{spx_add:(a,b)=>checked(a+b,1),spx_sub:(a,b)=>checked(a-b,2),spx_mul:(a,b)=>checked(a*b,3),spx_div:(a,b)=>b===0n?fail(4,"semaprax.arithmetic.v1"):a/b,spx_rem:(a,b)=>b===0n?fail(6,"semaprax.arithmetic.v1"):a%b,spx_neg:a=>checked(-a,8),spx_contract_fail:c=>fail(c,"semaprax.contract.v1"),...arena.imports}}),result=await WebAssembly.instantiate(bytes,Object.freeze({{env}}));arena.bind(result.instance);return Object.freeze({{instance:result.instance,arena,copyInto:(target,source,offset)=>reflectApply(typedSet,target,[source,offset])}})}}
export const wasmSha256=EXPECTED_WASM_SHA256;
"#,
        digest = quote_json(wasm_sha256)
    )
}

fn render_mixed_runtime_facade(exports: &[OwnedExport]) -> String {
    let facts = exports
        .iter()
        .map(|export| {
            format!(
                "[{},Object.freeze({{raw:{},params:Object.freeze([{}]),result:{}}})]",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|parameter| quote_json(parameter_wire(*parameter)))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.wire_name()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"const FACTS=new Map([{facts}]),IDS=Object.freeze(Array.from(FACTS.keys())),RESULT=65536,POISON=0xa5;
const encoder=new TextEncoder();
function snapshot(value,type,label){{if(type==="borrow-str"){{if(typeof value!=="string")throw new TypeError(`${{label}} must be a string`);for(let i=0;i<value.length;i++){{const unit=value.charCodeAt(i);if(unit>=0xd800&&unit<=0xdbff){{if(++i>=value.length||value.charCodeAt(i)<0xdc00||value.charCodeAt(i)>0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}else if(unit>=0xdc00&&unit<=0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}return encoder.encode(value)}}if(type==="borrow-slice-u8")return snapshotUint8(value,label);if(type==="i64"){{if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError(`${{label}} must be signed i64 bigint`);return value}}if(type==="bool"){{if(typeof value!=="boolean")throw new TypeError(`${{label}} must be boolean`);return value?1:0}}throw new Error("unknown descriptor parameter type")}}
function resultSize(result){{if(result==="bool")return 4;return result==="option-owned-bytes"||result==="result-owned-bytes-i64"?16:8}}
function facade(linked){{const e=linked.instance.exports,memory=e.memory;if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength!==131072)throw new Error("SEMAPRAX fixed owned-data memory invariant");let busy=false,poisoned=false;function invoke(id,values){{if(poisoned)throw new Error("SEMAPRAX owned-data runtime is poisoned");if(busy)throw new Error("SEMAPRAX owned-data call is non-reentrant");const fact=FACTS.get(id);if(!fact)throw new RangeError(`unknown SEMAPRAX export: ${{id}}`);if(values.length!==fact.params.length)throw new TypeError("SEMAPRAX argument count disagrees");const snapshots=values.map((value,index)=>snapshot(value,fact.params[index],`argument ${{index}}`));let used=0;for(const value of snapshots)if(value instanceof Uint8Array){{if(value.byteLength>65536-used)throw new RangeError("SEMAPRAX borrowed input capacity exceeded");used+=value.byteLength}}busy=true;let began=false,answer,primary=null;const bytes=new Uint8Array(memory.buffer),view=new DataView(memory.buffer),size=resultSize(fact.result);try{{let offset=0;const raw=[];for(const value of snapshots){{if(value instanceof Uint8Array){{linked.copyInto(bytes,value,offset);raw.push(offset,value.byteLength);offset+=value.byteLength}}else raw.push(value)}}bytes.fill(POISON,RESULT,RESULT+size);linked.arena.begin();began=true;const fn=e[fact.raw];if(typeof fn!=="function")throw new Error("SEMAPRAX raw adapter missing");const status=fn(...raw,RESULT);if(status!==0){{for(let index=RESULT;index<RESULT+size;index++)if(bytes[index]!==POISON)throw new Error("SEMAPRAX failure modified result slot");if(status>=1&&status<=10)throw Object.assign(new Error(`SEMAPRAX semantic failure ${{status}}`),{{status,semapraxSemantic:true}});throw new Error(`SEMAPRAX call failed with status ${{status}}`)}}switch(fact.result){{case "i64":answer=view.getBigInt64(RESULT,true);break;case "usize":answer=view.getBigUint64(RESULT,true);break;case "bool":{{const value=view.getUint32(RESULT,true);if(value>1)throw new Error("SEMAPRAX bool result invariant");answer=value===1;break}}case "owned-bytes":answer=linked.arena.consume(view.getBigInt64(RESULT,true));break;case "option-owned-bytes":{{const tag=view.getUint32(RESULT,true);if(tag>1)throw new Error("SEMAPRAX owned variant tag invariant");answer=tag===0?null:linked.arena.consume(view.getBigInt64(RESULT+8,true));break}}case "result-owned-bytes-i64":{{const tag=view.getUint32(RESULT,true);if(tag>1)throw new Error("SEMAPRAX owned variant tag invariant");answer=tag===0?Object.freeze({{ok:true,value:linked.arena.consume(view.getBigInt64(RESULT+8,true))}}):Object.freeze({{ok:false,error:view.getBigInt64(RESULT+8,true)}});break}}default:throw new Error("unknown descriptor result type")}}}}catch(error){{primary=error;if(!(error instanceof RangeError)&&!(error instanceof TypeError)&&error?.semapraxSemantic!==true){{poisoned=true;linked.arena.poison()}}}}finally{{let settlement=null;if(began)try{{linked.arena.settle()}}catch(error){{poisoned=true;settlement=error}}try{{bytes.fill(0,0,used);bytes.fill(POISON,RESULT,RESULT+size)}}catch(error){{poisoned=true;settlement??=error}}busy=false;if(settlement!==null&&primary===null)primary=settlement}}if(primary)throw primary;return answer}}const functions=Object.create(null);for(const id of IDS)Object.defineProperty(functions,id,{{value:(...values)=>invoke(id,values),enumerable:true}});return Object.freeze({{functions:Object.freeze(functions),call:(id,...values)=>invoke(id,values),wasmSha256}})}}
export async function instantiate(bytes){{return facade(await instantiateCore(bytes))}}export const exportIds=IDS;export default instantiate;
"#
    )
}

fn render_runtime_facade(exports: &[OwnedExport]) -> String {
    let facts = exports
        .iter()
        .map(|export| {
            format!(
                "[{},Object.freeze({{raw:{},params:Object.freeze([{}])}})]",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|p| quote_json(parameter_wire(*p)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"const FACTS=new Map([{facts}]),IDS=Object.freeze(Array.from(FACTS.keys())),RESULT=65536,POISON=0xa5;
const encoder=new TextEncoder();
function snapshot(value,type,label){{if(type==="borrow-str"){{if(typeof value!=="string")throw new TypeError(`${{label}} must be a string`);for(let i=0;i<value.length;i++){{const unit=value.charCodeAt(i);if(unit>=0xd800&&unit<=0xdbff){{if(++i>=value.length||value.charCodeAt(i)<0xdc00||value.charCodeAt(i)>0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}else if(unit>=0xdc00&&unit<=0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}return encoder.encode(value)}}if(type==="borrow-slice-u8")return snapshotUint8(value,label);if(type==="i64"){{if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError(`${{label}} must be signed i64 bigint`);return value}}if(type==="bool"){{if(typeof value!=="boolean")throw new TypeError(`${{label}} must be boolean`);return value?1:0}}throw new Error("unknown descriptor parameter type")}}
function facade(linked){{const e=linked.instance.exports,memory=e.memory;if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength!==131072)throw new Error("SEMAPRAX fixed owned-data memory invariant");let busy=false,poisoned=false;function invoke(id,values){{if(poisoned)throw new Error("SEMAPRAX owned-data runtime is poisoned");if(busy)throw new Error("SEMAPRAX owned-data call is non-reentrant");const fact=FACTS.get(id);if(!fact)throw new RangeError(`unknown SEMAPRAX export: ${{id}}`);if(values.length!==fact.params.length)throw new TypeError("SEMAPRAX argument count disagrees");const snapshots=values.map((v,i)=>snapshot(v,fact.params[i],`argument ${{i}}`));let used=0;for(const v of snapshots)if(v instanceof Uint8Array){{if(v.byteLength>65536-used)throw new RangeError("SEMAPRAX borrowed input capacity exceeded");used+=v.byteLength}}busy=true;let began=false,answer,primary=null;const bytes=new Uint8Array(memory.buffer),view=new DataView(memory.buffer);try{{let offset=0;const raw=[];for(const v of snapshots){{if(v instanceof Uint8Array){{linked.copyInto(bytes,v,offset);raw.push(offset,v.byteLength);offset+=v.byteLength}}else raw.push(v)}}bytes.fill(POISON,RESULT,RESULT+8);linked.arena.begin();began=true;const fn=e[fact.raw];if(typeof fn!=="function")throw new Error("SEMAPRAX raw adapter missing");const status=fn(...raw,RESULT);if(status!==0){{for(let i=RESULT;i<RESULT+8;i++)if(bytes[i]!==POISON)throw new Error("SEMAPRAX failure modified result slot");if(status>=1&&status<=10)throw Object.assign(new Error(`SEMAPRAX semantic failure ${{status}}`),{{status,semapraxSemantic:true}});throw new Error(`SEMAPRAX call failed with status ${{status}}`)}}const carrier=view.getBigInt64(RESULT,true);answer=linked.arena.consume(carrier)}}catch(error){{primary=error;if(!(error instanceof RangeError)&&!(error instanceof TypeError)&&error?.semapraxSemantic!==true){{poisoned=true;linked.arena.poison()}}}}finally{{let settlement=null;if(began)try{{linked.arena.settle()}}catch(error){{poisoned=true;settlement=error}}try{{bytes.fill(0,0,used);bytes.fill(POISON,RESULT,RESULT+8)}}catch(error){{poisoned=true;settlement??=error}}busy=false;if(settlement!==null&&primary===null)primary=settlement}}if(primary)throw primary;return answer}}const functions=Object.create(null);for(const id of IDS)Object.defineProperty(functions,id,{{value:(...v)=>invoke(id,v),enumerable:true}});return Object.freeze({{functions:Object.freeze(functions),call:(id,...v)=>invoke(id,v),wasmSha256}})}}
export async function instantiate(bytes){{return facade(await instantiateCore(bytes))}}export const exportIds=IDS;export default instantiate;
"#
    )
}

fn render_variant_runtime_facade(exports: &[OwnedExport]) -> String {
    let facts = exports
        .iter()
        .map(|export| {
            format!(
                "[{},Object.freeze({{raw:{},params:Object.freeze([{}]),result:{}}})]",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|p| quote_json(parameter_wire(*p)))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.wire_name()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"const FACTS=new Map([{facts}]),IDS=Object.freeze(Array.from(FACTS.keys())),RESULT=65536,POISON=0xa5;
const encoder=new TextEncoder();
function snapshot(value,type,label){{if(type==="borrow-str"){{if(typeof value!=="string")throw new TypeError(`${{label}} must be a string`);for(let i=0;i<value.length;i++){{const unit=value.charCodeAt(i);if(unit>=0xd800&&unit<=0xdbff){{if(++i>=value.length||value.charCodeAt(i)<0xdc00||value.charCodeAt(i)>0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}else if(unit>=0xdc00&&unit<=0xdfff)throw new TypeError(`${{label}} must contain Unicode scalar values`)}}return encoder.encode(value)}}if(type==="borrow-slice-u8")return snapshotUint8(value,label);if(type==="i64"){{if(typeof value!=="bigint"||value<-(1n<<63n)||value>(1n<<63n)-1n)throw new TypeError(`${{label}} must be signed i64 bigint`);return value}}if(type==="bool"){{if(typeof value!=="boolean")throw new TypeError(`${{label}} must be boolean`);return value?1:0}}throw new Error("unknown descriptor parameter type")}}
function facade(linked){{const e=linked.instance.exports,memory=e.memory;if(!(memory instanceof WebAssembly.Memory)||memory.buffer.byteLength!==131072)throw new Error("SEMAPRAX fixed owned-data memory invariant");let busy=false,poisoned=false;function invoke(id,values){{if(poisoned)throw new Error("SEMAPRAX owned-data runtime is poisoned");if(busy)throw new Error("SEMAPRAX owned-data call is non-reentrant");const fact=FACTS.get(id);if(!fact)throw new RangeError(`unknown SEMAPRAX export: ${{id}}`);if(values.length!==fact.params.length)throw new TypeError("SEMAPRAX argument count disagrees");const snapshots=values.map((v,i)=>snapshot(v,fact.params[i],`argument ${{i}}`));let used=0;for(const v of snapshots)if(v instanceof Uint8Array){{if(v.byteLength>65536-used)throw new RangeError("SEMAPRAX borrowed input capacity exceeded");used+=v.byteLength}}busy=true;let began=false,answer,primary=null;const bytes=new Uint8Array(memory.buffer),view=new DataView(memory.buffer),resultSize=fact.result==="owned-bytes"?8:16;try{{let offset=0;const raw=[];for(const v of snapshots){{if(v instanceof Uint8Array){{linked.copyInto(bytes,v,offset);raw.push(offset,v.byteLength);offset+=v.byteLength}}else raw.push(v)}}bytes.fill(POISON,RESULT,RESULT+resultSize);linked.arena.begin();began=true;const fn=e[fact.raw];if(typeof fn!=="function")throw new Error("SEMAPRAX raw adapter missing");const status=fn(...raw,RESULT);if(status!==0){{for(let i=RESULT;i<RESULT+resultSize;i++)if(bytes[i]!==POISON)throw new Error("SEMAPRAX failure modified result slot");if(status>=1&&status<=10)throw Object.assign(new Error(`SEMAPRAX semantic failure ${{status}}`),{{status,semapraxSemantic:true}});throw new Error(`SEMAPRAX call failed with status ${{status}}`)}}if(fact.result==="owned-bytes")answer=linked.arena.consume(view.getBigInt64(RESULT,true));else{{const tag=view.getUint32(RESULT,true);if(tag>1)throw new Error("SEMAPRAX owned variant tag invariant");if(fact.result==="option-owned-bytes")answer=tag===0?null:linked.arena.consume(view.getBigInt64(RESULT+8,true));else answer=tag===0?Object.freeze({{ok:true,value:linked.arena.consume(view.getBigInt64(RESULT+8,true))}}):Object.freeze({{ok:false,error:view.getBigInt64(RESULT+8,true)}})}}}}catch(error){{primary=error;if(!(error instanceof RangeError)&&!(error instanceof TypeError)&&error?.semapraxSemantic!==true){{poisoned=true;linked.arena.poison()}}}}finally{{let settlement=null;if(began)try{{linked.arena.settle()}}catch(error){{poisoned=true;settlement=error}}try{{bytes.fill(0,0,used);bytes.fill(POISON,RESULT,RESULT+resultSize)}}catch(error){{poisoned=true;settlement??=error}}busy=false;if(settlement!==null&&primary===null)primary=settlement}}if(primary)throw primary;return answer}}const functions=Object.create(null);for(const id of IDS)Object.defineProperty(functions,id,{{value:(...v)=>invoke(id,v),enumerable:true}});return Object.freeze({{functions:Object.freeze(functions),call:(id,...v)=>invoke(id,v),wasmSha256}})}}
export async function instantiate(bytes){{return facade(await instantiateCore(bytes))}}export const exportIds=IDS;export default instantiate;
"#
    )
}

fn render_bindings(_exports: &[OwnedExport], _wasm_sha256: &str) -> String {
    "export { instantiate, exportIds, wasmSha256, default } from \"./semaprax.js\";\n".to_owned()
}

fn render_declarations(exports: &[OwnedExport]) -> String {
    let has_variants = exports.iter().any(|export| {
        matches!(
            export.result,
            PublicApiResultType::OptionOwnedBytes | PublicApiResultType::ResultOwnedBytesI64
        )
    });
    let rows = exports
        .iter()
        .map(|export| {
            let params = export
                .parameters
                .iter()
                .enumerate()
                .map(|(i, p)| format!("arg{i}: {}", parameter_ts(*p)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "  readonly {}: ({params}) => {};",
                quote_json(&export.stable_id),
                result_ts(export.result)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let aliases = if has_variants {
        "export type OptionalBytes = Uint8Array | null;\nexport type SemapraxResult<T, E> =\n  | { readonly ok: true; readonly value: T }\n  | { readonly ok: false; readonly error: E };\n"
    } else {
        ""
    };
    format!("{aliases}export interface SemapraxApi {{\n{rows}\n}}\nexport interface SemapraxRuntime {{ readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id: I, ...args: Parameters<SemapraxApi[I]>): ReturnType<SemapraxApi[I]>; readonly wasmSha256: string; }}\nexport declare function instantiate(bytes: Uint8Array): Promise<SemapraxRuntime>;\nexport declare const exportIds: readonly (keyof SemapraxApi)[];\nexport default instantiate;\n")
}

fn render_metadata(
    package: &str,
    version: &str,
    descriptor: &PublicApiDescriptor,
    wasm_sha256: &str,
    exports: &[OwnedExport],
) -> String {
    let shapes = exports.iter().map(|e| format!("{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{},\"call\":\"(parameters..., result_out: i32) -> status: i32\"}}", quote_json(&e.stable_id), quote_json(&e.wasm_export), e.parameters.iter().map(|p| quote_json(parameter_wire(*p))).collect::<Vec<_>>().join(","), quote_json(e.result.wire_name()))).collect::<Vec<_>>().join(",");
    format!("{{\"schema\":\"{OWNED_DATA_API_SCHEMA}\",\"package\":{},\"version\":{},\"descriptor\":{},\"descriptor_digest\":{},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}},\"limits\":{{\"borrowed_input_bytes\":65536,\"owned_output_bytes\":65536}},\"target\":[{shapes}],\"settlement\":{{\"copy_before_consume\":true,\"consume_exactly_once\":true,\"require_empty_arena\":true,\"poison_result_memory\":true}},\"artifacts\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\",\"package.json\"]}}\n", quote_json(package), quote_json(version), quote_json(&String::from_utf8(descriptor.canonical_bytes()).expect("descriptor is UTF-8")), quote_json(&descriptor.digest()), quote_json(wasm_sha256))
}

fn render_package_json(package: &str, version: &str) -> String {
    format!("{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.api.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\"]}}\n", quote_json(package), quote_json(version))
}
fn parameter_wire(value: PublicApiParameterType) -> &'static str {
    value.wire_name()
}
fn parameter_ts(value: PublicApiParameterType) -> &'static str {
    match value {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}
fn result_ts(value: PublicApiResultType) -> &'static str {
    match value {
        PublicApiResultType::I64 | PublicApiResultType::Usize => "bigint",
        PublicApiResultType::Bool => "boolean",
        PublicApiResultType::OwnedBytes => "Uint8Array",
        PublicApiResultType::OptionOwnedBytes => "OptionalBytes",
        PublicApiResultType::ResultOwnedBytesI64 => "SemapraxResult<Uint8Array, bigint>",
    }
}
fn raw_symbol(stable_id: &str) -> String {
    let mut s = String::from("spx_owned_v1_");
    for b in stable_id.bytes() {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}
fn descriptor_subject(descriptor: &PublicApiDescriptor) -> PublicApiSubject<'_> {
    PublicApiSubject {
        project_schema: descriptor.project_schema(),
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    }
}
pub(super) fn validate_identity(package: &str, version: &str) -> Result<(), Diagnostic> {
    if package.is_empty()
        || package.len() > 214
        || !package.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'-' | b'_' | b'.' | b'@' | b'/')
        })
        || version.is_empty()
        || version.len() > 128
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
    {
        return Err(package_error(
            "owned-data package identity is not canonical",
        ));
    }
    Ok(())
}
fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|a| a.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error("owned-data artifact is absent"))
}

#[cfg(test)]
mod hostile_source_tests {
    use super::*;

    #[test]
    fn every_owned_facade_rejects_non_scalar_strings_and_wide_i64_before_effects() {
        let sources = [
            render_mixed_runtime_facade(&[]),
            render_runtime_facade(&[]),
            render_variant_runtime_facade(&[]),
        ];
        for source in sources {
            let snapshot = source.find("const snapshots=").unwrap();
            assert!(source[..snapshot].contains("unit>=0xd800&&unit<=0xdbff"));
            assert!(source[..snapshot].contains("unit>=0xdc00&&unit<=0xdfff"));
            assert!(source[..snapshot].contains("++i>=value.length"));
            assert!(source[..snapshot].contains("return encoder.encode(value)"));
            assert!(source[..snapshot].contains("value<-(1n<<63n)"));
            assert!(source[..snapshot].contains("value>(1n<<63n)-1n"));
            assert!(snapshot < source.find("busy=true").unwrap());
            assert!(snapshot < source.find("arena.begin()").unwrap());
        }
    }
}
