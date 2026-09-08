//! Consuming Bytes payload boundaries for the v2 Box runtime.
use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_box_bytes_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::box_ops::BoxOp,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        if op == crate::box_ops::BoxOp::Get {
            return Err(backend_error("owned Box payload cannot be copied"));
        }
        let value = self.emit_expr(&args[0])?;
        let value = self.stage_bytes_call_argument(
            &expr.id,
            0,
            &args[0],
            crate::hir::OwnershipMode::Own,
            value,
        )?;
        self.require_type(
            &value.ty,
            &op.resolved_param_type(&ResolvedType::Bytes),
            "owned Box argument",
        )?;
        let return_type = op.resolved_return_type(&ResolvedType::Bytes);
        self.require_type(&expr.ty, &return_type, "owned Box result")?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned Box requires cleanup plan"))?;
        let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
        let source = source.to_owned();
        let source_flag = source_flag.to_owned();
        let destination = plan
            .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
            .to_owned();
        match op {
            crate::box_ops::BoxOp::New => {
                self.line(&format!(
                    "spx_status = spx_box_bytes_new(spx_ctx, &{source}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            crate::box_ops::BoxOp::IntoInner => {
                self.line(&format!(
                    "{destination} = spx_box_bytes_into_inner(spx_ctx, &{source});"
                ));
            }
            crate::box_ops::BoxOp::Get => unreachable!(),
        }
        self.line(&format!("{source_flag} = false;"));
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
            ty: return_type,
        })
    }
}
