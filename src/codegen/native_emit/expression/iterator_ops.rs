//! Native lowering for consuming scalar iterator operations.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedType};

use super::super::{backend_error, CEmitter, COutput, CValue};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_iterator_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::iterator_ops::IteratorOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let [element] = type_arguments else {
            return Err(backend_error("iterator operation has incorrect type arity"));
        };
        if !crate::iterator_ops::resolved_element_is_admitted_in(
            &self.program.declarations,
            element,
        ) || args.len() != 1
        {
            return Err(backend_error(
                "iterator operation has invalid resolved shape",
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
            ResolvedType::Bytes => 9,
            _ if crate::hir::owned_record_collection::
                is_admitted_owned_record_collection_element(
                    &self.program.declarations,
                    element,
                ) => 10,
            _ => return Err(backend_error("unsupported iterator element")),
        };
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
            &op.resolved_param_type(element),
            "iterator owner",
        )?;
        self.require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "iterator result",
        )?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("iterator operation has no cleanup plan"))?;
        let (source, source_flag, _) = plan.call_argument(&expr.id, 0)?;
        let destination = if op == crate::iterator_ops::IteratorOp::Next {
            self.call_result_temporary(&expr.ty)?
        } else {
            plan.value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        };
        match op {
            crate::iterator_ops::IteratorOp::VecIntoIter if *element == ResolvedType::Bytes => self
                .line(&format!(
                    "{destination} = spx_iter_bytes_from_vec(spx_ctx, &{source});"
                )),
            crate::iterator_ops::IteratorOp::Next if *element == ResolvedType::Bytes => {
                self.line(&format!(
                    "spx_status = spx_iter_bytes_next(spx_ctx, &{source}, &{destination});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
            crate::iterator_ops::IteratorOp::VecIntoIter if tag == 10 => self.line(&format!(
                "{destination} = spx_iter_record_from_vec(spx_ctx, &{source});"
            )),
            crate::iterator_ops::IteratorOp::Next if tag == 10 => {
                let fields = crate::hir::owned_record_collection::owned_record_element_fields(
                    &self.program.declarations,
                    element,
                )
                .ok_or_else(|| backend_error("record iterator element shape changed"))?;
                let raw = format!("spx_iter_record_item_{}", self.next_local);
                self.next_local += 1;
                let rest = format!("spx_iter_record_rest_{}", self.next_local);
                self.next_local += 1;
                let step_tag = format!("spx_iter_record_tag_{}", self.next_local);
                self.next_local += 1;
                self.line(&format!("spx_vec_record_v1 {raw} = {{0}};"));
                self.line(&format!("spx_iter_v1 {rest} = {{0}};"));
                self.line(&format!("uint32_t {step_tag} = UINT32_C(0);"));
                self.line(&format!(
                    "spx_status = spx_iter_record_next(spx_ctx, &{source}, &{step_tag}, &{raw}, &{rest});"
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                self.line(&format!("({destination}).spx_tag = {step_tag};"));
                let case = super::super::c_case_symbol(&crate::hir::DeclarationId::new(
                    crate::iterator_ops::YIELD_ID,
                ));
                let item = super::super::c_field_symbol(&crate::hir::DeclarationId::new(
                    crate::iterator_ops::ITEM_ID,
                ));
                let rest_field = super::super::c_field_symbol(&crate::hir::DeclarationId::new(
                    crate::iterator_ops::REST_ID,
                ));
                self.line(&format!("if ({step_tag} == UINT32_C(1)) {{"));
                self.indent += 1;
                for (index, field) in fields.owned.iter().enumerate() {
                    let field = super::super::c_field_symbol(&field.id);
                    self.line(&format!(
                        "({destination}).spx_payload.{case}.{item}.{field} = spx_bytes_move(&{raw}.spx_owned[{index}]);"
                    ));
                }
                let scalar = super::super::c_field_symbol(&fields.scalar.id);
                let scalar_bits = super::super::super::native_iter::item_from_bits(
                    &format!("{raw}.spx_scalar"),
                    &fields.scalar.ty,
                );
                self.line(&format!(
                    "({destination}).spx_payload.{case}.{item}.{scalar} = {scalar_bits};"
                ));
                self.line(&format!(
                    "({destination}).spx_payload.{case}.{rest_field} = {rest};"
                ));
                self.indent -= 1;
                self.line("}");
            }
            crate::iterator_ops::IteratorOp::VecIntoIter => self.line(&format!(
                "{destination} = spx_iter_from_vec(spx_ctx, &{source}, UINT32_C({tag}));"
            )),
            crate::iterator_ops::IteratorOp::Next => {
                self.line(&format!("spx_status = spx_iter_next(spx_ctx, &{source}, UINT32_C({tag}), &{destination});"));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            }
        }
        // Both helpers have consumed the source only after their authenticated
        // read succeeds; the canonical plan owns result publication.
        self.line(&format!("{source_flag} = false;"));
        if op == crate::iterator_ops::IteratorOp::Next {
            for line in plan
                .initialize_variant_result_at(
                    &expr.id,
                    &destination,
                    &self.variant_layout(&expr.ty)?,
                )?
                .lines()
            {
                self.line(line);
            }
        }
        let result = CValue {
            code: if op == crate::iterator_ops::IteratorOp::Next {
                destination
            } else {
                plan.result_at(&expr.id).unwrap_or(&destination).to_owned()
            },
            ty: expr.ty.clone(),
        };
        self.apply_owned_plan_at_value(&expr.id, &result)?;
        Ok(result)
    }
}
