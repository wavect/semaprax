use super::*;

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_if_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        struct Continuation<'a> {
            expr: &'a ResolvedExpr,
            then_branch: &'a ResolvedExpr,
            else_branch: &'a ResolvedExpr,
            temporary: String,
        }

        let mut continuations = Vec::new();
        let mut current = expr;
        let mut value = loop {
            let ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } = &current.kind
            else {
                unreachable!("non-If expression reached emit_if_expr")
            };
            let condition = self.emit_expr(condition)?;
            self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
            let temporary = if matches!(current.ty, ResolvedType::Bytes) {
                self.bytes_plan
                    .ok_or_else(|| backend_error("owned Bytes if has no cleanup plan"))?
                    .value(&crate::cleanup_plan::StorageId::Temporary(
                        current.id.clone(),
                    ))?
                    .to_owned()
            } else {
                self.temporary(&current.ty)?
            };
            self.line(&format!("if ({}) {{", condition.code));
            self.indent += 1;
            continuations.push(Continuation {
                expr: current,
                then_branch,
                else_branch,
                temporary,
            });
            if matches!(then_branch.kind, ResolvedExprKind::If { .. }) {
                current = then_branch;
            } else {
                break self.emit_expr(then_branch)?;
            }
        };

        while let Some(continuation) = continuations.pop() {
            self.require_type(&value.ty, &continuation.expr.ty, "then branch")?;
            self.assign_branch_result(
                continuation.expr,
                continuation.then_branch,
                &continuation.temporary,
                &value,
            )?;
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            let else_value = self.emit_expr(continuation.else_branch)?;
            self.require_type(&else_value.ty, &continuation.expr.ty, "else branch")?;
            self.assign_branch_result(
                continuation.expr,
                continuation.else_branch,
                &continuation.temporary,
                &else_value,
            )?;
            self.indent -= 1;
            self.line("}");
            self.finish_variant_join(continuation.expr, &continuation.temporary)?;
            value = CValue {
                code: continuation.temporary,
                ty: continuation.expr.ty.clone(),
            };
        }
        Ok(value)
    }

    fn assign_branch_result(
        &mut self,
        expr: &ResolvedExpr,
        branch: &ResolvedExpr,
        temporary: &str,
        value: &CValue,
    ) -> Result<(), Diagnostic> {
        if matches!(expr.ty, ResolvedType::Bytes) {
            let plan = self
                .bytes_plan
                .ok_or_else(|| backend_error("owned Bytes if has no cleanup plan"))?;
            let transitions = plan.transfer_branch_at(
                &expr.id,
                &value.code,
                &crate::cleanup_plan::CleanupPlace {
                    storage: crate::cleanup_plan::StorageId::Temporary(expr.id.clone()),
                    projections: Vec::new(),
                },
            )?;
            for line in transitions.lines() {
                self.line(line);
            }
        } else if variant_declaration_id(self.program, &expr.ty)?.is_some() {
            if expr.ownership == hir::OwnershipMode::Own {
                let plan = self
                    .bytes_plan
                    .ok_or_else(|| backend_error("owning variant If has no plan"))?;
                let layout = self.variant_layout(&expr.ty)?;
                let transitions =
                    plan.apply_variant_if_branch(&expr.id, branch, &value.code, &layout)?;
                for line in transitions.lines() {
                    self.line(line);
                }
            }
            if expr.ownership == hir::OwnershipMode::Own {
                self.copy_variant_join_carrier(temporary, &value.code, &expr.ty)?;
            } else {
                self.line(&format!("{temporary} = {};", value.code));
            }
        } else if self.record_contains_owned_bytes(&expr.ty)? {
            let transitions = self
                .bytes_plan
                .ok_or_else(|| backend_error("owned record If has no cleanup plan"))?
                .apply_record_if_branch(&expr.id, &branch.id)?;
            for line in transitions.lines() {
                self.line(line);
            }
            self.zero_owned_record_bytes(temporary, &expr.ty)?;
            self.move_owned_record_fields(temporary, &value.code, &expr.ty)?;
        } else if matches!(expr.ty, ResolvedType::String) && self.owned_strings.is_some() {
            self.string_move(temporary, &value.code);
        } else if !matches!(expr.ty, ResolvedType::ArrayU8(0)) {
            self.line(&format!("{temporary} = {};", value.code));
        }
        Ok(())
    }
}
