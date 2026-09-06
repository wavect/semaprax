//! Core-Wasm lowering for compiler-owned bounded Box operations.

use super::*;

impl Emitter<'_> {
    pub(super) fn emit_box_op(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::box_ops::BoxOp,
        type_arguments: &[ResolvedType],
        args: &[ResolvedExpr],
    ) -> Result<Value, Diagnostic> {
        let [element] = type_arguments else {
            return Err(error("Box operation requires one exact type argument"));
        };
        if !crate::box_ops::resolved_element_is_admitted(element) || args.len() != 1 {
            return Err(error(
                "Box operation disagrees with its admitted scalar profile",
            ));
        }
        require_type(
            &args[0].ty,
            &op.resolved_param_type(element),
            "Box operation argument",
        )?;
        require_type(
            &expr.ty,
            &op.resolved_return_type(element),
            "Box operation result",
        )?;
        let tag = vec_element_tag(element)?;
        let base = box_import_base(self.program);
        match op {
            crate::box_ops::BoxOp::New => {
                let value = self.emit_expr(&args[0])?;
                self.require_scalar(&value, element, "Box element")?;
                let result = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: expr.ty.clone(),
                };
                self.output.push(0x41);
                write_i64(self.output, i64::from(tag));
                self.emit_vec_element_bits(&value, element)?;
                self.output.push(0x10);
                write_u32(self.output, base);
                self.output.push(0x21);
                write_u32(self.output, scalar_local(&result)?);
                self.get_scalar(&result);
                self.output.push(0x50);
                self.emit_vec_failure_if(expr, STATUS_BOX_ALLOCATION_FAILURE)?;
                Ok(result)
            }
            crate::box_ops::BoxOp::Get => {
                let source = self.emit_box_borrow_place(&args[0], element)?;
                let result = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: element.clone(),
                };
                self.get_scalar(&source);
                self.output.push(0x41);
                write_i64(self.output, i64::from(tag));
                self.output.push(0x10);
                write_u32(self.output, base + 1);
                self.store_vec_element_bits(&result, element)?;
                Ok(result)
            }
            crate::box_ops::BoxOp::IntoInner => {
                let source_value = self.emit_expr(&args[0])?;
                self.require_scalar(
                    &source_value,
                    &crate::box_ops::resolved_box(element.clone()),
                    "Box owner",
                )?;
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
                        .ok_or_else(|| {
                            error("Box consume has no authenticated call-argument carrier")
                        })?,
                    ty: crate::box_ops::resolved_box(element.clone()),
                };
                let result = Value::Scalar {
                    local: self.plan.expr_scalar(expr)?,
                    ty: element.clone(),
                };
                self.get_scalar(&source);
                self.output.push(0x41);
                write_i64(self.output, i64::from(tag));
                self.output.push(0x10);
                write_u32(self.output, base + 2);
                self.store_vec_element_bits(&result, element)?;
                self.apply_call_commit(&expr.id)?;
                self.clear_scalar(&source)?;
                Ok(result)
            }
        }
    }

    fn emit_box_borrow_place(
        &self,
        argument: &ResolvedExpr,
        element: &ResolvedType,
    ) -> Result<Value, Diagnostic> {
        let ResolvedExprKind::Place(place) = &argument.kind else {
            return Err(error("borrowed Box argument is not an exact place"));
        };
        if !place.projections.is_empty() {
            return Err(error("borrowed Box projections are outside bounded Box v1"));
        }
        let value = self.place_value(place)?;
        require_type(
            value_type(&value),
            &crate::box_ops::resolved_box(element.clone()),
            "borrowed Box carrier",
        )?;
        Ok(value)
    }
}
