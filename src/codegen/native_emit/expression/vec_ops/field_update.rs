//! Consuming Copy-slot update: stage all operands before one checked store.
use super::super::super::{backend_error, CEmitter, COutput, CValue};
use crate::codegen::native_bytes::NativeBytesProducer as _;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};
impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_field_update(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let call = crate::vec_ops::field_update::inspect(expression)
            .ok_or_else(|| backend_error("invalid checked field update call"))?;
        let (position, scalar) = crate::vec_ops::field_update::position(
            &self.program.declarations,
            call.element,
            call.identity,
        )
        .ok_or_else(|| backend_error("field update descriptor failed independent replay"))?;
        self.require_type(&call.value.ty, &scalar, "field update scalar")?;
        let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, call.element)
            .ok_or_else(|| backend_error("field update has no native nominal descriptor"))?;
        let tag = super::vec_element_tag(&scalar)?;
        if layout.kinds.get(position).copied() != Some(tag as u32) {
            return Err(backend_error(
                "field update declaration ordinal/type disagrees with native layout",
            ));
        }
        self.require_type(&expression.ty, &call.source.ty, "field update owner result")?;
        let mut values = Vec::with_capacity(3);
        for (ordinal, argument) in [call.source, call.index, call.value]
            .into_iter()
            .enumerate()
        {
            let value = self.emit_expr(argument)?;
            values.push(self.stage_bytes_call_argument(
                &expression.id,
                ordinal,
                argument,
                crate::vec_ops::VecOp::UpdateField.param_ownership_for(ordinal, call.element),
                value,
            )?);
        }
        self.require_type(&values[1].ty, &ResolvedType::Usize, "field update index")?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("field update lacks cleanup plan"))?;
        let (source, flag, _) = plan.call_argument(&expression.id, 0)?;
        let (source, flag) = (source.to_owned(), flag.to_owned());
        let destination = plan
            .value(&crate::cleanup_plan::StorageId::Temporary(
                expression.id.clone(),
            ))?
            .to_owned();
        let bits = super::vec_scalar_to_bits(&values[2]);
        self.line(&format!("spx_status = spx_leaf_update_field(spx_ctx, &{source}, &{}, {}, UINT32_C({position}), UINT32_C({tag}), {bits}, &{destination});", layout.symbol, values[1].code));
        self.emit_operation_failure_check()?;
        self.line(&format!("{flag} = false;"));
        for line in plan.apply_at(&expression.id)?.lines() {
            self.line(line);
        }
        Ok(CValue {
            code: plan
                .result_at(&expression.id)
                .unwrap_or(&destination)
                .to_owned(),
            ty: expression.ty.clone(),
        })
    }
}
