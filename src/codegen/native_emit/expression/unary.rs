use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_unary_expr(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        operand: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let mut pending: Vec<(UnaryOp, &ResolvedExpr)> = Vec::new();
        pending.push((op, expr));
        let mut current = operand;
        while let ResolvedExprKind::Unary {
            op: next_op,
            value: next_value,
        } = &current.kind
        {
            pending.push((*next_op, current));
            current = next_value;
        }
        let mut value = self.emit_expr(current)?;
        for (op, expr) in pending.into_iter().rev() {
            value = self.emit_unary_value(expr, op, value)?;
        }
        Ok(value)
    }

    fn emit_unary_value(
        &mut self,
        expr: &ResolvedExpr,
        op: UnaryOp,
        value: CValue,
    ) -> Result<CValue, Diagnostic> {
        let (ty, operand_type) = match op {
            UnaryOp::Neg => match &value.ty {
                ResolvedType::F32 => (ResolvedType::F32, ResolvedType::F32),
                ResolvedType::F64 => (ResolvedType::F64, ResolvedType::F64),
                ResolvedType::I32 => (ResolvedType::I32, ResolvedType::I32),
                _ => (ResolvedType::I64, ResolvedType::I64),
            },
            UnaryOp::Not => (ResolvedType::Bool, ResolvedType::Bool),
        };
        self.require_type(&value.ty, &operand_type, "unary operand")?;
        self.require_type(&expr.ty, &ty, "unary result")?;
        let temporary = self.temporary(&ty)?;
        match op {
            UnaryOp::Neg if matches!(ty, ResolvedType::F32 | ResolvedType::F64) => {
                self.line(&format!("{temporary} = (-({}));", value.code));
            }
            UnaryOp::Neg if ty == ResolvedType::I32 => {
                self.line(&format!(
                    "spx_status = spx_rt_neg_i32(spx_ctx, {}, &{temporary});",
                    value.code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            UnaryOp::Neg => {
                self.line(&format!(
                    "spx_status = spx_rt_neg(spx_ctx, {}, &{temporary});",
                    value.code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            UnaryOp::Not => self.line(&format!("{temporary} = (!{});", value.code)),
        }
        let value = CValue {
            code: temporary,
            ty,
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }
}
