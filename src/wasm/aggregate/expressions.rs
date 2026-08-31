//! Small recursive dispatch and block lowering. Keep inactive aggregate-arm
//! temporaries off the host stack while recursively emitting scalar bodies.

use super::*;

impl Emitter<'_> {
    pub(super) fn emit_expr_inner(&mut self, expr: &ResolvedExpr) -> Result<Value, Diagnostic> {
        match &expr.kind {
            ResolvedExprKind::Block { statements, tail } => self.emit_block(expr, statements, tail),
            ResolvedExprKind::Unary { op, value } => self.emit_unary(expr, *op, value),
            ResolvedExprKind::Binary { op, left, right } => {
                self.emit_binary(expr, *op, left, right)
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.emit_if(expr, condition, then_branch, else_branch),
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                ..
            } => self.emit_call(expr, callee, instance.as_ref(), args),
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Value,
                scrutinee,
                arms,
            } if matches!(
                scrutinee.ty,
                ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::Char
                    | ResolvedType::Bool
            ) =>
            {
                if is_aggregate(self.program, &expr.ty)? {
                    return Err(error("copy match result must be i64 or bool"));
                }
                let scrutinee = self.emit_expr(scrutinee)?;
                self.emit_scalar_refutable_match(expr, &scrutinee, arms)
            }
            _ => self.emit_complex_expr(expr),
        }
    }

    fn emit_block(
        &mut self,
        expr: &ResolvedExpr,
        statements: &[ResolvedStatement],
        tail: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let saved = self.bindings.clone();
        for statement in statements {
            self.emit_block_statement(statement)?;
        }
        let tail = self.emit_expr(tail)?;
        let result = self.materialize(expr, &tail)?;
        self.emit_block_scope_cleanup(statements)?;
        self.bindings = saved;
        Ok(result)
    }

    fn emit_block_statement(&mut self, statement: &ResolvedStatement) -> Result<(), Diagnostic> {
        if self.owned_utf8_literals.is_some()
            && matches!(statement, ResolvedStatement::Unsafe { body, .. } if body.ty == ResolvedType::String)
        {
            return Err(error(
                "discarding an owned string has no admitted WebAssembly lowering",
            ));
        }
        if self.owned_utf8_literals.is_some()
            && matches!(statement, ResolvedStatement::Assign { binding, .. } if binding.ty == ResolvedType::String)
        {
            return Err(error(
                "string assignment has no admitted WebAssembly lowering",
            ));
        }
        // Field Mutation v1: the assigned value evaluates fully first, then
        // stores into the direct scalar field of the aggregate frame slot.
        if let ResolvedStatement::Assign {
            binding,
            field: Some(field_id),
            ..
        } = statement
        {
            let value = self.emit_expr(statement.value())?;
            let offset = self
                .plan
                .aggregate_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing aggregate binding `{}`", binding.id)))?;
            let record_layout = layout(self.program, &binding.ty)?;
            let field = record_layout.field(field_id).cloned().ok_or_else(|| {
                error(format!(
                    "record `{}` has no assignment field `{field_id}`",
                    record_layout.record
                ))
            })?;
            let destination = value_at(
                Pointer {
                    local: self.plan.frame_base,
                    offset: offset
                        .checked_add(field.offset)
                        .ok_or_else(|| error("field pointer overflows u32"))?,
                },
                field.ty,
                self.program,
            )?;
            self.copy_value(&destination, &value, "field assignment")?;
            return Ok(());
        }
        // Lets declare and store; assignments re-store into the same slot.
        // Unsafe boundaries emit their body transparently and bind nothing.
        let (ResolvedStatement::Let { binding, .. } | ResolvedStatement::Assign { binding, .. }) =
            statement
        else {
            if let ResolvedStatement::While {
                condition, body, ..
            } = statement
            {
                self.emit_while(condition, body)?;
            } else {
                self.emit_expr(statement.value())?;
            }
            return Ok(());
        };
        let value = self.emit_expr(statement.value())?;
        let destination = if is_aggregate(self.program, &binding.ty)? {
            let offset = self
                .plan
                .aggregate_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing aggregate binding `{}`", binding.id)))?;
            Value::Aggregate {
                pointer: Pointer {
                    local: self.plan.frame_base,
                    offset,
                },
                ty: binding.ty.clone(),
            }
        } else {
            let local = self
                .plan
                .scalar_bindings
                .get(&binding.id)
                .copied()
                .ok_or_else(|| error(format!("missing scalar binding `{}`", binding.id)))?;
            Value::Scalar {
                local,
                ty: binding.ty.clone(),
            }
        };
        self.copy_value(&destination, &value, "local binding")?;
        self.bindings.insert(binding.id.clone(), destination);
        Ok(())
    }
}
