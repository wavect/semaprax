//! Closed Wasm provider ABI for Environment I/O v1.

#[path = "environment_text.rs"]
pub(super) mod text;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

use super::{function_import, intern_type, write_u32, Signature, I32, I64};

pub(super) const IMPORT_COUNT: u32 = 3;
pub(super) const IMPORT_BASE: u32 =
    super::command_io::ARGS_LEN_IMPORT + super::command_io::IMPORT_COUNT;
pub(super) const LEN_IMPORT: u32 = IMPORT_BASE;
pub(super) const NAME_UTF8_IMPORT: u32 = IMPORT_BASE + 1;
pub(super) const VALUE_UTF8_IMPORT: u32 = IMPORT_BASE + 2;
pub(super) const STATUS_GLOBAL: u32 = 16;
pub(super) const STATUS_EXPORT: &str = "__spx_environment_status_v1";
const NAMES: [&str; 3] = [
    "spx_environment_len_v1",
    "spx_environment_name_utf8_v1",
    "spx_environment_value_utf8_v1",
];

pub(crate) fn emit_resolved_environment_io_v1(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = super::command_io::prepare(
        program,
        command_id,
        crate::command_io_ops::CommandOperationProfile::EnvironmentV1,
    )?;
    super::aggregate::emit_language_command_io(program, &plan)
}

pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    const ADMITTED: [&str; 5] = [
        crate::command_io_ops::ARGS_READ_EFFECT,
        crate::environment_ops::EFFECT,
        crate::command_io_ops::STDIN_READ_EFFECT,
        crate::host_io_ops::STDOUT_WRITE_EFFECT,
        crate::command_io_ops::STDERR_WRITE_EFFECT,
    ];
    if permits.is_empty()
        || permits
            .iter()
            .any(|permit| !ADMITTED.contains(&permit.as_str()))
        || !permits
            .iter()
            .any(|permit| permit == crate::environment_ops::EFFECT)
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Environment I/O v1 requires process.environment.read and admits only its five permits",
        ));
    }
    Ok(())
}

pub(super) fn intern_import_types(
    types: &mut Vec<Signature>,
    indexes: &mut std::collections::HashMap<Signature, u32>,
) -> [u32; 3] {
    [
        intern_type(
            Signature {
                params: vec![I32],
                results: vec![I32],
            },
            types,
            indexes,
        ),
        intern_type(
            Signature {
                params: vec![I64, I32],
                results: vec![I32],
            },
            types,
            indexes,
        ),
        intern_type(
            Signature {
                params: vec![I64, I32],
                results: vec![I32],
            },
            types,
            indexes,
        ),
    ]
}

pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 3]) {
    for (name, ty) in NAMES.iter().zip(types) {
        function_import(imports, "env", name, *ty);
    }
}

pub(super) fn append_global(globals: &mut Vec<u8>) {
    globals.extend([I32, 0x01, 0x41, 0x00, 0x0b]);
}
pub(super) fn append_export(exports: &mut Vec<u8>) {
    super::write_name(exports, STATUS_EXPORT);
    exports.push(0x03);
    write_u32(exports, STATUS_GLOBAL);
}
pub(super) fn emit_reset(body: &mut Vec<u8>) {
    body.extend([0x41, 0x00, 0x24, STATUS_GLOBAL as u8]);
}
