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
            Value::Closure(value) => Value::Closure(Arc::clone(value)),
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
            ResolvedExprKind::Closure { .. } => self.make_closure(expression, environment, depth),
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
                let callable_value = self.evaluate(callable, environment, depth)?;
                if let Value::Closure(closure) = callable_value {
                    if closure.result != expression.ty || closure.parameters.len() != args.len() {
                        return Err(Flow::Guard("closure invocation signature mismatch"));
                    }
                    let mut frame = closure
                        .captures
                        .iter()
                        .map(|(id, value)| Ok((id.clone(), self.clone_value(value)?)))
                        .collect::<Result<Vec<_>, Flow>>()?;
                    for (parameter, argument) in closure.parameters.iter().zip(args) {
                        if parameter.ty != argument.ty {
                            return Err(Flow::Guard("closure argument type mismatch"));
                        }
                        frame.push((
                            parameter.id.clone(),
                            self.evaluate(argument, environment, depth)?,
                        ));
                    }
                    return self.call_frame(&closure.function, frame, depth + 1);
                }
                let Value::Function(target) = callable_value else {
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

pub(super) fn evaluate_resolved_entry<'a>(
    entry: &'a ResolvedFunction,
    arguments: &[(String, ArgumentValue)],
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    program: &'a hir::ResolvedProgram,
    budget: usize,
    host_stdout: bool,
) -> (Result<Value, Flow>, usize, Vec<u8>) {
    let (outcome, steps, transcript, _) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        arguments,
        admitted,
        program,
        budget,
        host_stdout,
        Utf8MaterializationBudget::UnlimitedLegacy,
    );
    (outcome, steps, transcript)
}

pub(super) fn evaluate_resolved_entry_with_utf8_budget<'a>(
    entry: &'a ResolvedFunction,
    arguments: &[(String, ArgumentValue)],
    admitted: &'a BTreeMap<&'a str, &'a ResolvedFunction>,
    program: &'a hir::ResolvedProgram,
    budget: usize,
    host_stdout: bool,
    utf8_materialization_budget: Utf8MaterializationBudget,
) -> (Result<Value, Flow>, usize, Vec<u8>, (u64, u64)) {
    let closure_functions = match closures::checked_functions(program) {
        Ok(functions) => functions,
        Err(_) => {
            return (
                Err(Flow::Guard("invalid checked closure function")),
                0,
                Vec::new(),
                (0, 0),
            )
        }
    };
    let mut evaluator = Evaluator {
        admitted: FunctionLookup::Borrowed(admitted),
        closure_functions,
        declarations: &program.declarations,
        steps: 0,
        budget,
        next_byte_allocation: 0,
        allocated_byte_payload: 0,
        box_live_allocations: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        utf8_materialization_budget,
        stdout_transcript: host_stdout.then(Vec::new),
        stderr_transcript: None,
        command_input: None,
        cancellation: PreparedCancellation::Never,
        trace_limit: 0,
        trace_events: Vec::new(),
        dropped_trace_events: 0,
        current_function: None,
        trace_identities: BTreeMap::new(),
        trace_phase: ResolvedTracePhase::Body,
        failure_detail: None,
    };
    let outcome = evaluator.evaluate_entry(entry, arguments);
    let utf8_usage = evaluator.utf8_materialization_budget.usage();
    (
        outcome,
        evaluator.steps,
        evaluator.stdout_transcript.unwrap_or_default(),
        utf8_usage,
    )
}
