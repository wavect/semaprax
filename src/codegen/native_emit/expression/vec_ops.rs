//! Native expression lowering for compiler-owned bounded Vec operations.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};

use super::super::{backend_error, CEmitter, COutput, CValue};

// `format!` resolves to the bounded codegen macro declared before
// `mod native_emit`; it must never fall back to `std::format!` here.
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_vec_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let [element] = type_arguments else {
            return Err(backend_error(
                "bounded Vec operation has incorrect type arity",
            ));
        };
        if !crate::vec_ops::resolved_element_is_admitted(element) || args.len() != op.arity() {
            return Err(backend_error(
                "bounded Vec operation has invalid resolved shape",
            ));
        }
        let tag = match element {
            ResolvedType::I64 => 1,
            ResolvedType::I32 => 2,
            ResolvedType::U8 => 3,
            ResolvedType::Usize => 4,
            ResolvedType::Char => 5,
            ResolvedType::F32 => 6,
            ResolvedType::F64 => 7,
            ResolvedType::Bool => 8,
            _ => unreachable!("admitted bounded Vec element is scalar"),
        };
        let mut values = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.emit_expr(argument)?;
            // Stage at the argument's canonical boundary. Producers may have
            // already reached this epoch; the shared plan helper authenticates
            // that case without replaying initialization or earlier transfers.
            values.push(self.stage_bytes_call_argument(
                &expr.id,
                index,
                argument,
                op.param_ownership(index),
                value,
            )?);
        }
        let return_type = op.resolved_return_type(element);
        self.require_type(&expr.ty, &return_type, "bounded Vec operation result")?;
        let plan = self.bytes_plan;
        match op {
            crate::vec_ops::VecOp::WithCapacity => {
                self.require_type(&values[0].ty, &ResolvedType::Usize, "Vec capacity")?;
                let plan =
                    plan.ok_or_else(|| backend_error("Vec allocation has no cleanup plan"))?;
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                self.line(&format!(
                    "spx_status = spx_vec_with_capacity(spx_ctx, UINT32_C({tag}), {}, &{destination});",
                    values[0].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
                    ty: return_type,
                })
            }
            crate::vec_ops::VecOp::Push => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec push owner",
                )?;
                self.require_type(&values[1].ty, element, "Vec push element")?;
                let plan = plan.ok_or_else(|| backend_error("Vec push has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let source = source.to_owned();
                let source_flag = source_flag.to_owned();
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                let bits = vec_scalar_to_bits(&values[1]);
                self.line(&format!(
                    "spx_status = spx_vec_push(spx_ctx, UINT32_C({tag}), &{source}, {bits}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
                    ty: return_type,
                })
            }
            crate::vec_ops::VecOp::ReserveExact => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec reserve owner",
                )?;
                self.require_type(
                    &values[1].ty,
                    &ResolvedType::Usize,
                    "Vec reserve additional",
                )?;
                let plan = plan.ok_or_else(|| backend_error("Vec reserve has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let source = source.to_owned();
                let source_flag = source_flag.to_owned();
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                self.line(&format!(
                    "spx_status = spx_vec_reserve_exact(spx_ctx, UINT32_C({tag}), &{source}, {}, &{destination});",
                    values[1].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
                    ty: return_type,
                })
            }
            crate::vec_ops::VecOp::Set => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec set owner",
                )?;
                self.require_type(&values[1].ty, &ResolvedType::Usize, "Vec set index")?;
                self.require_type(&values[2].ty, element, "Vec set element")?;
                let plan = plan.ok_or_else(|| backend_error("Vec set has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let source = source.to_owned();
                let source_flag = source_flag.to_owned();
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                let bits = vec_scalar_to_bits(&values[2]);
                self.line(&format!(
                    "spx_status = spx_vec_set(spx_ctx, UINT32_C({tag}), &{source}, {}, {bits}, &{destination});",
                    values[1].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
                    ty: return_type,
                })
            }
            crate::vec_ops::VecOp::Clear => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec clear owner",
                )?;
                let plan = plan.ok_or_else(|| backend_error("Vec clear has no cleanup plan"))?;
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let source = source.to_owned();
                let source_flag = source_flag.to_owned();
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                self.line(&format!(
                    "spx_status = spx_vec_clear(spx_ctx, UINT32_C({tag}), &{source}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("{source_flag} = false;"));
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: plan.result_at(&expr.id).unwrap_or(&destination).to_owned(),
                    ty: return_type,
                })
            }
            crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec borrow",
                )?;
                let temporary = self.temporary(&ResolvedType::Usize)?;
                let helper = if op == crate::vec_ops::VecOp::Len {
                    "spx_vec_len"
                } else {
                    "spx_vec_capacity"
                };
                self.line(&format!(
                    "{temporary} = {helper}(spx_ctx, &({}), UINT32_C({tag}));",
                    values[0].code
                ));
                Ok(CValue {
                    code: temporary,
                    ty: ResolvedType::Usize,
                })
            }
            crate::vec_ops::VecOp::Get => {
                self.require_type(
                    &values[0].ty,
                    &crate::vec_ops::resolved_vec(element.clone()),
                    "Vec get borrow",
                )?;
                self.require_type(&values[1].ty, &ResolvedType::Usize, "Vec get index")?;
                let bits = self.temporary(&ResolvedType::Usize)?;
                self.line(&format!(
                    "spx_status = spx_vec_get(spx_ctx, &({}), UINT32_C({tag}), {}, &{bits});",
                    values[0].code, values[1].code
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                Ok(CValue {
                    code: vec_bits_to_scalar(&bits, element),
                    ty: element.clone(),
                })
            }
        }
    }
}

fn vec_scalar_to_bits(value: &CValue) -> String {
    match value.ty {
        ResolvedType::F32 => format!("spx_vec_f32_bits({})", value.code),
        ResolvedType::F64 => format!("spx_vec_f64_bits({})", value.code),
        _ => format!("((uint64_t)({}))", value.code),
    }
}

fn vec_bits_to_scalar(bits: &str, ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::I64 => format!("((int64_t){bits})"),
        ResolvedType::I32 => format!("((int32_t){bits})"),
        ResolvedType::U8 => format!("((uint8_t){bits})"),
        ResolvedType::Usize => bits.to_owned(),
        ResolvedType::Char => format!("((uint32_t){bits})"),
        ResolvedType::F32 => format!("spx_vec_bits_f32({bits})"),
        ResolvedType::F64 => format!("spx_vec_bits_f64({bits})"),
        ResolvedType::Bool => format!("((bool){bits})"),
        _ => unreachable!("admitted bounded Vec element is scalar"),
    }
}
