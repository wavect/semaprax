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
