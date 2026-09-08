use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_closure(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &BTreeMap<ValueId, ValidationBinding>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        super::super::closure::validate_shape(self.program, expression)?;
        let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
            unreachable!()
        };
        for (index, capture) in captures.iter().enumerate() {
            let ResolvedExprKind::Place(place) = &capture.value.kind else {
                unreachable!()
            };
            if capture.value.id != ExpressionId::new(function, &format!("{path}.capture.{index}"))
                || !self.expression_ids.insert(capture.value.id.clone())
            {
                return Err(hir_error(
                    "closure capture expression identity is not canonical",
                ));
            }
            if scope.get(&place.root).is_none_or(|binding| {
                binding.ty != capture.value.ty
                    || binding.ownership != OwnershipMode::Value
                    || binding.availability != Availability::Available
            }) {
                return Err(hir_error(
                    "closure capture does not name an available outer scalar",
                ));
            }
        }
        let body = super::super::closure::closure_function(self.program, expression)?;
        let execution = FunctionExecutionId::Monomorphic(body.id.clone());
        let mut checked = HirValidator::new(self.program)?;
        checked.validate_function(&body, &execution)?;
        if checked
            .expression_ids
            .iter()
            .any(|id| !self.expression_ids.insert(id.clone()))
            || checked
                .value_ids
                .iter()
                .any(|id| !self.value_ids.insert(id.clone()))
        {
            return Err(hir_error(
                "closure body identities collide with the enclosing program",
            ));
        }
        self.finish_expr(expression, &expression.ty, OwnershipMode::Value)
    }
}

impl HirValidator<'_> {
    pub(super) fn validate_template_closure(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
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
            self.validate_template_expr(
                template,
                execution,
                &capture.value,
                values,
                &format!("{path}.capture.{index}"),
            )?;
        }
        let mut body_values = BTreeMap::new();
        for binding in captures
            .iter()
            .map(|capture| &capture.binding)
            .chain(parameters.iter())
        {
            if !self.value_ids.insert(binding.id.clone())
                || body_values
                    .insert(binding.id.clone(), binding.ty.clone())
                    .is_some()
            {
                return Err(hir_error("generic closure bindings collide"));
            }
        }
        let body_execution =
            FunctionExecutionId::Monomorphic(super::super::closure::closure_id(&expression.id));
        self.validate_template_expr(template, &body_execution, body, &mut body_values, "body")?;
        self.finish_expr(expression, &expression.ty, OwnershipMode::Value)
    }
}
