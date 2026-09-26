//! `NativeStageExecutor`: the native C11 leg of the sealed [`super::StageExecutor`]
//! seam (#142).
//!
//! This executor never invents a second code-generation path. It compiles the
//! *entire* module through [`crate::codegen::emit_hir_c`] -- the exact,
//! already-proven-equivalent general native backend
//! `tests/agent_runtime_v1/stage_backend_parity.rs` exercises -- and then
//! appends one small, hand-written `main` that calls the bound stage's own
//! `spx_decl_<hex>` symbol directly, exactly the calling convention that
//! parity test's `native_probe` already establishes. It performs no codegen
//! of its own: every semantic byte of the stage body comes from
//! `emit_hir_c`.
//!
//! ## Scope, stated precisely
//!
//! The only values this executor can marshal across the C boundary are the
//! closed vocabulary [`super::super::stages`] already restricts Agent stage
//! signatures to: `i32`/`i64`/`bool`/`u8`/`usize` scalars, one level of
//! `own`/`borrow` record arguments built only from `Bytes`/`i64`/`bool` leaves
//! (`Task`/`State`/`Observation`/`Outcome`), and record or two-/four-case
//! variant results whose leaves are `Bytes`, `i64`, `bool`, or `u8`, plus record
//! results that may additionally carry `usize` (`Decision`/`Report`/`Step`).
//! Record
//! and variant field/case C member names are recomputed here from each
//! field's persistent [`DeclarationId`] using the exact hex-encoding
//! `src/codegen/native_emit/symbols.rs` uses (`spx_record_<hex>`,
//! `spx_field_<hex>`, ...); field *order* and byte-leaf placement are never
//! guessed -- both come straight from
//! [`crate::aggregate_layout::AggregateLayout`] /
//! [`crate::variant_layout::VariantLayout`], the same `pub(crate)` layout
//! authority the native backend itself consumes, so this module can never
//! silently disagree with codegen about which field lives where.
//!
//! A call outside this vocabulary (a nested record, a resource, a fifth
//! variant case, a contract failure reported through the native status
//! arena, ...) is refused with a diagnostic rather than guessed at. In
//! particular, this executor does not yet decode a native contract-failure
//! status into [`RetainedCallOutcome::LanguageFailure`]; it reports a
//! diagnostic instead. Every Agent stage body exercised by the bound
//! `stages.rs` vocabulary today is a plain deterministic function with no
//! declared effect, and the fixture this module's own tests share with
//! `stage_backend_parity.rs` carries no `requires`/`ensures` -- so this gap
//! is a scoped, honestly-reported one, not a silently-passing one.
//!
//! The wrapper settles its borrowed argument copies and returned Bytes before
//! accepting an exact post-finalizer receipt. `cleanup_events` describes only
//! successful result copy-out (as on the interpreter), not body finalizers,
//! instruction fuel, or cleanup after process termination/cancellation.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::process_provider::registered::{HeldProcessTool, RegisteredProcessProvider};
use crate::process_provider::ProcessTermination;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use crate::process_provider::{ProcessFailure, ProcessProvider, ProcessRequest};

use crate::aggregate_layout::{AggregateLayout, AggregateTarget};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, OwnershipMode, ResolvedType};
use crate::interpreter::retained_call::{
    PreparedRetainedCall, RetainedCallEvaluation, RetainedCallOutcome, RetainedField,
    RetainedRecord, RetainedValue, RetainedVariant,
};
use crate::interpreter::OwnedDataCleanupEvent;
use crate::variant_layout::{VariantLayout, VariantTarget};

use crate::agent_lifecycle::stages::invariant;

use super::{sealed, ExecutionAuthority, StageExecutor};

/// Explicit authority to use one trusted native C compiler for one local
/// stage-execution route.
///
/// The caller supplies an already-selected absolute compiler path. Opening it
/// turns that choice into a held-file capability; native execution never
/// searches `PATH`, inherits a host environment, or treats a compiler name as
/// authority. The held file is handed to the registered-process provider,
/// which executes the descriptor rather than resolving the path again.
///
/// This representation remains crate-private. The public target route exposes
/// it only through `iterative::effects::NativeTargetHost`, preserving the same
/// bounded held-descriptor substrate without exposing compiler argv or process
/// control as a public capability.
#[derive(Debug)]
pub struct NativeStageHost {
    compiler: File,
    #[cfg(test)]
    compiler_path: PathBuf,
    identity: String,
    compiler_digest: [u8; 32],
    compiler_len: u64,
    #[cfg(unix)]
    compiler_device: u64,
    #[cfg(unix)]
    compiler_inode: u64,
}

impl NativeStageHost {
    /// Holds one caller-selected native compiler. The supplied path must be
    /// absolute; it is resolved once before the file is held.
    pub(in crate::agent_lifecycle) fn open(compiler: &Path) -> Result<Self, Diagnostic> {
        if !compiler.is_absolute() {
            return Err(invariant("native_executor.host.compiler_path"));
        }
        let canonical = compiler
            .canonicalize()
            .map_err(|_| invariant("native_executor.host.compiler_path"))?;
        let held = OpenOptions::new()
            .read(true)
            .open(&canonical)
            .map_err(|_| invariant("native_executor.host.compiler_open"))?;
        let metadata = held
            .metadata()
            .map_err(|_| invariant("native_executor.host.compiler_metadata"))?;
        if !metadata.is_file() {
            return Err(invariant("native_executor.host.compiler_regular"));
        }
        let compiler_digest = digest_held_file(&held)?;
        let compiler_len = metadata.len();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(invariant("native_executor.host.compiler_executable"));
            }
        }
        let compiler_hex = compiler_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let identity = format!("native-c11:sha256:{compiler_hex}:{compiler_len}");
        Ok(Self {
            compiler: held,
            #[cfg(test)]
            compiler_path: canonical,
            identity,
            compiler_digest,
            compiler_len,
            #[cfg(unix)]
            compiler_device: {
                use std::os::unix::fs::MetadataExt;
                metadata.dev()
            },
            #[cfg(unix)]
            compiler_inode: {
                use std::os::unix::fs::MetadataExt;
                metadata.ino()
            },
        })
    }

    /// Non-authorizing identity that binds parity target evidence to the held
    /// compiler selection. It cannot be spent as process authority.
    pub(in crate::agent_lifecycle) fn identity(&self) -> &str {
        &self.identity
    }

    /// The canonical path originally used to establish this held capability.
    /// This is diagnostic/test plumbing only; execution always uses `compiler`.
    #[cfg(test)]
    pub(in crate::agent_lifecycle) fn compiler_path(&self) -> &Path {
        &self.compiler_path
    }

    fn compile(
        &self,
        directory: &ProbeDirectory,
        optimization: &str,
        cancellation: Option<&crate::agent_runtime::AgentCancellation>,
    ) -> Result<(), Diagnostic> {
        self.recheck_compiler()?;
        directory.recheck()?;
        let compiler = self
            .compiler
            .try_clone()
            .map_err(|_| invariant("native_executor.host.compiler_clone"))?;
        let mut arguments = vec![
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-tautological-compare",
            "-DSPX_NO_ENTRY_WRAPPER",
            "native_executor.c",
            "-o",
            "native_executor",
        ];
        // The process provider supplies no PATH. On Linux, Clang must name
        // the fixed system linker explicitly, as the native interop builder
        // already does; otherwise its driver fails before testing the stage.
        #[cfg(target_os = "linux")]
        arguments.push("--ld-path=/usr/bin/ld");
        let output = run_held(
            compiler,
            directory
                .held
                .try_clone()
                .map_err(|_| invariant("native_executor.probe_directory"))?,
            b"semaprax-stage-clang",
            &arguments,
            15_000,
            4 * 1024,
            60 * 1024 - 32,
            cancellation,
        )?;
        match output.termination {
            ProcessTermination::Exited(0) => Ok(()),
            _ => {
                #[cfg(test)]
                eprintln!(
                    "native stage compiler termination={:?} stderr={}",
                    output.termination,
                    String::from_utf8_lossy(&output.stderr)
                );
                Err(invariant("native_executor.compile"))
            }
        }
    }

    fn run(
        &self,
        directory: &ProbeDirectory,
        cancellation: Option<&crate::agent_runtime::AgentCancellation>,
    ) -> Result<Vec<u8>, Diagnostic> {
        directory.recheck()?;
        let executable = directory.open_child(c"native_executor")?;
        let output = run_held(
            executable,
            directory
                .held
                .try_clone()
                .map_err(|_| invariant("native_executor.probe_directory"))?,
            b"semaprax-stage-program",
            &[],
            2_000,
            48 * 1024,
            16 * 1024 - 32,
            cancellation,
        )?;
        match output.termination {
            ProcessTermination::Exited(0) => Ok(output.stdout),
            _ => Err(invariant("native_executor.run")),
        }
    }

    fn recheck_compiler(&self) -> Result<(), Diagnostic> {
        let metadata = self
            .compiler
            .metadata()
            .map_err(|_| invariant("native_executor.host.compiler_metadata"))?;
        if !metadata.is_file() || metadata.len() != self.compiler_len {
            return Err(invariant("native_executor.host.compiler_drift"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != self.compiler_device || metadata.ino() != self.compiler_inode {
                return Err(invariant("native_executor.host.compiler_drift"));
            }
        }
        if digest_held_file(&self.compiler)? != self.compiler_digest {
            return Err(invariant("native_executor.host.compiler_drift"));
        }
        Ok(())
    }
}

// The Windows LLVM runner ships a larger clang executable than the Unix
// launchers. Both limits remain explicit, finite bounds on the held bytes.
#[cfg(windows)]
const MAX_HELD_COMPILER_BYTES: u64 = 256 * 1024 * 1024;
#[cfg(not(windows))]
const MAX_HELD_COMPILER_BYTES: u64 = 64 * 1024 * 1024;

fn digest_held_file(file: &File) -> Result<[u8; 32], Diagnostic> {
    use sha2::{Digest, Sha256};
    #[cfg(not(unix))]
    let mut reader = file
        .try_clone()
        .map_err(|_| invariant("native_executor.host.compiler_clone"))?;
    #[cfg(not(unix))]
    {
        use std::io::{Seek, SeekFrom};
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|_| invariant("native_executor.host.compiler_read"))?;
    }
    let mut hash = Sha256::new();
    let mut remaining = MAX_HELD_COMPILER_BYTES;
    let mut offset = 0_u64;
    let mut bytes = [0_u8; 16 * 1024];
    loop {
        #[cfg(unix)]
        let read = {
            use std::os::unix::fs::FileExt;
            file.read_at(&mut bytes, offset)
        };
        #[cfg(not(unix))]
        let read = reader.read(&mut bytes);
        let read = read.map_err(|_| invariant("native_executor.host.compiler_read"))?;
        if read == 0 {
            break;
        }
        let read =
            u64::try_from(read).map_err(|_| invariant("native_executor.host.compiler_read"))?;
        if read > remaining {
            return Err(invariant("native_executor.host.compiler_budget"));
        }
        remaining -= read;
        hash.update(&bytes[..read as usize]);
        offset = offset
            .checked_add(read)
            .ok_or_else(|| invariant("native_executor.host.compiler_budget"))?;
    }
    Ok(hash.finalize().into())
}

/// Encodes the process-provider's closed argv wire. This module owns every
/// byte it supplies: callers cannot smuggle compiler flags or a second
/// executable through this boundary.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn argv_wire(arguments: &[&str]) -> Result<Vec<u8>, Diagnostic> {
    let count = u32::try_from(arguments.len())
        .map_err(|_| invariant("native_executor.process.arguments"))?;
    let mut wire = Vec::with_capacity(4 + arguments.iter().map(|arg| arg.len() + 4).sum::<usize>());
    wire.extend_from_slice(&count.to_le_bytes());
    for argument in arguments {
        if argument.as_bytes().contains(&0) {
            return Err(invariant("native_executor.process.arguments"));
        }
        let length = u32::try_from(argument.len())
            .map_err(|_| invariant("native_executor.process.arguments"))?;
        wire.extend_from_slice(&length.to_le_bytes());
        wire.extend_from_slice(argument.as_bytes());
    }
    Ok(wire)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn stage_arguments(_: &[Vec<u8>]) -> bool {
    // `run_held` is private and builds the entire argv wire above. The
    // registered provider still checks this predicate before its held-fd
    // launch, so no external caller can select a command through this tool.
    true
}

/// Runs one held executable through the repository's process-provider
/// boundary. That boundary owns process-group cleanup, finite stdout/stderr,
/// and the deadline; this executor never uses `Command`, `.output()`, PATH,
/// or an inherited environment.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_held(
    executable: File,
    directory: File,
    argv0: &[u8],
    arguments: &[&str],
    timeout_ms: u64,
    stdout_max: usize,
    stderr_max: usize,
    cancellation: Option<&crate::agent_runtime::AgentCancellation>,
) -> Result<crate::process_provider::ProcessOutput, Diagnostic> {
    let tool = HeldProcessTool::new(
        executable,
        directory,
        argv0.to_vec(),
        Vec::new(),
        stage_arguments,
    )
    .map_err(|_| invariant("native_executor.process.tool"))?;
    let mut provider = RegisteredProcessProvider::new([(1_u64, tool)])
        .map_err(|_| invariant("native_executor.process.tool"))?;
    let argv = argv_wire(arguments)?;
    let request = ProcessRequest::from_wire(
        1,
        &argv,
        argv.len(),
        &[],
        0,
        timeout_ms,
        stdout_max,
        stderr_max,
    )
    .map_err(|_| invariant("native_executor.process.request"))?;
    let result = provider.run_cancellable(&request, cancellation);
    let settled = provider.settle();
    match (result, settled) {
        (Ok(output), Ok(())) => Ok(output),
        (Err(ProcessFailure::TimedOut), _) => Err(invariant("native_executor.process.deadline")),
        (Err(ProcessFailure::CapacityExceeded), _) => {
            Err(invariant("native_executor.process.output_budget"))
        }
        (Err(ProcessFailure::Cancelled), _) => Err(invariant("native_executor.process.cancelled")),
        (Err(_), _) | (_, Err(_)) => Err(invariant("native_executor.process.run")),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn run_held(
    _executable: File,
    _directory: File,
    _argv0: &[u8],
    _arguments: &[&str],
    _timeout_ms: u64,
    _stdout_max: usize,
    _stderr_max: usize,
    _cancellation: Option<&crate::agent_runtime::AgentCancellation>,
) -> Result<crate::process_provider::ProcessOutput, Diagnostic> {
    Err(invariant("native_executor.host.unsupported"))
}

/// The native C11 executor, parameterized by the `clang` optimization flag
/// its one compile step uses.
///
/// [`super::StageBackend::Native`] always selects [`Self::o0`] -- production
/// dispatch behavior is unchanged by this field's addition. A second
/// dispatch route, [`super::StageBackend::NativeAtOptimization`], exists
/// solely so `tests.rs`'s cross-engine parity evidence can additionally
/// compile and run the exact same stage body at `-O2`: an optimizer is
/// exactly where backend divergence hides, and `-O0` alone never exercises
/// it. Both construct this same, single `StageExecutor` implementation --
/// the optimization level is data on an existing seam, not a fourth sealed
/// executor.
pub(in crate::agent_lifecycle) struct NativeStageExecutor<'a> {
    pub(in crate::agent_lifecycle) host: &'a NativeStageHost,
    pub(in crate::agent_lifecycle) optimization: &'static str,
}

impl<'a> NativeStageExecutor<'a> {
    pub(in crate::agent_lifecycle) const fn o0(host: &'a NativeStageHost) -> Self {
        Self {
            host,
            optimization: "-O0",
        }
    }
}

impl sealed::Sealed for NativeStageExecutor<'_> {}

impl StageExecutor for NativeStageExecutor<'_> {
    fn execute(
        &self,
        _authority: ExecutionAuthority,
        program: &hir::ResolvedProgram,
        prepared: &PreparedRetainedCall,
        arguments: &[RetainedValue],
        max_steps: usize,
        cancellation: Option<&crate::agent_runtime::AgentCancellation>,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        if cancellation.is_some_and(crate::agent_runtime::AgentCancellation::is_cancelled) {
            return Err(vec![super::super::stages::invariant(
                "stage_executor.cancelled",
            )]);
        }
        run(
            program,
            prepared,
            arguments,
            max_steps,
            self.host,
            self.optimization,
            cancellation,
        )
        .map_err(|error| vec![error])
    }
}

fn hex_symbol(prefix: &str, id: &DeclarationId) -> String {
    let mut symbol = String::from(prefix);
    for byte in id.as_str().bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn function_symbol(id: &DeclarationId) -> String {
    hex_symbol("spx_decl_", id)
}

fn record_symbol(id: &DeclarationId) -> String {
    hex_symbol("spx_record_", id)
}

fn variant_symbol(id: &DeclarationId) -> String {
    hex_symbol("spx_variant_", id)
}

fn case_symbol(id: &DeclarationId) -> String {
    hex_symbol("spx_case_", id)
}

fn field_symbol(id: &DeclarationId) -> String {
    hex_symbol("spx_field_", id)
}

/// The hex payload alone (no prefix), used both to print a field/case
/// identity from C and to decode it back on the Rust side without needing a
/// second, independent naming scheme.
fn hex_payload(id: &DeclarationId) -> String {
    let mut out = String::new();
    for byte in id.as_str().bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn decode_hex_payload(hex: &str) -> Result<DeclarationId, Diagnostic> {
    if hex.is_empty() || hex.len() % 2 != 0 {
        return Err(invariant("native_executor.decode.hex"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let chars = hex.as_bytes();
    let mut index = 0;
    while index < chars.len() {
        let byte = u8::from_str_radix(&hex[index..index + 2], 16)
            .map_err(|_| invariant("native_executor.decode.hex"))?;
        bytes.push(byte);
        index += 2;
    }
    let text = String::from_utf8(bytes).map_err(|_| invariant("native_executor.decode.hex"))?;
    Ok(DeclarationId::new(text))
}

fn c_i64(value: i64) -> String {
    if value == i64::MIN {
        // `INT64_C(-9223372036854775808)` asks the C preprocessor to form a
        // positive magnitude it cannot represent. Keep the same portable
        // spelling the main native backend uses for the signed minimum.
        "(-INT64_C(9223372036854775807) - INT64_C(1))".to_owned()
    } else {
        format!("INT64_C({value})")
    }
}

fn c_i32(value: i32) -> String {
    if value == i32::MIN {
        // A positive `2147483648` cannot be named as an `int32_t` literal.
        "(-INT32_C(2147483647) - INT32_C(1))".to_owned()
    } else if value < 0 {
        format!("-INT32_C({})", value.unsigned_abs())
    } else {
        format!("INT32_C({value})")
    }
}

struct Emitter {
    body: String,
    byte_ordinal: usize,
    arg_ordinal: usize,
    borrowed_owners: Vec<String>,
}

impl Emitter {
    fn new() -> Self {
        Self {
            body: String::new(),
            byte_ordinal: 0,
            arg_ordinal: 0,
            borrowed_owners: Vec::new(),
        }
    }

    fn bytes_expr(&mut self, bytes: &[u8]) -> String {
        if bytes.is_empty() {
            return "spx_bytes_copy((spx_slice_u8_v1){ .ptr = NULL, .len = UINT64_C(0) })"
                .to_owned();
        }
        self.byte_ordinal += 1;
        let name = format!("spx_native_exec_bytes_{}", self.byte_ordinal);
        let literal = bytes
            .iter()
            .map(|byte| format!("UINT8_C(0x{byte:02x})"))
            .collect::<Vec<_>>()
            .join(", ");
        self.body.push_str(&format!(
            "    static const uint8_t {name}[] = {{ {literal} }};\n"
        ));
        format!(
            "spx_bytes_copy((spx_slice_u8_v1){{ .ptr = {name}, .len = UINT64_C({}) }})",
            bytes.len()
        )
    }

    /// Declares one local `struct spx_record_<hex> <var> = { ... };` from a
    /// [`RetainedRecord`], using the record's own [`AggregateLayout`] for
    /// field order and C member names -- never a re-derived guess.
    fn record_local(
        &mut self,
        layout: &AggregateLayout,
        record: &RetainedRecord,
        var: &str,
    ) -> Result<(), Diagnostic> {
        let mut inits = Vec::with_capacity(layout.fields.len());
        for field in &layout.fields {
            let value = record
                .fields
                .iter()
                .find(|item| item.field == field.field)
                .ok_or_else(|| invariant("native_executor.record.missing_field"))?;
            let expr = match (&field.ty, &value.value) {
                (ResolvedType::Bytes, RetainedValue::Bytes(bytes)) => self.bytes_expr(bytes),
                (ResolvedType::I64, RetainedValue::I64(scalar)) => c_i64(*scalar),
                (ResolvedType::Bool, RetainedValue::Bool(flag)) => {
                    (if *flag { "true" } else { "false" }).to_owned()
                }
                _ => return Err(invariant("native_executor.record.field_shape")),
            };
            inits.push(format!("        .{} = {expr},", field_symbol(&field.field)));
        }
        self.body.push_str(&format!(
            "    struct {} {var} = {{\n{}\n    }};\n",
            record_symbol(&layout.record),
            inits.join("\n")
        ));
        Ok(())
    }
}

/// One prepared argument: its C expression (or the name of the local it was
/// declared into) plus any borrowed-byte-path pointer parameters the
/// generated C ABI additionally requires for a borrowed aggregate.
struct PreparedArgument {
    primary: String,
    extra_borrow_pointers: Vec<String>,
}

fn prepare_argument(
    emitter: &mut Emitter,
    program: &hir::ResolvedProgram,
    ownership: OwnershipMode,
    ty: &ResolvedType,
    value: &RetainedValue,
) -> Result<PreparedArgument, Diagnostic> {
    match (ty, value) {
        (ResolvedType::I32, RetainedValue::I32(scalar)) => Ok(PreparedArgument {
            primary: c_i32(*scalar),
            extra_borrow_pointers: Vec::new(),
        }),
        (ResolvedType::I64, RetainedValue::I64(scalar)) => Ok(PreparedArgument {
            primary: c_i64(*scalar),
            extra_borrow_pointers: Vec::new(),
        }),
        (ResolvedType::Bool, RetainedValue::Bool(flag)) => Ok(PreparedArgument {
            primary: if *flag {
                "true".to_owned()
            } else {
                "false".to_owned()
            },
            extra_borrow_pointers: Vec::new(),
        }),
        (ResolvedType::U8, RetainedValue::U8(byte)) => Ok(PreparedArgument {
            primary: format!("UINT8_C({byte})"),
            extra_borrow_pointers: Vec::new(),
        }),
        (ResolvedType::Usize, RetainedValue::Usize(count)) => Ok(PreparedArgument {
            primary: format!("UINT64_C({count})"),
            extra_borrow_pointers: Vec::new(),
        }),
        // An `own Bytes` parameter crosses the generated C ABI by value.
        // `bytes_expr` creates the one C owner which the callee's checked
        // cleanup plan consumes; it is deliberately not a borrowed slice or
        // a pointer to a caller-owned temporary.
        (ResolvedType::Bytes, RetainedValue::Bytes(bytes)) => Ok(PreparedArgument {
            primary: emitter.bytes_expr(bytes),
            extra_borrow_pointers: Vec::new(),
        }),
        (ResolvedType::Nominal { .. }, RetainedValue::Record(record)) => {
            if record.record != *nominal_declaration(ty)? {
                return Err(invariant("native_executor.argument.record_identity"));
            }
            let layout = AggregateLayout::for_type(program, AggregateTarget::Native64, ty)
                .map_err(|_| invariant("native_executor.argument.layout"))?;
            emitter.arg_ordinal += 1;
            let var = format!("spx_native_exec_arg_{}", emitter.arg_ordinal);
            emitter.record_local(&layout, record, &var)?;
            let mut extra = Vec::new();
            if ownership == OwnershipMode::Borrow {
                for field in &layout.fields {
                    if field.ty == ResolvedType::Bytes {
                        extra.push(format!("&{var}.{}", field_symbol(&field.field)));
                        emitter
                            .borrowed_owners
                            .push(format!("{var}.{}", field_symbol(&field.field)));
                    }
                }
            }
            Ok(PreparedArgument {
                primary: format!("&{var}"),
                extra_borrow_pointers: extra,
            })
        }
        _ => Err(invariant("native_executor.argument.shape")),
    }
}

fn nominal_declaration(ty: &ResolvedType) -> Result<&DeclarationId, Diagnostic> {
    match ty {
        ResolvedType::Nominal { declaration, .. } => Ok(declaration),
        _ => Err(invariant("native_executor.argument.not_nominal")),
    }
}

/// Emits the decode/print statements for one record-typed result, reusing
/// the record's own layout for field order and member names.
fn emit_record_print(
    body: &mut String,
    layout: &AggregateLayout,
    expr: &str,
) -> Result<(), Diagnostic> {
    body.push_str("    printf(\"RECORD\");\n");
    for field in &layout.fields {
        let hex = hex_payload(&field.field);
        match field.ty {
            ResolvedType::Bytes => {
                body.push_str(&format!(
                    "    printf(\" {hex}=B:\"); for (uint64_t spx_i = UINT64_C(0); spx_i < ({expr}).{member}.len; ++spx_i) printf(\"%02x\", (unsigned)((({expr}).{member}.ptr)[spx_i]));\n",
                    member = field_symbol(&field.field)
                ));
            }
            ResolvedType::I64 => {
                body.push_str(&format!(
                    "    printf(\" {hex}=I:%lld\", (long long)({expr}).{member});\n",
                    member = field_symbol(&field.field)
                ));
            }
            ResolvedType::Bool => {
                body.push_str(&format!(
                    "    printf(\" {hex}=T:%u\", (unsigned)(({expr}).{member} ? 1 : 0));\n",
                    member = field_symbol(&field.field)
                ));
            }
            ResolvedType::Usize => {
                body.push_str(&format!(
                    "    printf(\" {hex}=U:%llu\", (unsigned long long)({expr}).{member});\n",
                    member = field_symbol(&field.field)
                ));
            }
            ResolvedType::U8 => {
                body.push_str(&format!(
                    "    printf(\" {hex}=Q:%u\", (unsigned)({expr}).{member});\n",
                    member = field_symbol(&field.field)
                ));
            }
            _ => return Err(invariant("native_executor.result.leaf")),
        }
    }
    body.push_str("    printf(\"\\n\");\n");
    for field in layout
        .fields
        .iter()
        .rev()
        .filter(|field| field.ty == ResolvedType::Bytes)
    {
        emit_settle(
            body,
            &format!("({expr}).{}", field_symbol(&field.field)),
            "spx_result_settled",
        );
    }
    Ok(())
}

fn emit_variant_print(
    body: &mut String,
    layout: &VariantLayout,
    expr: &str,
) -> Result<(), Diagnostic> {
    body.push_str(&format!("    switch (({expr}).spx_tag) {{\n"));
    for case in &layout.cases {
        body.push_str(&format!("    case UINT32_C({}): {{\n", case.tag));
        body.push_str(&format!(
            "        printf(\"VARIANT {}\");\n",
            hex_payload(&case.case)
        ));
        for field in &case.fields {
            let hex = hex_payload(&field.field);
            let member = format!(
                "({expr}).spx_payload.{}.{}",
                case_symbol(&case.case),
                field_symbol(&field.field)
            );
            match field.ty {
                ResolvedType::Bytes => {
                    body.push_str(&format!(
                        "        printf(\" {hex}=B:\"); for (uint64_t spx_i = UINT64_C(0); spx_i < ({member}).len; ++spx_i) printf(\"%02x\", (unsigned)((({member}).ptr)[spx_i]));\n"
                    ));
                }
                ResolvedType::I64 => {
                    body.push_str(&format!(
                        "        printf(\" {hex}=I:%lld\", (long long)({member}));\n"
                    ));
                }
                ResolvedType::Bool => {
                    body.push_str(&format!(
                        "        printf(\" {hex}=T:%u\", (unsigned)({member} ? 1 : 0));\n"
                    ));
                }
                ResolvedType::U8 => {
                    body.push_str(&format!(
                        "        printf(\" {hex}=Q:%u\", (unsigned)({member}));\n"
                    ));
                }
                _ => return Err(invariant("native_executor.result.leaf")),
            }
        }
        body.push_str("        printf(\"\\n\");\n");
        for field in case
            .fields
            .iter()
            .rev()
            .filter(|field| field.ty == ResolvedType::Bytes)
        {
            emit_settle(
                body,
                &format!(
                    "({expr}).spx_payload.{}.{}",
                    case_symbol(&case.case),
                    field_symbol(&field.field)
                ),
                "spx_result_settled",
            );
        }
        body.push_str("        break;\n    }\n");
    }
    body.push_str("    default: printf(\"INVALID_TAG\\n\"); break;\n    }\n");
    Ok(())
}

// A receipt counts completed physical finalizers, not a cleanup-plan prediction.
// Clearing the carrier is part of the runtime finalizer's postcondition. Result
// publication remains in Rust, after the complete transcript is authenticated.
fn emit_settle(body: &mut String, owner: &str, counter: &str) {
    body.push_str(&format!(
        "    spx_bytes_drop(&({owner}));\n    if (({owner}).ptr != NULL || ({owner}).len != 0) return 91;\n    ++{counter};\n"
    ));
}

fn c_value_type_name(
    program: &hir::ResolvedProgram,
    ty: &ResolvedType,
) -> Result<String, Diagnostic> {
    match ty {
        ResolvedType::Nominal { declaration, .. } => {
            if AggregateLayout::for_type(program, AggregateTarget::Native64, ty).is_ok() {
                return Ok(format!("struct {}", record_symbol(declaration)));
            }
            if VariantLayout::for_type(program, VariantTarget::Native64, ty).is_ok() {
                return Ok(format!("struct {}", variant_symbol(declaration)));
            }
            Err(invariant("native_executor.result.shape"))
        }
        _ => Err(invariant("native_executor.result.shape")),
    }
}

static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);

fn probe_root() -> PathBuf {
    let ordinal = NEXT_PROBE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-native-stage-executor-{}-{ordinal}",
        std::process::id()
    ))
}

/// One private 0700 probe directory held by descriptor. Every transition from
/// generated source to compiled child rechecks this held directory and opens
/// its child by `openat(..., NOFOLLOW)`, so replacing the path cannot redirect
/// the native stage executor into an attacker-selected file.
#[cfg(unix)]
struct ProbeDirectory {
    path: PathBuf,
    held: File,
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl ProbeDirectory {
    fn create() -> Result<Self, Diagnostic> {
        use rustix::fs::{mkdir, open, Mode, OFlags};
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let path = probe_root();
        mkdir(&path, Mode::from_bits_truncate(0o700))
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        let held = open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.probe_directory"))?;
        let metadata = held
            .metadata()
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(invariant("native_executor.probe_directory"));
        }
        Ok(Self {
            path,
            held,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn recheck(&self) -> Result<(), Diagnostic> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = self
            .held
            .metadata()
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.dev() != self.device
            || metadata.ino() != self.inode
        {
            return Err(invariant("native_executor.probe_directory"));
        }
        Ok(())
    }

    fn write_source(&self, source: &[u8]) -> Result<(), Diagnostic> {
        use rustix::fs::{openat, Mode, OFlags};
        self.recheck()?;
        let file = openat(
            &self.held,
            c"native_executor.c",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.write_source"))?;
        let mut file = file;
        file.write_all(source)
            .and_then(|()| file.sync_all())
            .map_err(|_| invariant("native_executor.write_source"))
    }

    fn open_child(&self, name: &std::ffi::CStr) -> Result<File, Diagnostic> {
        use rustix::fs::{openat, Mode, OFlags};
        self.recheck()?;
        let child = openat(
            &self.held,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.host.program_open"))?;
        if !child
            .metadata()
            .map_err(|_| invariant("native_executor.host.program_open"))?
            .is_file()
        {
            return Err(invariant("native_executor.host.program_open"));
        }
        Ok(child)
    }

    fn cleanup(&self) {
        use std::os::unix::fs::MetadataExt;
        // Never recursively remove a path that could have been replaced by a
        // same-UID adversary. A drifted probe is intentionally left for the
        // host's temporary-file cleanup rather than deleting foreign data.
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_dir() && metadata.dev() == self.device && metadata.ino() == self.inode {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(not(unix))]
struct ProbeDirectory {
    path: PathBuf,
    held: File,
}

#[cfg(not(unix))]
impl ProbeDirectory {
    fn create() -> Result<Self, Diagnostic> {
        let path = probe_root();
        std::fs::create_dir(&path).map_err(|_| invariant("native_executor.probe_directory"))?;
        let held = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        Ok(Self { path, held })
    }
    fn recheck(&self) -> Result<(), Diagnostic> {
        Ok(())
    }
    fn write_source(&self, source: &[u8]) -> Result<(), Diagnostic> {
        std::fs::write(self.path.join("native_executor.c"), source)
            .map_err(|_| invariant("native_executor.write_source"))
    }
    fn open_child(&self, _name: &std::ffi::CStr) -> Result<File, Diagnostic> {
        OpenOptions::new()
            .read(true)
            .open(self.path.join("native_executor"))
            .map_err(|_| invariant("native_executor.host.program_open"))
    }
    fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run(
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
    host: &NativeStageHost,
    optimization: &str,
    cancellation: Option<&crate::agent_runtime::AgentCancellation>,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    if !(1..=1_000_000).contains(&max_steps) {
        return Err(invariant("native_executor.max_steps"));
    }
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == prepared.function_id())
        .ok_or_else(|| invariant("native_executor.entry.absent"))?;
    if entry.params.len() != arguments.len() || arguments.len() != prepared.parameter_count() {
        return Err(invariant("native_executor.argument.arity"));
    }
    let (body, borrowed_count) = render_driver(program, entry, arguments)?;
    let generated =
        crate::codegen::emit_hir_c(program).map_err(|_| invariant("native_executor.codegen"))?;
    let root = ProbeDirectory::create()?;
    let outcome = compile_and_run(&generated, &body, &root, host, optimization, cancellation);
    root.cleanup();
    let stdout = outcome?;
    let result_declaration = nominal_declaration(&entry.return_type)?.clone();
    decode(
        entry.id.clone(),
        &result_declaration,
        &stdout,
        max_steps,
        borrowed_count,
    )
}

fn render_driver(
    program: &hir::ResolvedProgram,
    entry: &hir::ResolvedFunction,
    arguments: &[RetainedValue],
) -> Result<(String, usize), Diagnostic> {
    let mut emitter = Emitter::new();
    let mut call_args = Vec::new();
    for (parameter, argument) in entry.params.iter().zip(arguments) {
        let prepared_argument = prepare_argument(
            &mut emitter,
            program,
            parameter.ownership,
            &parameter.ty,
            argument,
        )?;
        // Each borrowed aggregate's extra `spx_bytes_v1*` byte-path
        // parameters are interleaved immediately after that same argument,
        // exactly as `codegen::native_emit::emit_function` emits them --
        // never batched at the end of the parameter list.
        call_args.push(prepared_argument.primary);
        call_args.extend(prepared_argument.extra_borrow_pointers);
    }

    let result_type = c_value_type_name(program, &entry.return_type)?;
    let symbol = function_symbol(&entry.id);

    let mut body = String::new();
    body.push_str("    struct spx_status_entry spx_status_entries[UINT32_C(32)];\n");
    body.push_str("    struct spx_context spx_ctx = {0};\n");
    body.push_str("    unsigned spx_borrowed_settled = 0, spx_result_settled = 0;\n");
    body.push_str("    if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(32), NULL, NULL, NULL)) { return 90; }\n");
    body.push_str(&emitter.body);
    body.push_str(&format!(
        "    {result_type} spx_native_exec_result;\n    spx_status_token spx_native_exec_token = {symbol}(&spx_ctx"
    ));
    for argument in &call_args {
        body.push_str(&format!(", {argument}"));
    }
    body.push_str(", &spx_native_exec_result);\n");
    // Borrowed argument copies stay caller-owned on both success and failure.
    // Own parameters were transferred to the selected callee: never drop them here.
    for owner in emitter.borrowed_owners.iter().rev() {
        emit_settle(&mut body, owner, "spx_borrowed_settled");
    }
    body.push_str("    if (spx_native_exec_token != SPX_STATUS_SUCCESS) { printf(\"STATUS_FAILURE\\n\"); } else {\n");

    if let Ok(layout) =
        AggregateLayout::for_type(program, AggregateTarget::Native64, &entry.return_type)
    {
        emit_record_print(&mut body, &layout, "spx_native_exec_result")?;
    } else if let Ok(layout) =
        VariantLayout::for_type(program, VariantTarget::Native64, &entry.return_type)
    {
        emit_variant_print(&mut body, &layout, "spx_native_exec_result")?;
    } else {
        return Err(invariant("native_executor.result.shape"));
    }
    body.push_str("    }\n");
    body.push_str(&format!(
        "    printf(\"SETTLED native-boundary-v1 {} %u %u\\n\", spx_borrowed_settled, spx_result_settled);\n    return 0;\n",
        hex_payload(&entry.id)
    ));
    Ok((body, emitter.borrowed_owners.len()))
}

fn compile_and_run(
    generated: &str,
    driver_body: &str,
    root: &ProbeDirectory,
    host: &NativeStageHost,
    optimization: &str,
    cancellation: Option<&crate::agent_runtime::AgentCancellation>,
) -> Result<String, Diagnostic> {
    let source = format!("{generated}\nint main(void) {{\n{driver_body}\n}}\n");
    root.write_source(source.as_bytes())?;
    if cancellation.is_some_and(crate::agent_runtime::AgentCancellation::is_cancelled) {
        return Err(invariant("native_executor.process.cancelled"));
    }
    host.compile(root, optimization, cancellation)?;
    if cancellation.is_some_and(crate::agent_runtime::AgentCancellation::is_cancelled) {
        return Err(invariant("native_executor.process.cancelled"));
    }
    let stdout = host.run(root, cancellation)?;
    if cancellation.is_some_and(crate::agent_runtime::AgentCancellation::is_cancelled) {
        return Err(invariant("native_executor.process.cancelled"));
    }
    String::from_utf8(stdout).map_err(|_| invariant("native_executor.output_utf8"))
}

fn decode(
    function_id: DeclarationId,
    result_declaration: &DeclarationId,
    stdout: &str,
    max_steps: usize,
    borrowed_count: usize,
) -> Result<RetainedCallEvaluation, Diagnostic> {
    let mut lines = stdout.split_terminator('\n');
    let line = lines
        .next()
        .ok_or_else(|| invariant("native_executor.decode.empty"))?;
    let receipt = lines
        .next()
        .ok_or_else(|| invariant("native_executor.decode.settlement"))?;
    if lines.next().is_some() || !stdout.ends_with('\n') {
        return Err(invariant("native_executor.decode.settlement"));
    }
    if line == "STATUS_FAILURE" {
        check_receipt(receipt, &function_id, borrowed_count, 0)?;
        // A native contract/status failure is detected, not silently
        // ignored, but this executor does not yet reconstruct the exact
        // interpreter-shaped `NormalizedStatus` from the native status
        // arena. Reported honestly as a diagnostic rather than a guessed
        // `LanguageFailure`.
        return Err(invariant("native_executor.decode.status_failure"));
    }
    let outcome = if let Some(rest) = line.strip_prefix("RECORD") {
        RetainedCallOutcome::Returned(RetainedValue::Record(decode_record(
            result_declaration,
            rest,
        )?))
    } else if let Some(rest) = line.strip_prefix("VARIANT ") {
        RetainedCallOutcome::Returned(RetainedValue::Variant(decode_variant(
            result_declaration,
            rest,
        )?))
    } else {
        return Err(invariant("native_executor.decode.shape"));
    };
    let RetainedCallOutcome::Returned(value) = &outcome else {
        unreachable!()
    };
    let fields = match value {
        RetainedValue::Record(record) => &record.fields,
        RetainedValue::Variant(variant) => &variant.fields,
        _ => unreachable!(),
    };
    let result_count = fields
        .iter()
        .filter(|field| matches!(field.value, RetainedValue::Bytes(_)))
        .count();
    check_receipt(receipt, &function_id, borrowed_count, result_count)?;
    Ok(RetainedCallEvaluation {
        function_id,
        outcome,
        cleanup_events: vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes; result_count],
        steps_used: 0,
        max_steps,
        failure: None,
    })
}

fn check_receipt(
    receipt: &str,
    function: &DeclarationId,
    borrowed: usize,
    results: usize,
) -> Result<(), Diagnostic> {
    if receipt
        != format!(
            "SETTLED native-boundary-v1 {} {borrowed} {results}",
            hex_payload(function)
        )
    {
        return Err(invariant("native_executor.decode.settlement"));
    }
    Ok(())
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
impl NativeStageHost {
    /// Physical negative controls, reached from the owning lifecycle harness.
    pub(in crate::agent_lifecycle) fn assert_boundary_settlement_controls(&self) {
        let parsed = crate::check(
            crate::agent_lifecycle::tests::MODULE,
            Path::new("settlement.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let entry = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fixture.agent.fn.observe")
            .unwrap();
        let state = RetainedValue::Record(RetainedRecord {
            record: DeclarationId::new("fixture.agent.type.state"),
            fields: [
                ("objective", RetainedValue::Bytes(vec![1, 7, 13])),
                ("budget", RetainedValue::I64(10)),
                ("epoch", RetainedValue::I64(1)),
            ]
            .into_iter()
            .map(|(name, value)| RetainedField {
                field: DeclarationId::new(format!("fixture.agent.type.state.{name}")),
                value,
            })
            .collect(),
        });
        let (body, borrowed) = render_driver(&program, entry, &[state]).unwrap();
        assert_eq!(borrowed, 1);
        let generated_body = crate::codegen::emit_hir_c(&program).unwrap();
        // Test-only physical allocation inventory. Neither emitted receipts nor
        // result-shape counts can hide an omitted or duplicated physical free.
        let generated = format!(
            r#"#include <stdlib.h>
static void *test_live[32];
static unsigned test_allocs, test_frees;
static void *test_malloc(size_t n) {{
    void *p = malloc(n); if (!p || test_allocs == 32) abort();
    test_live[test_allocs++] = p; return p;
}}
static void test_free(void *p) {{
    if (!p) return;
    for (unsigned i=0; i<test_allocs; ++i) if (test_live[i] == p) {{
        test_live[i] = NULL; ++test_frees; free(p); return;
    }}
    abort();
}}
#define malloc test_malloc
#define free test_free
{generated_body}"#
        );
        let failure_source = crate::agent_lifecycle::tests::MODULE.replace(
            "fn observe(state: borrow State) -> Observation\n{",
            "fn observe(state: borrow State) -> Observation\n    requires false\n{",
        );
        let failure_program =
            hir::resolve(&crate::check(&failure_source, Path::new("failure.spx")).unwrap())
                .unwrap();
        let failure_generated = generated.replace(
            &generated_body,
            &crate::codegen::emit_hir_c(&failure_program).unwrap(),
        );
        let symbol = function_symbol(&entry.id);
        let body = format!("unsigned test_calls = 0;\n#define {symbol}(...) (++test_calls, {symbol}(__VA_ARGS__))\n{body}")
            .replace("    return 0;", "    if (test_calls != 1 || test_allocs != 2 || test_frees != 2) return 92;\n    return 0;");
        for optimization in ["-O0", "-O2"] {
            for mutation in [
                "none",
                "omit-borrow",
                "omit-result",
                "drop-failure",
                "duplicate-call",
                "primary-failure",
            ] {
                let mut driver = body.clone();
                if mutation.starts_with("omit-") {
                    let owner = if mutation == "omit-borrow" {
                        format!(
                            "spx_native_exec_arg_1.{}",
                            field_symbol(&DeclarationId::new("fixture.agent.type.state.objective"))
                        )
                    } else {
                        format!(
                            "(spx_native_exec_result).{}",
                            field_symbol(&DeclarationId::new("fixture.agent.type.observation.tag"))
                        )
                    };
                    let drop = format!("spx_bytes_drop(&({owner}));");
                    assert_eq!(driver.matches(&drop).count(), 1);
                    driver =
                        driver.replace(&drop, &format!("({owner}).ptr = NULL; ({owner}).len = 0;"));
                } else if mutation == "drop-failure" {
                    driver.insert_str(0, "#define spx_bytes_drop(...) abort()\n");
                } else if mutation == "duplicate-call" {
                    driver = driver.replace(
                        &format!("(++test_calls, {symbol}(__VA_ARGS__))"),
                        &format!("(++test_calls, {symbol}(__VA_ARGS__), ++test_calls, {symbol}(__VA_ARGS__))"),
                    );
                } else if mutation == "primary-failure" {
                    driver = driver.replace(
                        "test_allocs != 2 || test_frees != 2",
                        "test_allocs != 1 || test_frees != 1",
                    );
                }
                let root = ProbeDirectory::create().unwrap();
                let selected = if mutation == "primary-failure" {
                    &failure_generated
                } else {
                    &generated
                };
                let result = compile_and_run(selected, &driver, &root, self, optimization, None);
                root.cleanup();
                if mutation == "none" {
                    let stdout = result.unwrap();
                    let declaration = nominal_declaration(&entry.return_type).unwrap();
                    let evaluation =
                        decode(entry.id.clone(), declaration, &stdout, 1000, borrowed).unwrap();
                    assert_eq!(
                        evaluation.cleanup_events,
                        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
                    );
                    for forged in [
                        stdout.lines().next().unwrap().to_owned(),
                        format!("{stdout}{stdout}"),
                        stdout.replace(" 1 1\n", " 0 1\n"),
                        stdout.replace(" 1 1\n", " 1 0\n"),
                    ] {
                        assert!(
                            decode(entry.id.clone(), declaration, &forged, 1000, borrowed).is_err()
                        );
                    }
                } else if mutation == "primary-failure" {
                    let stdout =
                        result.expect("checked failure physically settles borrowed owners");
                    assert_eq!(
                        stdout,
                        format!(
                            "STATUS_FAILURE\nSETTLED native-boundary-v1 {} 1 0\n",
                            hex_payload(&entry.id)
                        )
                    );
                    let error = decode(
                        entry.id.clone(),
                        nominal_declaration(&entry.return_type).unwrap(),
                        &stdout,
                        1000,
                        borrowed,
                    )
                    .unwrap_err();
                    assert!(error
                        .message
                        .contains("native_executor.decode.status_failure"));
                } else {
                    let error = result.expect_err("physical negative control must fail");
                    assert!(error.message.contains("native_executor.run"),
                        "{mutation} at {optimization} must execute, not fail compilation: {error:?}");
                }
            }
        }
    }
}

fn decode_field(token: &str) -> Result<RetainedField, Diagnostic> {
    let (hex, value) = token
        .split_once('=')
        .ok_or_else(|| invariant("native_executor.decode.field"))?;
    let field = decode_hex_payload(hex)?;
    let value = if let Some(bytes_hex) = value.strip_prefix("B:") {
        RetainedValue::Bytes(decode_bytes_hex(bytes_hex)?)
    } else if let Some(scalar) = value.strip_prefix("I:") {
        RetainedValue::I64(
            scalar
                .parse()
                .map_err(|_| invariant("native_executor.decode.i64"))?,
        )
    } else if let Some(flag) = value.strip_prefix("T:") {
        RetainedValue::Bool(match flag {
            "0" => false,
            "1" => true,
            _ => return Err(invariant("native_executor.decode.bool")),
        })
    } else if let Some(count) = value.strip_prefix("U:") {
        RetainedValue::Usize(
            count
                .parse()
                .map_err(|_| invariant("native_executor.decode.usize"))?,
        )
    } else if let Some(byte) = value.strip_prefix("Q:") {
        RetainedValue::U8(
            byte.parse()
                .map_err(|_| invariant("native_executor.decode.u8"))?,
        )
    } else {
        return Err(invariant("native_executor.decode.field_value"));
    };
    Ok(RetainedField { field, value })
}

fn decode_bytes_hex(hex: &str) -> Result<Vec<u8>, Diagnostic> {
    if hex.len() % 2 != 0 {
        return Err(invariant("native_executor.decode.bytes"));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut index = 0;
    while index < hex.len() {
        bytes.push(
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|_| invariant("native_executor.decode.bytes"))?,
        );
        index += 2;
    }
    Ok(bytes)
}

fn decode_record(
    record_declaration: &DeclarationId,
    rest: &str,
) -> Result<RetainedRecord, Diagnostic> {
    let mut fields = Vec::new();
    for token in rest.split_whitespace() {
        fields.push(decode_field(token)?);
    }
    Ok(RetainedRecord {
        record: record_declaration.clone(),
        fields,
    })
}

fn decode_variant(
    variant_declaration: &DeclarationId,
    rest: &str,
) -> Result<RetainedVariant, Diagnostic> {
    let mut parts = rest.split_whitespace();
    let case_hex = parts
        .next()
        .ok_or_else(|| invariant("native_executor.decode.case"))?;
    let case = decode_hex_payload(case_hex)?;
    let mut fields = Vec::new();
    for token in parts {
        fields.push(decode_field(token)?);
    }
    Ok(RetainedVariant {
        variant: variant_declaration.clone(),
        case,
        fields,
    })
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod multi_owner_cleanup_tests;
