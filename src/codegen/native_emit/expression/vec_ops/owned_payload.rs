//! Consuming Bytes payload boundaries for the native Vec v2 runtime.
use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_vec_bytes_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        if op == crate::vec_ops::VecOp::Get {
            return Err(backend_error("owned Vec payload cannot be copied"));
        }
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            values.push(self.stage_bytes_call_argument(
                &expr.id,
                index,
                argument,
                op.param_ownership_for(index, &ResolvedType::Bytes),
                value,
            )?);
        }
        let return_type = op.resolved_return_type(&ResolvedType::Bytes);
        self.require_type(&expr.ty, &return_type, "owned Bytes Vec operation result")?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned Bytes Vec operation has no cleanup plan"))?;
        let destination = if op.returns_owner() {
            plan.value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            String::new()
        };
        match op {
            crate::vec_ops::VecOp::WithCapacity => {
                self.require_type(&values[0].ty, &ResolvedType::Usize, "Vec capacity")?;
                self.line(&format!(
                    "spx_status = spx_vec_bytes_with_capacity(spx_ctx, {}, &{destination});",
                    values[0].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            crate::vec_ops::VecOp::Push => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(ResolvedType::Bytes),
                    "Vec push owner",
                )?;
                self.require_type(&values[1].ty, &ResolvedType::Bytes, "Vec push element")?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let (value, value_flag, _) = plan.call_argument(&expr.id, 1)?;
                self.line(&format!("spx_status = spx_vec_bytes_push(spx_ctx, &{source}, &{value}, &{destination});"));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                self.line(&format!("{value_flag} = false;"));
            }
            crate::vec_ops::VecOp::ReserveExact => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(ResolvedType::Bytes),
                    "Vec reserve owner",
                )?;
                self.require_type(
                    &values[1].ty,
                    &ResolvedType::Usize,
                    "Vec reserve additional",
                )?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                self.line(&format!("spx_status = spx_vec_bytes_reserve_exact(spx_ctx, &{source}, {}, &{destination});", values[1].code));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
            }
            crate::vec_ops::VecOp::Set => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(ResolvedType::Bytes),
                    "Vec set owner",
                )?;
                self.require_type(&values[1].ty, &ResolvedType::Usize, "Vec set index")?;
                self.require_type(&values[2].ty, &ResolvedType::Bytes, "Vec set element")?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let (value, value_flag, _) = plan.call_argument(&expr.id, 2)?;
                self.line(&format!("spx_status = spx_vec_bytes_set(spx_ctx, &{source}, {}, &{value}, &{destination});", values[1].code));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                self.line(&format!("{value_flag} = false;"));
            }
            crate::vec_ops::VecOp::Clear => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(ResolvedType::Bytes),
                    "Vec clear owner",
                )?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                self.line(&format!(
                    "spx_status = spx_vec_bytes_clear(spx_ctx, &{source}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
            }
            crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(ResolvedType::Bytes),
                    "Vec borrow",
                )?;
                let temporary = self.temporary(&ResolvedType::Usize)?;
                let helper = if op == crate::vec_ops::VecOp::Len {
                    "spx_vec_len"
                } else {
                    "spx_vec_capacity"
                };
                self.line(&format!(
                    "{temporary} = {helper}(spx_ctx, &({}), UINT32_C(9));",
                    values[0].code
                ));
                return Ok(CValue {
                    code: temporary,
                    ty: ResolvedType::Usize,
                });
            }
            crate::vec_ops::VecOp::Get => unreachable!(),
        }
        for line in plan.apply_at(&expr.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
            ty: return_type,
        })
    }
}
