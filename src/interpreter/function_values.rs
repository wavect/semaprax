//! Checked function identity carriers and genuine indirect interpreter calls.
use super::*;

impl Evaluator<'_> {
    /// The runtime value carrier deliberately has no `Clone` implementation.
    /// Every semantic copy therefore passes through this evaluator-owned seam,
    /// making owned UTF-8 accounting compiler-enforced at future call sites.
    pub(super) fn clone_value(&mut self, value: &Value) -> Result<Value, Flow> {
        Ok(match value {
            Value::Int(value) => Value::Int(*value),
            Value::Int32(value) => Value::Int32(*value),
            Value::Uint8(value) => Value::Uint8(*value),
            Value::Usize(value) => Value::Usize(*value),
            Value::Char(value) => Value::Char(*value),
            Value::Float32(value) => Value::Float32(*value),
            Value::Float64(value) => Value::Float64(*value),
            Value::Bool(value) => Value::Bool(*value),
            Value::ArrayU8(value) => Value::ArrayU8(Arc::clone(value)),
            Value::Bytes(value) => Value::Bytes(value.clone()),
            Value::Vec(value) => Value::Vec(Arc::clone(value)),
            Value::Box(value) => Value::Box(Arc::clone(value)),
            Value::String(value) => Value::String(self.materialize_utf8_copy(value)?),
            Value::BorrowedStr(value) => Value::BorrowedStr(value.clone()),
            Value::BorrowedSlice(value) => Value::BorrowedSlice(value.clone()),
            Value::OptionU8(value) => Value::OptionU8(*value),
            // Aggregate aliases preserve the existing authenticated-borrow
            // semantics; Arc cloning does not duplicate any nested payload.
            Value::Record(value) => Value::Record(Arc::clone(value)),
            Value::Variant(value) => Value::Variant(Arc::clone(value)),
            Value::Function(target) => Value::Function(target.clone()),
            Value::Moved => Value::Moved,
        })
    }

    pub(super) fn evaluate_function_value(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::FunctionReference { target } => {
                let function = self
                    .admitted
                    .get(target.as_str())
                    .ok_or(Flow::Guard("function reference outside admitted closure"))?;
                if hir::function_value::signature(function).as_ref() != Some(&expression.ty) {
                    return Err(Flow::Guard("function reference signature mismatch"));
                }
                Ok(Value::Function(target.clone()))
            }
            ResolvedExprKind::Invoke { callable, args } => {
                // Capture the operand before any argument is evaluated.
                let Value::Function(target) = self.evaluate(callable, environment, depth)? else {
                    return Err(Flow::Guard("indirect operand is not a function value"));
                };
                let function = self
                    .admitted
                    .get(target.as_str())
                    .ok_or(Flow::Guard("indirect target outside admitted closure"))?;
                if hir::function_value::signature(function).as_ref() != Some(&callable.ty)
                    || function.return_type != expression.ty
                    || args.len() != function.params.len()
                {
                    return Err(Flow::Guard("indirect target signature mismatch"));
                }
                let mut values = Vec::with_capacity(args.len());
                for (parameter, argument) in function.params.iter().zip(args) {
                    if parameter.ty != argument.ty {
                        return Err(Flow::Guard("indirect argument type mismatch"));
                    }
                    values.push((
                        parameter.id.clone(),
                        self.evaluate(argument, environment, depth)?,
                    ));
                }
                self.call_frame(function, values, depth + 1)
            }
            _ => Err(Flow::Guard(
                "non-callable expression reached indirect evaluator",
            )),
        }
    }
}

pub(super) fn scan_targets<'a>(
    expression: &ResolvedExpr,
    program: &'a hir::ResolvedProgram,
    admitted: &BTreeMap<&'a str, &'a ResolvedFunction>,
    visited: &mut BTreeSet<&'a str>,
    queue: &mut Vec<&'a str>,
) -> Result<(), Vec<Diagnostic>> {
    let targets = match &expression.kind {
        ResolvedExprKind::FunctionReference { target } => {
            vec![program
                .functions
                .iter()
                .find(|function| &function.id == target)
                .ok_or_else(|| reject_scan(expression, REASON_UNSUPPORTED_CALLEE))?]
        }
        ResolvedExprKind::Invoke { callable, .. } => {
            hir::function_value::compatible_targets(program, &callable.ty)
        }
        _ => Vec::new(),
    };
    for target in targets {
        let id = target.id.as_str();
        if !admitted.contains_key(id) {
            return Err(reject_scan(expression, REASON_UNSUPPORTED_CALLEE));
        }
        if visited.insert(id) {
            queue.push(id);
        }
    }
    Ok(())
}
