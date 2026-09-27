use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedType};
use crate::resumable_effects::lowering::ResumableScalar;

use super::{backend_failure, normalized_status, scalar_matches_type, TargetOutcome};

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn execute(
    program: &ResolvedProgram,
    function_id: &str,
    arguments: &[ResumableScalar],
    result_type: &ResolvedType,
) -> Result<TargetOutcome, Diagnostic> {
    if matches!(result_type, ResolvedType::Usize)
        || arguments
            .iter()
            .any(|value| matches!(value, ResumableScalar::Usize(_)))
    {
        return Err(Diagnostic::io(
            "SPX-W115",
            "resumable Core Wasm parity evidence does not admit `usize`; the Public Scalar Export Profile intentionally has no host-width carrier",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| failure("selected Core Wasm projection function is absent"))?;
    if function.params.len() != arguments.len()
        || function
            .params
            .iter()
            .zip(arguments)
            .any(|(parameter, value)| !scalar_matches_type(value, &parameter.ty))
        || function.return_type != *result_type
    {
        return Err(failure(
            "Core Wasm projection signature disagrees with its values",
        ));
    }

    let export_ids = vec![function_id.to_owned()];
    let prepared = crate::wasm::prepare_project_web_with_scalar_exports(
        program,
        "resumable-parity",
        "local",
        "local",
        "local",
        "resumable",
        &export_ids,
    )?;
    let root = ProbeRoot::create()?;
    let package = root.path.join("package");
    prepared.publish(&package)?;
    let script = root.path.join("probe.mjs");
    fs::write(&script, SCRIPT)
        .map_err(|error| failure(format!("cannot write Core Wasm probe: {error}")))?;

    let mut command = Command::new("node");
    command
        .arg(&script)
        .arg(&package)
        .arg(function_id)
        .arg(result_type_text(result_type)?);
    for argument in arguments {
        command.arg(argument_token(argument));
    }
    let output = command
        .output()
        .map_err(|error| failure(format!("cannot execute required node: {error}")))?;
    if !output.status.success() {
        return Err(failure(format!(
            "Core Wasm resumable probe failed with status {}: {}",
            output.status,
            bounded_stderr(&output.stderr),
        )));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| failure("Core Wasm resumable probe output is not UTF-8"))?;
    decode(stdout, result_type)
}

const SCRIPT: &str = r#"import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
const [root,id,resultType,...tokens]=process.argv.slice(2);
const bindings=await import(pathToFileURL(`${root}/semaprax.bindings.js`));
const runtime=await bindings.instantiateBytes(await readFile(`${root}/app.wasm`));
const hexBig=t=>BigInt(`0x${t}`);
function decode(token){
  const split=token.indexOf(":"),type=token.slice(0,split),hex=token.slice(split+1);
  if(type==="i64")return BigInt.asIntN(64,hexBig(hex));
  if(type==="i32")return Number(BigInt.asIntN(32,hexBig(hex)));
  if(type==="u8"||type==="char")return Number(hexBig(hex));
  if(type==="bool")return hex==="01";
  if(type==="f32"){const b=new ArrayBuffer(4),v=new DataView(b);v.setUint32(0,Number(hexBig(hex)),false);return v.getFloat32(0,false);}
  if(type==="f64"){const b=new ArrayBuffer(8),v=new DataView(b);v.setBigUint64(0,hexBig(hex),false);return v.getFloat64(0,false);}
  throw new Error(`unknown scalar token ${type}`);
}
function encode(value,type){
  if(type==="i64")return BigInt.asUintN(64,value).toString(16).padStart(16,"0");
  if(type==="i32")return BigInt.asUintN(32,BigInt(value)).toString(16).padStart(8,"0");
  if(type==="u8")return value.toString(16).padStart(2,"0");
  if(type==="char")return value.toString(16).padStart(8,"0");
  if(type==="bool")return value?"01":"00";
  if(type==="f32"){const b=new ArrayBuffer(4),v=new DataView(b);v.setFloat32(0,value,false);return v.getUint32(0,false).toString(16).padStart(8,"0");}
  if(type==="f64"){const b=new ArrayBuffer(8),v=new DataView(b);v.setFloat64(0,value,false);return v.getBigUint64(0,false).toString(16).padStart(16,"0");}
  throw new Error(`unknown result type ${type}`);
}
const outcome=runtime.call(id,...tokens.map(decode));
if(!outcome.ok)console.log(`STATUS ${JSON.stringify(outcome.status)}`);
else console.log(`OK ${encode(outcome.value,resultType)}`);
"#;

fn argument_token(value: &ResumableScalar) -> String {
    match value {
        ResumableScalar::I64(value) => format!("i64:{:016x}", *value as u64),
        ResumableScalar::I32(value) => format!("i32:{:08x}", *value as u32),
        ResumableScalar::U8(value) => format!("u8:{value:02x}"),
        ResumableScalar::Usize(value) => format!("usize:{value:016x}"),
        ResumableScalar::Char(value) => format!("char:{value:08x}"),
        ResumableScalar::F32(bits) => format!("f32:{bits:08x}"),
        ResumableScalar::F64(bits) => format!("f64:{bits:016x}"),
        ResumableScalar::Bool(value) => format!("bool:{}", if *value { "01" } else { "00" }),
        // Issue #296 R20: an aggregate `yields` channel is admitted only for
        // the direct top-level (sequential) placement, and this compiler-private
        // Wasm parity runner only ever executes a Copy-scalar plan's
        // yield-free projection -- native/Wasm still refuse every
        // `yields`-declaring function's own emission (`SPX-W126`), so an
        // aggregate value can never reach this encoder in practice.
        ResumableScalar::Record(_) | ResumableScalar::Variant { .. } => unreachable!(
            "the Wasm resumable-effect parity runner does not encode an aggregate channel value"
        ),
    }
}

fn result_type_text(ty: &ResolvedType) -> Result<&'static str, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok("i64"),
        ResolvedType::I32 => Ok("i32"),
        ResolvedType::U8 => Ok("u8"),
        ResolvedType::Char => Ok("char"),
        ResolvedType::F32 => Ok("f32"),
        ResolvedType::F64 => Ok("f64"),
        ResolvedType::Bool => Ok("bool"),
        ResolvedType::Usize => Err(Diagnostic::io(
            "SPX-W115",
            "resumable Core Wasm parity evidence does not admit `usize`",
        )),
        _ => Err(failure("Core Wasm projection result is not a Copy scalar")),
    }
}

fn decode(stdout: &str, ty: &ResolvedType) -> Result<TargetOutcome, Diagnostic> {
    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| failure("Core Wasm resumable probe produced no result"))?;
    if let Some(status) = line.strip_prefix("STATUS ") {
        let value: serde_json::Value = serde_json::from_str(status)
            .map_err(|_| failure("Core Wasm normalized status is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| failure("Core Wasm normalized status is not an object"))?;
        if object.get("schema").and_then(serde_json::Value::as_str)
            != Some(crate::conformance::NORMALIZED_STATUS_SCHEMA_V1)
        {
            return Err(failure("Core Wasm normalized status schema is invalid"));
        }
        let domain = object
            .get("domain_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| failure("Core Wasm normalized status domain is invalid"))?;
        let code = object
            .get("code")
            .and_then(serde_json::Value::as_u64)
            .and_then(|code| u32::try_from(code).ok())
            .ok_or_else(|| failure("Core Wasm normalized status code is invalid"))?;
        return Ok(TargetOutcome::LanguageFailure(normalized_status(
            domain, code,
        )?));
    }
    let hex = line
        .strip_prefix("OK ")
        .ok_or_else(|| failure("Core Wasm resumable probe result has an unknown shape"))?;
    let bits = u64::from_str_radix(hex, 16)
        .map_err(|_| failure("Core Wasm resumable result is not hexadecimal"))?;
    match ty {
        ResolvedType::I64 => Ok(TargetOutcome::Success(ResumableScalar::I64(bits as i64))),
        ResolvedType::I32 => Ok(TargetOutcome::Success(ResumableScalar::I32(
            bits as u32 as i32,
        ))),
        ResolvedType::U8 => Ok(TargetOutcome::Success(ResumableScalar::U8(bits as u8))),
        ResolvedType::Char => Ok(TargetOutcome::Success(ResumableScalar::Char(bits as u32))),
        ResolvedType::F32 => Ok(TargetOutcome::Success(ResumableScalar::F32(bits as u32))),
        ResolvedType::F64 => Ok(TargetOutcome::Success(ResumableScalar::F64(bits))),
        ResolvedType::Bool if bits <= 1 => {
            Ok(TargetOutcome::Success(ResumableScalar::Bool(bits == 1)))
        }
        ResolvedType::Bool => Err(failure("Core Wasm backend returned a non-canonical bool")),
        ResolvedType::Usize => Err(Diagnostic::io(
            "SPX-W115",
            "resumable Core Wasm parity evidence does not admit `usize`",
        )),
        _ => Err(failure(
            "Core Wasm backend returned an unsupported scalar type",
        )),
    }
}

fn bounded_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned()
}

struct ProbeRoot {
    path: PathBuf,
}

impl ProbeRoot {
    fn create() -> Result<Self, Diagnostic> {
        let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-resumable-wasm-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).map_err(|error| {
            failure(format!("cannot create Core Wasm probe directory: {error}"))
        })?;
        Ok(Self { path })
    }
}

impl Drop for ProbeRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn failure(message: impl Into<String>) -> Diagnostic {
    backend_failure(
        "SPX-F105",
        format!(
            "resumable Core Wasm parity evidence failed: {}",
            message.into()
        ),
    )
}
