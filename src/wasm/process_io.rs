//! Additive Process I/O v1 imports after the frozen command/environment prefix.
use super::{function_import, intern_type, write_u32, Signature, I32, I64};
use crate::{diagnostic::Diagnostic, hir::ResolvedProgram};
pub(super) const IMPORT_COUNT: u32 = 2;
pub(super) const RUN_IMPORT: u32 =
    super::environment_io::IMPORT_BASE + super::environment_io::IMPORT_COUNT;
pub(super) const SETTLE_IMPORT: u32 = RUN_IMPORT + 1;
pub(super) const STATUS_GLOBAL: u32 = 17;
pub(super) const RUN_COUNT_GLOBAL: u32 = 18;
pub(super) const BYTE_COUNT_GLOBAL: u32 = 19;
pub(crate) fn emit_resolved_process_io_v1(
    program: &ResolvedProgram,
    command: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let plan = super::command_io::prepare(
        program,
        command,
        crate::command_io_ops::CommandOperationProfile::ProcessV1,
    )?;
    super::aggregate::emit_language_command_io(program, &plan)
}
pub(super) fn check_permits(permits: &[String]) -> Result<(), Diagnostic> {
    const ALLOWED: [&str; 6] = [
        "process.args.read",
        "process.environment.read",
        "process.execute",
        "process.stderr.write",
        "process.stdin.read",
        "process.stdout.write",
    ];
    if !permits.iter().any(|p| p == crate::process_ops::EFFECT)
        || permits.iter().any(|p| !ALLOWED.contains(&p.as_str()))
    {
        return Err(Diagnostic::io(
            "SPX-W114",
            "Process I/O v1 requires process.execute and only command/environment/append authority",
        ));
    }
    Ok(())
}
pub(super) fn intern_import_types(
    types: &mut Vec<Signature>,
    indexes: &mut std::collections::HashMap<Signature, u32>,
) -> [u32; 2] {
    let mut parameters = vec![I64; 8];
    parameters.push(I32);
    [
        intern_type(
            Signature {
                params: parameters,
                results: vec![I32],
            },
            types,
            indexes,
        ),
        intern_type(
            Signature {
                params: vec![],
                results: vec![I32],
            },
            types,
            indexes,
        ),
    ]
}
pub(super) fn emit_imports(imports: &mut Vec<u8>, types: &[u32; 2]) {
    for (name, ty) in ["spx_process_run_v1", "spx_process_settle_v1"]
        .into_iter()
        .zip(types)
    {
        function_import(imports, "env", name, *ty);
    }
}
pub(super) fn append_globals(globals: &mut Vec<u8>) {
    globals.extend([I32, 0x01, 0x41, 0, 0x0b]);
    for _ in 0..2 {
        globals.extend([I64, 0x01, 0x42, 0, 0x0b]);
    }
}
pub(super) fn append_export(exports: &mut Vec<u8>) {
    super::write_name(exports, "__spx_process_status_v1");
    exports.push(3);
    write_u32(exports, STATUS_GLOBAL);
}
pub(super) fn emit_reset(body: &mut Vec<u8>) {
    body.extend([0x41, 0, 0x24]);
    write_u32(body, STATUS_GLOBAL);
    for global in [RUN_COUNT_GLOBAL, BYTE_COUNT_GLOBAL] {
        body.extend([0x42, 0, 0x24]);
        write_u32(body, global);
    }
}
/// Always settle before publication; settlement can replace success, never a selected failure.
pub(super) fn emit_settle(body: &mut Vec<u8>, status: u32, scratch: u32) {
    body.push(0x10);
    write_u32(body, SETTLE_IMPORT);
    body.push(0x22);
    write_u32(body, scratch);
    body.extend([0x45, 0x45, 0x20]);
    write_u32(body, scratch);
    body.extend([0x41, 7, 0x47, 0x71, 0x04, 0x40]);
    // Unknown provider status is fail-stop, not a normalized process failure.
    body.push(0x41);
    super::write_i64(body, super::aggregate::STATUS_INTERNAL_INVALID_TAG as i64);
    body.push(0x21);
    write_u32(body, status);
    body.extend([0x05, 0x20]);
    write_u32(body, status);
    body.extend([0x45, 0x20]);
    write_u32(body, scratch);
    body.extend([0x41, 7, 0x46, 0x71, 0x04, 0x40, 0x41, 7, 0x22]);
    write_u32(body, status);
    body.push(0x24);
    write_u32(body, STATUS_GLOBAL);
    body.extend([0x0b, 0x0b]);
}
