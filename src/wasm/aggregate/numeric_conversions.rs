use crate::diagnostic::Diagnostic;
use crate::string_ops::StringOp;
use crate::wasm::numeric_conversions::OUT_OF_RANGE_STATUS;
use crate::wasm::write_i64;

impl super::Emitter<'_> {
    pub(super) fn emit_integer_conversion(
        &mut self,
        expression: &crate::hir::ResolvedExpr,
        op: StringOp,
        args: &[crate::hir::ResolvedExpr],
    ) -> Result<super::Value, Diagnostic> {
        use super::{error, require_type, value_type, Value};
        if args.len() != 1 {
            return Err(error("integer conversion arity disagrees with HIR"));
        }
        require_type(
            &expression.ty,
            &op.return_type(),
            "integer conversion result",
        )?;
        let value = self.emit_expr(&args[0])?;
        require_type(
            value_type(&value),
            &op.param_types()[0],
            "integer conversion operand",
        )?;
        self.apply_call_commit(&expression.id)?;
        if matches!(op, StringOp::I64FromUsize | StringOp::UsizeFromI64) {
            self.get_scalar(&value);
            self.output.push(0x42);
            write_i64(
                self.output,
                if op == StringOp::I64FromUsize {
                    i64::MAX
                } else {
                    0
                },
            );
            self.output.push(if op == StringOp::I64FromUsize {
                0x56
            } else {
                0x53
            });
            let saved = self.failure_expression.replace(expression.id.clone());
            let result = self.fail_if(OUT_OF_RANGE_STATUS);
            self.failure_expression = saved;
            result?;
        }
        self.get_scalar(&value);
        if matches!(op, StringOp::I64FromU8 | StringOp::UsizeFromU8) {
            self.output.push(0xad);
        }
        if op == StringOp::I64FromI32 {
            self.output.push(0xac);
        }
        let local = self.plan.expr_scalar(expression)?;
        self.output.push(0x21);
        crate::wasm::write_u32(self.output, local);
        Ok(Value::Scalar {
            local,
            ty: expression.ty.clone(),
        })
    }
}

// Shared checked integer division/remainder status selection.
pub(super) fn division_status(remainder: bool, zero: bool) -> i32 {
    match (remainder, zero) {
        (true, true) => super::STATUS_REM_ZERO,
        (true, false) => super::STATUS_REM_OVERFLOW,
        (false, true) => super::STATUS_DIV_ZERO,
        (false, false) => super::STATUS_DIV_OVERFLOW,
    }
}
