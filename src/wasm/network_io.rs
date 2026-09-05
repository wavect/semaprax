//! Additive raw Wasm boundary for Bounded Language Network I/O v1.
//!
//! The network profile extends the Language Command I/O boundary. Its seven
//! closed `env` imports are appended after the command imports, so no existing
//! import index moves, and a dedicated exported marker authenticates the
//! `semaprax.network.v1` status sub-domain the way the command-input marker
//! authenticates command failures. The compiler grants no authority: a host
//! adapter injects the provider behind these imports, and a module emitted for
//! the Language or Line lane never names them.
//!
//! Import ABI (every pointer is a guest address, every out-slot is one
//! little-endian i64 the provider writes only on status zero):
//!
//! | import | params | result |
//! | --- | --- | --- |
//! | `spx_network_connect_v1` | host root word, host length, port, out handle | status |
//! | `spx_network_send_v1` | handle, value root word, value length, out count | status |
//! | `spx_network_recv_v1` | handle, max, out owned token | status |
//! | `spx_network_stream_stdout_v1` | handle, destination, max, out count | status |
//! | `spx_network_wait_v1` | handle, timeout ms, out state | status |
//! | `spx_network_close_v1` | handle | status |
//! | `spx_network_settle_v1` | | |
//!
//! A root word is the high half of a packed `Slice<u8>` carrier: a fixed guest
//! address, a tagged owned-arena token, or a range descriptor, exactly as
//! `spx_bytes_get` already accepts it. The stream destination is the idle
//! published stdout range: its 64 KiB equals `MAX_CHUNK_BYTES`, and the module
//! copies the delivered bytes into the staged transcript itself so the
//! combined transcript bound is enforced by compiler-owned code.

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{function_import, intern_type, write_u32, Signature, I32};

pub(super) const IMPORT_COUNT: u32 = 7;
pub(super) const IMPORT_BASE: u32 =
    super::command_io::ARGS_LEN_IMPORT + super::command_io::IMPORT_COUNT;
pub(super) const CONNECT_IMPORT: u32 = IMPORT_BASE;
pub(super) const SEND_IMPORT: u32 = IMPORT_BASE + 1;
pub(super) const RECV_IMPORT: u32 = IMPORT_BASE + 2;
pub(super) const STREAM_STDOUT_IMPORT: u32 = IMPORT_BASE + 3;
pub(super) const WAIT_IMPORT: u32 = IMPORT_BASE + 4;
pub(super) const CLOSE_IMPORT: u32 = IMPORT_BASE + 5;
pub(super) const SETTLE_IMPORT: u32 = IMPORT_BASE + 6;
/// Shares the Line lane's slot: a module is never both a line and a network
/// command, and the export name is what distinguishes the two markers.
pub(super) const STATUS_GLOBAL: u32 = 15;
pub(super) const STATUS_EXPORT: &str = "__spx_network_status_v1";
pub(super) const STREAM_SCRATCH_BASE: u32 = super::host_output::TRANSCRIPT_BASE;
pub(super) const IMPORT_NAMES: [&str; IMPORT_COUNT as usize] = [
    "spx_network_connect_v1",
    "spx_network_send_v1",
    "spx_network_recv_v1",
    "spx_network_stream_stdout_v1",
    "spx_network_wait_v1",
    "spx_network_close_v1",
    "spx_network_settle_v1",
];
const ADMITTED_PERMITS: [&str; 7] = [
    crate::network_io_ops::NETWORK_CONNECT_EFFECT,
    crate::network_io_ops::NETWORK_READ_EFFECT,
    crate::network_io_ops::NETWORK_WRITE_EFFECT,
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
    crate::command_io_ops::STDIN_READ_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
];

/// Emit the Bounded Language Network I/O v1 boundary for one explicit
/// `fn () -> bool` command of a parsed module.
pub fn emit_language_network_io_v1(
    program: &Program,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    super::reject_native_rust_imports(program)?;
    let resolved = crate::hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_language_network_io_v1(&resolved, command_id)
}

/// Emit the additive Bounded Language Network I/O v1 boundary.
pub(crate) fn emit_resolved_language_network_io_v1(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = prepare(program, command_id)?;
    super::aggregate::emit_language_command_io(program, &plan)
}

pub(super) fn prepare(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<super::command_io::CommandPlan, Diagnostic> {
    super::command_io::prepare(
        program,
        command_id,
        crate::command_io_ops::CommandOperationProfile::NetworkV1,
    )
}

/// Module permits must stay within the seven admitted tokens and name at
/// least one network effect; the profile validator separately requires the
/// selected command to reach a network operation.
pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    if permits
        .iter()
        .any(|permit| !ADMITTED_PERMITS.contains(&permit.as_str()))
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Language Network I/O v1 admits only network and process command permits",
        ));
    }
    if !permits
        .iter()
        .any(|permit| crate::network_io_ops::NETWORK_EFFECTS.contains(&permit.as_str()))
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Language Network I/O v1 requires at least one network permit",
        ));
    }
    Ok(())
}

pub(super) fn intern_import_types(
    types: &mut Vec<Signature>,
    type_indexes: &mut std::collections::HashMap<Signature, u32>,
) -> [u32; IMPORT_COUNT as usize] {
    let mut intern = |params: Vec<u8>, results: Vec<u8>| {
        intern_type(Signature { params, results }, types, type_indexes)
    };
    let four = intern(vec![I32, I32, I32, I32], vec![I32]);
    let three = intern(vec![I32, I32, I32], vec![I32]);
    let one = intern(vec![I32], vec![I32]);
    let settle = intern(Vec::new(), Vec::new());
    [four, four, three, four, three, one, settle]
}

pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; IMPORT_COUNT as usize]) {
    for (name, ty) in IMPORT_NAMES.iter().zip(types) {
        function_import(imports, "env", name, *ty);
    }
}

pub(super) fn append_export(exports: &mut Vec<u8>) {
    super::write_name(exports, STATUS_EXPORT);
    exports.push(0x03);
    write_u32(exports, STATUS_GLOBAL);
}

pub(super) fn emit_reset(body: &mut Vec<u8>) {
    body.extend([0x41, 0x00, 0x24]);
    write_u32(body, STATUS_GLOBAL);
}

/// The wrapper settles the provider once the target has returned, before any
/// status or result inspection, so every exit path releases invocation-scoped
/// handles exactly once.
pub(super) fn emit_settle(body: &mut Vec<u8>) {
    body.push(0x10);
    write_u32(body, SETTLE_IMPORT);
}

#[cfg(test)]
#[path = "network_io/tests.rs"]
mod tests;
