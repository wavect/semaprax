use super::{COutput, ResolvedExprKind, ResolvedProgram, ResolvedType};
use crate::ast::BinaryOp;

pub(super) fn emit_runtime(output: &mut impl COutput, program: &ResolvedProgram) {
    let mut pending = Vec::new();
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        pending.push(&function.body);
        pending.extend(function.requires.iter().chain(&function.ensures));
    }
    while let Some(expression) = pending.pop() {
        if matches!(&expression.kind, ResolvedExprKind::Binary { op: BinaryOp::Rem, left, .. } if left.ty == ResolvedType::U8)
        {
            output.push_str(RUNTIME);
            return;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
}

const RUNTIME: &str = r#"static __attribute__((unused)) spx_status_token spx_rt_u8_rem(
    struct spx_context *spx_ctx, uint8_t a, uint8_t b, uint8_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    *result_out = (uint8_t)((int64_t)a % (int64_t)b);
    return SPX_STATUS_SUCCESS;
}
"#;
