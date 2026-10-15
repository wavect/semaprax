use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_closure(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        path: &str,
        allow_moves: bool,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if self.expression_ids.is_legacy() {
            return self.validate_closure_owned_reference(
                function,
                expression,
                scope,
                path,
                allow_moves,
            );
        }
        super::super::closure::validate_shape(self.program, expression)?;
        let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
            unreachable!()
        };
        let affine = expression.ty.is_once_function();
        if affine && !allow_moves {
            return Err(hir_error(
                "affine construction cannot consume in a contract",
            ));
        }
        let ownership = if affine || expression.ty.is_mut_function() {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        for (index, capture) in captures.iter().enumerate() {
            let ResolvedExprKind::Place(place) = &capture.value.kind else {
                unreachable!()
            };
            let expected = path_label::Label::child(path, ".capture.{}", [index, 0])?;
            if !capture
                .value
                .id
                .matches_display(function, expected.len(), expected)
                || !identity_insert::expression(&mut self.expression_ids, &capture.value.id)?
            {
                return Err(hir_error(
                    "closure capture expression identity is not canonical",
                ));
            }
            if scope.get(&place.root).is_none_or(|binding| {
                binding.ty != capture.value.ty
                    || binding.ownership != capture.value.ownership
                    || binding.availability != Availability::Available
            }) {
                return Err(hir_error(
                    "closure capture does not name an available outer scalar",
                ));
            }
        }
        if affine {
            for capture in captures
                .iter()
                .filter(|c| c.value.ownership == OwnershipMode::Own)
            {
                self.mark_value_sources_moved(&capture.value, scope)?;
            }
        }
        let body = super::super::closure::paid::function(self.program, expression)?;
        let execution = FunctionExecutionId::Monomorphic(body.id.clone());
        let mut checked = if let Some(functions) = self.functions.shared_directory() {
            HirValidator::with_shared_profiles(
                self.program,
                functions,
                self.canonical_loans.share(),
                self.executable_profiles.clone(),
            )?
        } else {
            HirValidator::with_profiles(
                self.program,
                self.functions.directory(),
                self.canonical_loans.share(),
                self.executable_profiles.clone(),
            )?
        };
        checked.validate_function(&body, &execution)?;
        // Preserve expression-before-value merge order and each full prefix
        // on duplicate/refusal; each actual new node has its own constructor payer.
        for id in &checked.expression_ids {
            if !identity_insert::expression(&mut self.expression_ids, id)? {
                return Err(hir_error(
                    "closure body identities collide with the enclosing program",
                ));
            }
        }
        for id in &checked.value_ids {
            if !identity_insert::value(&mut self.value_ids, id)? {
                return Err(hir_error(
                    "closure body identities collide with the enclosing program",
                ));
            }
        }
        self.finish_expr(expression, &expression.ty, ownership)
    }
}

impl HirValidator<'_> {
    pub(super) fn validate_template_closure<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if self.expression_ids.is_legacy() {
            return self.validate_template_closure_owned_reference(
                template, execution, expression, values, path,
            );
        }
        if !super::super::generic_collection::profile(template) {
            return Err(hir_error(
                "generic closure requires the private scalar collection profile",
            ));
        }
        super::super::closure::validate_shape_scoped(self.program, expression, Some(&template.id))?;
        let ResolvedExprKind::Closure {
            captures,
            parameters,
            body,
        } = &expression.kind
        else {
            unreachable!()
        };
        for (index, capture) in captures.iter().enumerate() {
            let capture_path = super::super::closure::paid::capture_path(path, index)?;
            self.validate_template_expr(
                template,
                execution,
                &capture.value,
                values,
                &capture_path,
            )?;
        }
        let mut body_values = values.empty_like()?;
        for binding in captures
            .iter()
            .map(|capture| &capture.binding)
            .chain(parameters.iter())
        {
            if !identity_insert::value(&mut self.value_ids, &binding.id)?
                || body_values.insert(binding.id.clone(), &binding.ty)?
            {
                return Err(hir_error("generic closure bindings collide"));
            }
        }
        let body_execution = FunctionExecutionId::Monomorphic(
            super::super::closure::paid::identity(&expression.id)?,
        );
        self.validate_template_expr(template, &body_execution, body, &mut body_values, "body")?;
        self.finish_expr(expression, &expression.ty, OwnershipMode::Value)
    }
}

impl HirValidator<'_> {
    pub(super) fn finish_affine_invocation(
        &self,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        allow_moves: bool,
    ) -> Result<(), Diagnostic> {
        if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
            if callable.ty.is_mut_function() {
                super::super::closure::mutable::validate_receiver(
                    self.program,
                    callable,
                    allow_moves,
                )?;
            }
            if callable.ty.is_once_function() {
                if !allow_moves {
                    return Err(hir_error("affine invocation cannot consume in a contract"));
                }
                self.mark_value_sources_moved(callable, scope)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod old;
#[cfg(test)]
mod tests;
