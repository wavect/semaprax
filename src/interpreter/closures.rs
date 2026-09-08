//! Immutable scalar snapshot closures for the reference interpreter.

use super::*;

pub(super) const CAPTURE_SLOTS: usize = 8;

#[derive(Debug, PartialEq)]
pub(super) struct ClosureValue {
    pub(super) target: DeclarationId,
    pub(super) parameters: Vec<crate::hir::ResolvedBinding>,
    pub(super) captures: Vec<(ValueId, Value)>,
    pub(super) function: ResolvedFunction,
    pub(super) result: ResolvedType,
}

impl Evaluator<'_> {
    pub(super) fn make_closure(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let ResolvedExprKind::Closure {
            parameters,
            captures,
            body: _,
        } = &expression.kind
        else {
            unreachable!()
        };
        if captures.len() > CAPTURE_SLOTS {
            return Err(Flow::Guard("closure capture bound"));
        }
        let mut values = Vec::with_capacity(captures.len());
        for capture in captures {
            let value = self.evaluate(&capture.value, environment, depth)?;
            values.push((capture.binding.id.clone(), self.clone_value(&value)?));
        }
        let ResolvedType::Function { result, .. } = &expression.ty else {
            return Err(Flow::Guard("closure type"));
        };
        let function = self
            .closure_functions
            .get(&expression.id)
            .ok_or(Flow::Guard("closure product outside checked inventory"))?
            .clone();
        let target = function.id.clone();
        Ok(Value::Closure(Arc::new(ClosureValue {
            target,
            parameters: parameters.clone(),
            captures: values,
            function,
            result: *result.clone(),
        })))
    }
}

/// Derive body products once from the already validated retained program.
pub(super) fn checked_functions(
    program: &hir::ResolvedProgram,
) -> Result<BTreeMap<hir::ExpressionId, ResolvedFunction>, Diagnostic> {
    hir::closure::inventory(program)
        .into_iter()
        .map(|expression| {
            hir::closure::closure_function(program, expression)
                .map(|function| (expression.id.clone(), function))
        })
        .collect()
}
