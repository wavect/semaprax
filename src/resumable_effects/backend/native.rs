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
    optimization: &str,
) -> Result<TargetOutcome, Diagnostic> {
    if !matches!(optimization, "-O0" | "-O2") {
        return Err(failure("native optimization selector is not -O0 or -O2"));
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .ok_or_else(|| failure("selected native projection function is absent"))?;
    if function.params.len() != arguments.len()
        || function
            .params
            .iter()
            .zip(arguments)
            .any(|(parameter, value)| !scalar_matches_type(value, &parameter.ty))
        || function.return_type != *result_type
    {
        return Err(failure(
            "native projection signature disagrees with its values",
        ));
    }

    let generated = crate::codegen::emit_hir_c(program)?;
    let body = driver(function_id, arguments, result_type)?;
    let root = ProbeRoot::create("native")?;
    let source = root.path.join("probe.c");
    let executable = root
        .path
        .join(format!("probe{}", std::env::consts::EXE_SUFFIX));
    fs::write(
        &source,
        format!("{generated}\nint main(void) {{\n{body}}}\n"),
    )
    .map_err(|error| failure(format!("cannot write native probe: {error}")))?;

    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-tautological-compare",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .map_err(|error| failure(format!("cannot execute required clang: {error}")))?;
    if !compiled.status.success() {
        return Err(failure(format!(
            "clang rejected the native resumable projection: {}",
            bounded_stderr(&compiled.stderr)
        )));
    }
    let output = Command::new(&executable)
        .output()
        .map_err(|error| failure(format!("cannot execute native probe: {error}")))?;
    if !output.status.success() {
        return Err(failure(format!(
            "native resumable probe failed with status {}: {}",
            output.status,
            bounded_stderr(&output.stderr),
        )));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| failure("native resumable probe output is not UTF-8"))?;
    decode(stdout, result_type)
}

fn driver(
    function_id: &str,
    arguments: &[ResumableScalar],
    result_type: &ResolvedType,
) -> Result<String, Diagnostic> {
    let mut body = String::new();
    body.push_str("    struct spx_status_entry spx_status_entries[UINT32_C(32)];\n");
    body.push_str("    struct spx_context spx_ctx = {0};\n");
    body.push_str("    if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(32), NULL, NULL, NULL)) return 90;\n");
    let mut names = Vec::with_capacity(arguments.len());
    for (index, value) in arguments.iter().enumerate() {
        let name = format!("spx_arg_{index}");
        emit_value(&mut body, &name, value);
        names.push(name);
    }
    let result_c_type = c_type(result_type)?;
    body.push_str(&format!("    {result_c_type} spx_result = {{0}};\n"));
    body.push_str(&format!(
        "    spx_status_token spx_token = {}(&spx_ctx",
        symbol("spx_decl_", function_id)
    ));
    for name in names {
        body.push_str(", ");
        body.push_str(&name);
    }
    body.push_str(", &spx_result);\n");
    body.push_str("    if (spx_token != SPX_STATUS_SUCCESS) { const struct spx_normalized_status *spx_failure = spx_status_resolve(&spx_ctx, spx_token); if (!spx_failure) return 91; printf(\"STATUS %s %u\\n\", spx_failure->domain_id, (unsigned)spx_failure->code); return 0; }\n");
    emit_result(&mut body, result_type)?;
    body.push_str("    return 0;\n");
    Ok(body)
}

fn emit_value(output: &mut String, name: &str, value: &ResumableScalar) {
    match value {
        ResumableScalar::I64(value) => output.push_str(&format!(
            "    uint64_t {name}_bits = UINT64_C(0x{:016x}); int64_t {name}; memcpy(&{name}, &{name}_bits, sizeof({name}));\n",
            *value as u64
        )),
        ResumableScalar::I32(value) => output.push_str(&format!(
            "    uint32_t {name}_bits = UINT32_C(0x{:08x}); int32_t {name}; memcpy(&{name}, &{name}_bits, sizeof({name}));\n",
            *value as u32
        )),
        ResumableScalar::U8(value) => output.push_str(&format!(
            "    uint8_t {name} = UINT8_C(0x{value:02x});\n"
        )),
        ResumableScalar::Usize(value) => output.push_str(&format!(
            "    uint64_t {name} = UINT64_C(0x{value:016x});\n"
        )),
        ResumableScalar::Char(value) => output.push_str(&format!(
            "    uint32_t {name} = UINT32_C(0x{value:08x});\n"
        )),
        ResumableScalar::F32(bits) => output.push_str(&format!(
            "    uint32_t {name}_bits = UINT32_C(0x{bits:08x}); float {name}; memcpy(&{name}, &{name}_bits, sizeof({name}));\n"
        )),
        ResumableScalar::F64(bits) => output.push_str(&format!(
            "    uint64_t {name}_bits = UINT64_C(0x{bits:016x}); double {name}; memcpy(&{name}, &{name}_bits, sizeof({name}));\n"
        )),
        ResumableScalar::Bool(value) => output.push_str(&format!(
            "    bool {name} = {};\n",
            if *value { "true" } else { "false" }
        )),
        // Issue #296 R20: an aggregate `yields` channel is admitted only for
        // the direct top-level (sequential) placement, and this
        // compiler-private native parity runner only ever executes a
        // Copy-scalar plan's yield-free projection -- native/Wasm still
        // refuse every `yields`-declaring function's own emission
        // (`SPX-B116`), so an aggregate value can never reach this encoder
        // in practice.
        ResumableScalar::Record(_) | ResumableScalar::Variant { .. } => unreachable!(
            "the native resumable-effect parity runner does not encode an aggregate channel value"
        ),
    }
}

fn emit_result(output: &mut String, ty: &ResolvedType) -> Result<(), Diagnostic> {
    let (bits_type, width, format) = match ty {
        ResolvedType::I64 | ResolvedType::Usize | ResolvedType::F64 => ("uint64_t", 64, "%016llx"),
        ResolvedType::I32 | ResolvedType::Char | ResolvedType::F32 => ("uint32_t", 32, "%08x"),
        ResolvedType::U8 | ResolvedType::Bool => ("uint8_t", 8, "%02x"),
        _ => return Err(failure("native projection result is not a Copy scalar")),
    };
    output.push_str(&format!(
        "    {bits_type} spx_result_bits = 0; memcpy(&spx_result_bits, &spx_result, sizeof(spx_result));\n"
    ));
    match width {
        64 => output.push_str(&format!(
            "    printf(\"OK {format}\\n\", (unsigned long long)spx_result_bits);\n"
        )),
        _ => output.push_str(&format!(
            "    printf(\"OK {format}\\n\", (unsigned)spx_result_bits);\n"
        )),
    }
    Ok(())
}

fn decode(stdout: &str, ty: &ResolvedType) -> Result<TargetOutcome, Diagnostic> {
    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| failure("native resumable probe produced no result"))?;
    if let Some(status) = line.strip_prefix("STATUS ") {
        let (domain, code) = status
            .split_once(' ')
            .ok_or_else(|| failure("native normalized status has an unknown shape"))?;
        let code = code
            .parse::<u32>()
            .map_err(|_| failure("native normalized status code is not u32"))?;
        return Ok(TargetOutcome::LanguageFailure(normalized_status(
            domain, code,
        )?));
    }
    let hex = line
        .strip_prefix("OK ")
        .ok_or_else(|| failure("native resumable probe result has an unknown shape"))?;
    let bits = u64::from_str_radix(hex, 16)
        .map_err(|_| failure("native resumable probe result is not hexadecimal"))?;
    match ty {
        ResolvedType::I64 => Ok(TargetOutcome::Success(ResumableScalar::I64(bits as i64))),
        ResolvedType::I32 => Ok(TargetOutcome::Success(ResumableScalar::I32(
            bits as u32 as i32,
        ))),
        ResolvedType::U8 => Ok(TargetOutcome::Success(ResumableScalar::U8(bits as u8))),
        ResolvedType::Usize => Ok(TargetOutcome::Success(ResumableScalar::Usize(bits))),
        ResolvedType::Char => Ok(TargetOutcome::Success(ResumableScalar::Char(bits as u32))),
        ResolvedType::F32 => Ok(TargetOutcome::Success(ResumableScalar::F32(bits as u32))),
        ResolvedType::F64 => Ok(TargetOutcome::Success(ResumableScalar::F64(bits))),
        ResolvedType::Bool if bits <= 1 => {
            Ok(TargetOutcome::Success(ResumableScalar::Bool(bits == 1)))
        }
        ResolvedType::Bool => Err(failure("native backend returned a non-canonical bool")),
        _ => Err(failure(
            "native backend returned an unsupported scalar type",
        )),
    }
}

fn c_type(ty: &ResolvedType) -> Result<&'static str, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok("int64_t"),
        ResolvedType::I32 => Ok("int32_t"),
        ResolvedType::U8 => Ok("uint8_t"),
        ResolvedType::Usize => Ok("uint64_t"),
        ResolvedType::Char => Ok("uint32_t"),
        ResolvedType::F32 => Ok("float"),
        ResolvedType::F64 => Ok("double"),
        ResolvedType::Bool => Ok("bool"),
        _ => Err(failure("native projection type is not a Copy scalar")),
    }
}

fn symbol(prefix: &str, id: &str) -> String {
    let mut symbol = String::from(prefix);
    for byte in id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn bounded_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).into_owned()
}

struct ProbeRoot {
    path: PathBuf,
}

impl ProbeRoot {
    fn create(lane: &str) -> Result<Self, Diagnostic> {
        let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-resumable-{lane}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path)
            .map_err(|error| failure(format!("cannot create native probe directory: {error}")))?;
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
            "resumable native parity evidence failed: {}",
            message.into()
        ),
    )
}
