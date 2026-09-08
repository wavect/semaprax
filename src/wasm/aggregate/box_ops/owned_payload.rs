//! Box v2 crosses owned Bytes handles through an explicit host contract.
use super::*;

impl Emitter<'_> {
    pub(super) fn emit_box_bytes_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::box_ops::BoxOp,
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        if op == crate::box_ops::BoxOp::Get {
            return Err(error("owned Box payload cannot be copied"));
        }
        self.emit_expr(&args[0])?;
        let source_epoch = crate::cleanup_plan::StorageId::CallArgument {
            call: expr.id.clone(),
            parameter_index: 0,
            value_expression: args[0].id.clone(),
        };
        let source = Value::Scalar {
            local: self
                .plan
                .cleanup_call_argument_carriers
                .get(&source_epoch)
                .copied()
                .ok_or_else(|| error("owned Box requires a checked staged owner"))?,
            ty: op.resolved_param_type(&ResolvedType::Bytes),
        };
        let result = Value::Scalar {
            local: self.plan.expr_scalar(expr)?,
            ty: expr.ty.clone(),
        };
        let base = box_import_base(self.program);
        match op {
            crate::box_ops::BoxOp::New => {
                self.output.push(0x41);
                write_i64(self.output, 9);
                self.get_scalar(&source);
                self.output.push(0x10);
                write_u32(self.output, base);
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
                self.get_scalar(&result);
                self.output.push(0x50);
                self.emit_vec_failure_if(expr, STATUS_BOX_ALLOCATION_FAILURE)?;
            }
            crate::box_ops::BoxOp::IntoInner => {
                self.get_scalar(&source);
                self.output.push(0x41);
                write_i64(self.output, 9);
                self.output.push(0x10);
                write_u32(self.output, base + 2);
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
            }
            crate::box_ops::BoxOp::Get => unreachable!(),
        }
        self.apply_call_commit(&expr.id)?;
        self.clear_scalar(&source)?;
        Ok(result)
    }
}
