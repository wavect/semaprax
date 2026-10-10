use super::*;

/// Additive match-mode projection. Value is intentionally absent because it
/// was implicit in every graph through v20; emitting it would change legacy
/// module, context, and agent-context bytes.
pub(super) fn explicit_match_mode_json(mode: ResolvedMatchMode) -> &'static str {
    match mode {
        ResolvedMatchMode::Value => "",
        ResolvedMatchMode::Own => ",\"ownership_mode\":\"own\"",
        ResolvedMatchMode::Borrow => ",\"ownership_mode\":\"borrow\"",
    }
}

pub(super) fn unary_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

pub(super) fn binary_text(op: BinaryOp) -> &'static str {
    op.text()
}
