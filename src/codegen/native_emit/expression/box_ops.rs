//! Native expression lowering for compiler-owned bounded Box operations.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};

use super::super::{backend_error, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_box_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::box_ops::BoxOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let [element] = type_arguments else {
            return Err(backend_error(
                "bounded Box operation has incorrect type arity",
            ));
        };
        if !crate::box_ops::resolved_element_is_admitted(element) || args.len() != 1 {
            return Err(backend_error(
                "bounded Box operation has invalid resolved shape",
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
            _ => unreachable!("admitted bounded Box element is scalar"),
        };
        let value = self.emit_expr(&args[0])?;
        let return_type = op.resolved_return_type(element);
        self.require_type(&expr.ty, &return_type, "bounded Box operation result")?;
        match op {
            crate::box_ops::BoxOp::New => {
                self.require_type(&value.ty, element, "Box element")?;
                let plan = self
                    .bytes_plan
                    .ok_or_else(|| backend_error("Box allocation has no cleanup plan"))?;
                let destination = plan
                    .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                    .to_owned();
                let bits = box_scalar_to_bits(&value);
                self.line(&format!(
                    "spx_status = spx_box_new(spx_ctx, UINT32_C({tag}), {bits}, &{destination});"
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
            crate::box_ops::BoxOp::Get => {
                self.require_type(
                    &value.ty,
                    &crate::box_ops::resolved_box(element.clone()),
                    "Box borrow",
                )?;
                let bits = self.temporary(&ResolvedType::Usize)?;
                self.line(&format!(
                    "{bits} = spx_box_get(spx_ctx, &({}), UINT32_C({tag}));",
                    value.code
                ));
                Ok(CValue {
                    code: box_bits_to_scalar(&bits, element),
                    ty: element.clone(),
                })
            }
            crate::box_ops::BoxOp::IntoInner => {
                self.require_type(
                    &value.ty,
                    &crate::box_ops::resolved_box(element.clone()),
                    "Box owner",
                )?;
                let plan = self
                    .bytes_plan
                    .ok_or_else(|| backend_error("Box consume has no cleanup plan"))?;
                for line in plan.apply_at(&args[0].id)?.lines() {
                    self.line(line);
                }
                let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
                let source = source.to_owned();
                let source_flag = source_flag.to_owned();
                let bits = self.temporary(&ResolvedType::Usize)?;
                self.line(&format!(
                    "{bits} = spx_box_into_inner(spx_ctx, &{source}, UINT32_C({tag}));"
                ));
                self.line(&format!("{source_flag} = false;"));
                for line in plan.apply_at(&expr.id)?.lines() {
                    self.line(line);
                }
                Ok(CValue {
                    code: box_bits_to_scalar(&bits, element),
                    ty: element.clone(),
                })
            }
        }
    }
}

fn box_scalar_to_bits(value: &CValue) -> String {
    match value.ty {
        ResolvedType::F32 => format!("spx_box_f32_bits({})", value.code),
        ResolvedType::F64 => format!("spx_box_f64_bits({})", value.code),
        _ => format!("((uint64_t)({}))", value.code),
    }
}

fn box_bits_to_scalar(bits: &str, ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::I64 => format!("((int64_t){bits})"),
        ResolvedType::I32 => format!("((int32_t){bits})"),
        ResolvedType::U8 => format!("((uint8_t){bits})"),
        ResolvedType::Usize => bits.to_owned(),
        ResolvedType::Char => format!("((uint32_t){bits})"),
        ResolvedType::F32 => format!("spx_box_bits_f32({bits})"),
        ResolvedType::F64 => format!("spx_box_bits_f64({bits})"),
        ResolvedType::Bool => format!("((bool){bits})"),
        _ => unreachable!("admitted bounded Box element is scalar"),
    }
}
