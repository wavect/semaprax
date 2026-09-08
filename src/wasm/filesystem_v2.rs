//! Additive filesystem v2 imports after the frozen v1 prefix.
use super::{function_import, intern_type, write_u32, Signature, I32};
use crate::{diagnostic::Diagnostic, hir::ResolvedProgram};
pub(super) const IMPORT_COUNT: u32 = 5;
pub(super) const BASE: u32 =
    super::filesystem_ops::IMPORT_BASE + super::filesystem_ops::IMPORT_COUNT;
pub(super) const STAT: u32 = BASE;
pub(super) const LIST: u32 = BASE + 1;
pub(super) const CREATE_DIR: u32 = BASE + 2;
pub(super) const REMOVE: u32 = BASE + 3;
pub(super) const WRITE_ATOMIC: u32 = BASE + 4;
const NAMES: [&str; 5] = [
    "spx_filesystem_stat_v2",
    "spx_filesystem_list_v2",
    "spx_filesystem_create_dir_v2",
    "spx_filesystem_remove_v2",
    "spx_filesystem_write_atomic_v2",
];
pub fn emit_resolved_filesystem_ops_v2(
    program: &ResolvedProgram,
    command_id: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = super::command_io::prepare(
        program,
        command_id,
        crate::command_io_ops::CommandOperationProfile::FilesystemV2,
    )?;
    super::aggregate::emit_language_command_io(program, &plan)
}
pub(super) fn intern_import_types(
    types: &mut Vec<Signature>,
    indexes: &mut std::collections::HashMap<Signature, u32>,
) -> [u32; 5] {
    [4, 5, 4, 4, 7].map(|count| {
        intern_type(
            Signature {
                params: vec![I32; count],
                results: vec![I32],
            },
            types,
            indexes,
        )
    })
}
pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 5]) {
    for (name, index) in NAMES.iter().zip(types) {
        function_import(imports, "env", name, *index);
    }
}
pub(super) fn append_export(exports: &mut Vec<u8>) {
    super::write_name(exports, "__spx_filesystem_status_v2");
    exports.push(0x03);
    write_u32(exports, super::filesystem_ops::STATUS_GLOBAL);
}
pub(super) fn needs_list_scan(program: &ResolvedProgram) -> bool {
    let mut found = false;
    for function in &program.functions {
        crate::hir::function_value::walk(function, |expr| {
            if let crate::hir::ResolvedExprKind::HostCommandCall(call) = &expr.kind {
                found |= call.operation == crate::hir::ResolvedHostCommandOperation::FileList;
            }
        });
    }
    found
}
