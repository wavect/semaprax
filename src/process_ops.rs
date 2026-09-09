//! Closed, explicitly registered, synchronously settled process launch.
use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{ResolvedHostCommandOperation as Op, ResolvedType};
pub(crate) const NAME: &str = "process_run";
pub(crate) const ID: &str = "core.host.process-run";
pub(crate) const EFFECT: &str = "process.execute";
pub(crate) const STATUS_DOMAIN: &str = crate::process_provider::ProcessFailure::DOMAIN;
pub(crate) const STATUS_CODES: [u32; 7] = [
    INVALID_INPUT,
    AUTHORITY_DENIED,
    LAUNCH_FAILED,
    TIMED_OUT,
    CAPACITY_EXCEEDED,
    IO_FAILURE,
    SETTLEMENT_FAILED,
];
pub(crate) const INVALID_INPUT: u32 = 1;
pub(crate) const AUTHORITY_DENIED: u32 = 2;
pub(crate) const LAUNCH_FAILED: u32 = 3;
pub(crate) const TIMED_OUT: u32 = 4;
pub(crate) const CAPACITY_EXCEEDED: u32 = 5;
pub(crate) const IO_FAILURE: u32 = 6;
pub(crate) const SETTLEMENT_FAILED: u32 = 7;
pub(crate) const MAX_ARGUMENTS: u64 = crate::process_provider::MAX_ARGUMENTS as u64;
pub(crate) const MAX_INPUT_BYTES: u64 = crate::process_provider::MAX_INPUT_BYTES as u64;
pub(crate) const MAX_OUTPUT_BYTES: u64 = crate::process_provider::MAX_OUTPUT_BYTES as u64;
pub(crate) const HEADER_BYTES: u64 = 32;
pub(crate) const MAX_WAIT_MILLIS: u64 = crate::process_provider::MAX_TIMEOUT_MS as u64;
pub(crate) const MAX_OPERATIONS: u64 = crate::process_provider::MAX_RUNS as u64;
pub(crate) const MAX_TOTAL_BYTES: u64 = crate::process_provider::MAX_TOTAL_BYTES as u64;
pub(crate) const fn is_process(op: Op) -> bool {
    matches!(op, Op::ProcessRun)
}
pub(crate) fn by_name(name: &str) -> Option<Op> {
    (name == NAME).then_some(Op::ProcessRun)
}
pub(crate) fn by_id(id: &str) -> Option<Op> {
    (id == ID).then_some(Op::ProcessRun)
}
pub(crate) fn accepts_ast(index: usize, ty: &Type) -> bool {
    index < 8
        && if matches!(index, 1 | 3) {
            *ty == Type::SliceU8
        } else {
            *ty == Type::Usize
        }
}
pub(crate) fn accepts_resolved(index: usize, ty: &ResolvedType) -> bool {
    index < 8
        && if matches!(index, 1 | 3) {
            *ty == ResolvedType::SliceU8
        } else {
            *ty == ResolvedType::Usize
        }
}
pub(crate) fn ast_params() -> Vec<Param> {
    [
        "tool",
        "argv",
        "argv_length",
        "stdin",
        "stdin_length",
        "timeout_ms",
        "stdout_max",
        "stderr_max",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, name)| Param {
        name: name.to_owned(),
        span: Span::default(),
        mode: if matches!(index, 1 | 3) {
            ParamMode::Borrow
        } else {
            ParamMode::Value
        },
        ty: if matches!(index, 1 | 3) {
            Type::SliceU8
        } else {
            Type::Usize
        },
    })
    .collect()
}
/// Exact reservation when both bounds are literal; otherwise the checked runtime cap.
/// Invalid requested bounds fail before allocation and do not enlarge this cap.
pub(crate) fn output_capacity(stdout: Option<u64>, stderr: Option<u64>) -> u64 {
    stdout
        .zip(stderr)
        .and_then(|(a, b)| HEADER_BYTES.checked_add(a)?.checked_add(b))
        .filter(|size| *size <= MAX_OUTPUT_BYTES)
        .unwrap_or(MAX_OUTPUT_BYTES)
}
pub(crate) fn program_uses_process(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            let mut found = false;
            crate::hir::function_value::walk(f, |e| {
                if let crate::hir::ResolvedExprKind::HostCommandCall(call) = &e.kind {
                    found |= is_process(call.operation);
                }
            });
            found
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_signature_and_capacity_are_closed() {
        assert_eq!(ast_params().len(), 8);
        for (index, param) in ast_params().iter().enumerate() {
            assert!(accepts_ast(index, &param.ty));
            assert!(!accepts_ast(index, &Type::Bool));
        }
        assert!(!accepts_ast(8, &Type::Usize));
        assert_eq!(output_capacity(Some(32), Some(16)), 80);
        assert_eq!(output_capacity(Some(65_504), Some(0)), 65_536);
        assert_eq!(output_capacity(Some(65_505), Some(0)), 65_536);
        assert_eq!(output_capacity(Some(u64::MAX), Some(1)), 65_536);
        assert_eq!(output_capacity(None, Some(0)), 65_536);
        assert!(!crate::command_io_ops::admitted_in_while(Op::ProcessRun));
    }
}
