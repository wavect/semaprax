//! Saved recursive Block semantics; no iterative resolver frames are reused.
use super::*;
impl Resolver<'_, '_> {
    pub(super) fn resolve_block_reference(
        &self,
        function: &FunctionExecutionId,
        statements: &[Statement],
        tail: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
        nearest: Option<&crate::hir::ControlTarget>,
    ) -> Result<(ResolvedExprKind, ResolvedType, OwnershipMode), Diagnostic> {
        Ok({
            let mut scope = bindings.clone();
            let mut resolved_statements = Vec::with_capacity(statements.len());
            for (index, statement) in statements.iter().enumerate() {
                let statement_path = format!("{path}.s{index}");
                match statement {
                    Statement::Let {
                        name,
                        name_span,
                        mutable,
                        declared: _,
                        syntax: _,
                        value,
                        span,
                    } => {
                        let value = self.resolve_expr_recursive_reference_inner(
                            function,
                            value,
                            &scope,
                            &format!("{statement_path}.value"),
                            nearest,
                        )?;
                        if super::super::copy_discard::source(statement) {
                            super::super::copy_discard::require(&value)?;
                            resolved_statements
                                .push(ResolvedStatement::CopyDiscard { value, span: *span });
                            continue;
                        }
                        let binding = ResolvedBinding {
                            id: ValueId::local(function, &statement_path),
                            name: name.clone(),
                            ownership: value.ownership,
                            ty: value.ty.clone(),
                            span: *name_span,
                        };
                        scope.insert(
                            name.clone(),
                            Binding {
                                traversal_row: None,
                                id: binding.id.clone(),
                                ty: binding.ty.clone(),
                                ownership: binding.ownership,
                                mutable: *mutable,
                            },
                        );
                        resolved_statements.push(ResolvedStatement::Let {
                            binding,
                            mutable: *mutable,
                            value,
                            span: *span,
                        });
                    }
                    Statement::Assign {
                        name,
                        name_span,
                        field,
                        value,
                        span,
                    } => {
                        let immutable_code = if field.is_some() {
                            "SPX-U107"
                        } else {
                            "SPX-U101"
                        };
                        let target =
                            self.resolve_assign_target(name, *name_span, &scope, immutable_code)?;
                        let target_field = match field {
                            Some(field) => Some(self.resolve_assign_field_target(&target, field)?),
                            None => None,
                        };
                        let value = self.resolve_expr_recursive_reference_inner(
                            function,
                            value,
                            &scope,
                            &format!("{statement_path}.value"),
                            nearest,
                        )?;
                        match &target_field {
                            Some((_, field_ty)) => {
                                if value.ty != *field_ty {
                                    return Err(self.error(
                                            "SPX-U110",
                                            format!(
                                                "assigned value type `{}` does not exactly match field type `{}`",
                                                value.ty.identity_key(),
                                                field_ty.identity_key()
                                            ),
                                            value.span,
                                        ));
                                }
                            }
                            None => {
                                super::super::resolve_vec_call::validate_whole_assignment(
                                    self, function, &target, &value,
                                )?;
                            }
                        }
                        resolved_statements.push(ResolvedStatement::Assign {
                            binding: target,
                            field: target_field.map(|(field_id, _)| field_id),
                            value,
                            span: *span,
                        });
                    }
                    Statement::Unsafe {
                        audit, body, span, ..
                    } => {
                        let body = self.resolve_expr_recursive_reference_inner(
                            function,
                            body,
                            &scope,
                            &format!("{statement_path}.body"),
                            nearest,
                        )?;
                        if body.ownership != OwnershipMode::Value
                            || !is_scalar_resolved_type(&body.ty)
                        {
                            return Err(self.error(
                                "SPX-N104",
                                "unsafe boundary bodies must produce a scalar Copy value",
                                body.span,
                            ));
                        }
                        resolved_statements.push(ResolvedStatement::Unsafe {
                            audit: audit.clone(),
                            body: Box::new(body),
                            span: *span,
                        });
                    }
                    Statement::While {
                        condition,
                        body,
                        span,
                        ..
                    } => resolved_statements.push(control::while_loop(
                        self,
                        function,
                        condition,
                        body,
                        &scope,
                        &statement_path,
                        *span,
                        nearest,
                    )?),
                    Statement::ForOwn { .. }
                    | Statement::For { .. }
                    | Statement::ForBorrow { .. } => resolved_statements.push(
                        super::super::resolve_for_own::resolve_statement_reference(
                            self,
                            function,
                            &scope,
                            &statement_path,
                            statement,
                        )?,
                    ),
                }
            }
            let tail = self.resolve_expr_recursive_reference_inner(
                function,
                tail,
                &scope,
                &format!("{path}.tail"),
                nearest,
            )?;
            let ty = tail.ty.clone();
            let ownership = tail.ownership;
            (
                ResolvedExprKind::Block {
                    statements: resolved_statements,
                    tail: Box::new(tail),
                },
                ty,
                ownership,
            )
        })
    }
}
