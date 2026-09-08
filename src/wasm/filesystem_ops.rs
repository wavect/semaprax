//! Closed Core-Wasm imports for Filesystem I/O v1.
//!
//! The module has no WASI imports. The two synchronous `env` functions are
//! supplied by an explicit filesystem provider and return a normalized
//! `semaprax.filesystem.v1` status. Their out slots are initialized only on
//! status zero.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{function_import, intern_type, write_u32, Signature, I32};

pub(super) const IMPORT_COUNT: u32 = 2;
pub(super) const IMPORT_BASE: u32 =
    super::command_io::ARGS_LEN_IMPORT + super::command_io::IMPORT_COUNT;
pub(super) const READ_IMPORT: u32 = IMPORT_BASE;
pub(super) const WRITE_NEW_IMPORT: u32 = IMPORT_BASE + 1;
pub(super) const STATUS_GLOBAL: u32 = 15;
pub(super) const OPERATION_COUNT_GLOBAL: u32 = 16;
pub(super) const BYTE_COUNT_GLOBAL: u32 = 17;
pub(super) const STATUS_EXPORT: &str = "__spx_filesystem_status_v1";
pub(super) const IMPORT_NAMES: [&str; IMPORT_COUNT as usize] =
    ["spx_filesystem_read_v1", "spx_filesystem_write_new_v1"];

pub fn emit_resolved_filesystem_ops_v1(
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
        crate::command_io_ops::CommandOperationProfile::FilesystemV1,
    )
}

pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    const ADMITTED: [&str; 2] = [
        crate::filesystem_ops::READ_EFFECT,
        crate::filesystem_ops::WRITE_EFFECT,
    ];
    if permits
        .iter()
        .any(|permit| !ADMITTED.contains(&permit.as_str()))
        || !permits
            .iter()
            .any(|permit| ADMITTED.contains(&permit.as_str()))
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Filesystem I/O v1 admits only fs.read and fs.write permits",
        ));
    }
    Ok(())
}

pub(super) fn intern_import_types(
    types: &mut Vec<Signature>,
    type_indexes: &mut std::collections::HashMap<Signature, u32>,
) -> [u32; IMPORT_COUNT as usize] {
    [
        intern_type(
            Signature {
                params: vec![I32; 5],
                results: vec![I32],
            },
            types,
            type_indexes,
        ),
        intern_type(
            Signature {
                params: vec![I32; 7],
                results: vec![I32],
            },
            types,
            type_indexes,
        ),
    ]
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
    body.extend([0x42, 0x00, 0x24]);
    write_u32(body, OPERATION_COUNT_GLOBAL);
    body.extend([0x42, 0x00, 0x24]);
    write_u32(body, BYTE_COUNT_GLOBAL);
}

pub(super) fn append_globals(globals: &mut Vec<u8>) {
    for _ in 0..2 {
        globals.extend([0x7e, 0x01, 0x42, 0x00, 0x0b]);
    }
}
