//! Moving a compact row into the ordinary typed IterStep result projection.
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_owned_leaf_iterator(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::iterator_ops::IteratorOp,
        element: &ResolvedType,
        args: &[ResolvedExpr],
    ) -> Result<CValue, Diagnostic> {
        let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, element)
            .ok_or_else(|| backend_error("missing owned leaf iterator descriptor"))?;
        if layout.tag != 12 || args.len() != 1 {
            return Err(backend_error("invalid owned leaf iterator"));
        }
        let fields = crate::hir::owned_leaf_collection::layout(&self.program.declarations, element)
            .and_then(|l| l.fields)
            .map(<[_]>::to_vec);
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
            "owned leaf iterator argument",
        )?;
        self.require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "owned leaf iterator result",
        )?;
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned leaf iterator lacks cleanup plan"))?;
        let (source, flag, _) = plan.call_argument(&expr.id, 0)?;
        let (source, flag) = (source.to_owned(), flag.to_owned());
        let destination = if op == crate::iterator_ops::IteratorOp::Next {
            self.call_result_temporary(&expr.ty)?
        } else {
            plan.value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        };
        if op == crate::iterator_ops::IteratorOp::VecIntoIter {
            self.line(&format!(
                "{destination} = spx_leaf_iter_from_vec(spx_ctx, &{source}, &{});",
                layout.symbol
            ));
        } else {
            let suffix = self.next_local;
            self.next_local += 1;
            let row = format!("spx_leaf_iter_row_{suffix}");
            let rest = format!("spx_leaf_iter_rest_{suffix}");
            let tag = format!("spx_leaf_iter_tag_{suffix}");
            self.line(&format!("unsigned char {row}[{}] = {{0}};", layout.stride));
            self.line(&format!("spx_iter_v1 {rest} = {{0}}; uint32_t {tag} = 0;"));
            self.line(&format!(
                "spx_status = spx_leaf_iter_next(spx_ctx, &{source}, &{}, &{tag}, {row}, &{rest});",
                layout.symbol
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            self.line(&format!("({destination}).spx_tag = {tag};"));
            let case = super::super::super::c_case_symbol(&crate::hir::DeclarationId::new(
                crate::iterator_ops::YIELD_ID,
            ));
            let item = super::super::super::c_field_symbol(&crate::hir::DeclarationId::new(
                crate::iterator_ops::ITEM_ID,
            ));
            let rest_field = super::super::super::c_field_symbol(&crate::hir::DeclarationId::new(
                crate::iterator_ops::REST_ID,
            ));
            self.line(&format!("if ({tag} == UINT32_C(1)) {{"));
            self.indent += 1;
            let target = format!("({destination}).spx_payload.{case}.{item}");
            for (k, kind) in layout.kinds.iter().enumerate() {
                let (code, ty) = if let Some(fields) = &fields {
                    (
                        format!(
                            "{target}.{}",
                            super::super::super::c_field_symbol(&fields[k].id)
                        ),
                        fields[k].ty.clone(),
                    )
                } else {
                    (target.clone(), ResolvedType::String)
                };
                let offset = layout.offsets[k];
                if *kind >= 9 {
                    self.line(&format!(
                        "memcpy(&({code}), {row} + {offset}, sizeof({code}));"
                    ));
                } else {
                    let bits = self.temporary(&ResolvedType::Usize)?;
                    self.line(&format!(
                        "memcpy(&{bits}, {row} + {offset}, sizeof({bits}));"
                    ));
                    self.line(&format!(
                        "{code} = {};",
                        super::super::vec_ops::vec_bits_to_scalar(&bits, &ty)?
                    ));
                }
            }
            self.line(&format!(
                "({destination}).spx_payload.{case}.{rest_field} = {rest};"
            ));
            self.indent -= 1;
            self.line("}");
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
        self.line(&format!("{flag} = false;"));
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
