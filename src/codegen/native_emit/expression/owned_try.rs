//! Checked owned-carrier Result propagation, with independently typed payloads.
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_try_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if !self.try_target_enabled {
                    return Err(backend_error(
                        "copy-result propagation is allowed only in a function body",
                    ));
                }
                self.require_type(
                    residual_type,
                    self.return_type,
                    "copy-result residual target",
                )?;
                let operand_layout = self.variant_layout(&operand.ty)?;
                let residual_layout = self.variant_layout(residual_type)?;
                if operand_layout.variant != *result || residual_layout.variant != *result {
                    return Err(backend_error(
                        "copy-result propagation does not reference its resolved Result declaration",
                    ));
                }
                let operand_ok = operand_layout
                    .case(ok_case)
                    .and_then(|case| case.field(ok_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Ok payload")
                    })?;
                let operand_err = operand_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result propagation has no resolved Err payload")
                    })?;
                let residual_err = residual_layout
                    .case(err_case)
                    .and_then(|case| case.field(err_field).map(|field| (case, field)))
                    .ok_or_else(|| {
                        backend_error("copy-result residual has no resolved Err payload")
                    })?;
                self.require_type(&operand_ok.1.ty, &expr.ty, "copy-result Ok payload")?;
                self.require_type(
                    &operand_err.1.ty,
                    &residual_err.1.ty,
                    "copy-result Err payload",
                )?;
                let owned_bytes = operand.ownership == hir::OwnershipMode::Own
                    && operand.ty == *residual_type
                    && result.as_str() == crate::prelude::RESULT_ID
                    && matches!(
                        &operand.ty,
                        ResolvedType::Nominal {
                            declaration,
                            arguments,
                        } if declaration == result
                            && crate::hir::admitted_owned_byte_prelude_instance(
                                declaration,
                                arguments,
                            )
                    );
                let operand_value = self.emit_expr(operand)?;
                self.require_type(&operand_value.ty, &operand.ty, "copy-result operand")?;
                if owned_bytes {
                    let plan = self.bytes_plan.ok_or_else(|| {
                        backend_error("owned Result propagation has no cleanup plan")
                    })?;
                    let operand_stage = operand_value.code;
                    self.line(&format!(
                        "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned Result tag\");",
                        operand_layout.cases.len()
                    ));
                    self.line(&format!(
                        "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                        operand_err.0.tag
                    ));
                    self.indent += 1;
                    let authentication = plan.authenticate_variant_case_at(&expr.id, err_case)?;
                    for line in authentication.lines() {
                        self.line(line);
                    }
                    let transitions = plan.apply_try_variant_case_at(&expr.id, err_case, true)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    self.line("memset(&spx_result, 0, sizeof(spx_result));");
                    let view = plan.materialize_variant_borrow_view(
                        &crate::cleanup_plan::StorageId::ProvisionalResult,
                        "spx_result",
                        &operand_stage,
                        &residual_layout,
                    )?;
                    for line in view.lines() {
                        self.line(line);
                    }
                    if crate::hir::is_scalar_resolved_type(&operand_err.1.ty) {
                        self.line(&format!(
                            "spx_result.spx_payload.{}.{} = {operand_stage}.spx_payload.{}.{};",
                            c_case_symbol(err_case),
                            c_field_symbol(err_field),
                            c_case_symbol(err_case),
                            c_field_symbol(err_field),
                        ));
                    }
                    self.line(&format!(
                        "spx_result.spx_tag = UINT32_C({});",
                        residual_err.0.tag
                    ));
                    self.line("spx_result_staged = true;");
                    self.line("goto spx_postconditions;");
                    self.indent -= 1;
                    self.line("}");
                    self.line(&format!(
                        "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned Result tag\");",
                        operand_ok.0.tag
                    ));
                    let authentication = plan.authenticate_variant_case_at(&expr.id, ok_case)?;
                    for line in authentication.lines() {
                        self.line(line);
                    }
                    // The success lane continues through every ordinary transfer
                    // anchored at the Try expression (projected extraction, then
                    // any enclosing binding/block destinations).
                    // Dynamic TransferVariant runs only on the Err lane above.
                    let transitions = plan.apply_at(&expr.id)?;
                    for line in transitions.lines() {
                        self.line(line);
                    }
                    if crate::hir::is_scalar_resolved_type(&expr.ty) {
                        let output = self.temporary(&expr.ty)?;
                        self.line(&format!(
                            "{output} = {operand_stage}.spx_payload.{}.{};",
                            c_case_symbol(ok_case),
                            c_field_symbol(ok_field),
                        ));
                        return Ok(CValue {
                            code: output,
                            ty: expr.ty.clone(),
                        });
                    }
                    let output = plan.result_at(&expr.id).ok_or_else(|| {
                        backend_error("owned Result Ok extraction has no Bytes destination")
                    })?;
                    return Ok(CValue {
                        code: output.to_owned(),
                        ty: expr.ty.clone(),
                    });
                }
                let operand_stage = self.temporary(&operand.ty)?;
                self.line(&format!("{operand_stage} = {};", operand_value.code));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid variant tag\");",
                    operand_layout.cases.len()
                ));
                self.line(&format!(
                    "if ({operand_stage}.spx_tag == UINT32_C({})) {{",
                    operand_err.0.tag
                ));
                self.indent += 1;
                self.line("memset(&spx_result, 0, sizeof(spx_result));");
                self.line(&format!(
                    "spx_result.spx_payload.{}.{} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                    c_case_symbol(err_case),
                    c_field_symbol(err_field),
                ));
                self.line(&format!(
                    "spx_result.spx_tag = UINT32_C({});",
                    residual_err.0.tag
                ));
                self.line("spx_result_staged = true;");
                self.line("goto spx_postconditions;");
                self.indent -= 1;
                self.line("}");
                self.line(&format!(
                    "if ({operand_stage}.spx_tag != UINT32_C({})) spx_runtime_invariant_failure(\"invalid Result tag\");",
                    operand_ok.0.tag
                ));
                let output = self.temporary(&expr.ty)?;
                self.line(&format!(
                    "{output} = {operand_stage}.spx_payload.{}.{};",
                    c_case_symbol(ok_case),
                    c_field_symbol(ok_field),
                ));
                CValue {
                    code: output,
                    ty: expr.ty.clone(),
                }
            }
            _ => unreachable!("non-Try expression reached emit_try_expr"),
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }
}
