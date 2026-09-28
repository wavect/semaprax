//! Sealed consuming evaluator foundation. Drop releases process backing only for
//! parked/staged owners; only explicit settlement produces a semantic receipt.
use super::super::{
    ArgumentValue, Environment, Evaluator, Flow, FunctionLookup, OwnedBytesValue, OwnedRecordValue,
    PreparedCancellation, Value,
};
use crate::cleanup_plan::FinalizeAction;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType};
use crate::interpreter::retained_call::RetainedValue;
use crate::resumable_effects::owned_frame::CheckedOwnedFramePlan;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) struct OwnedFrameBudget {
    remaining: usize,
    consumed: usize,
    cancelled: bool,
}
impl OwnedFrameBudget {
    pub(crate) fn new(steps: usize) -> Result<Self, Diagnostic> {
        if !(1..=super::super::MAX_STEPS_LIMIT).contains(&steps) {
            return Err(rejected("invalid fuel ceiling"));
        }
        Ok(Self {
            remaining: steps,
            consumed: 0,
            cancelled: false,
        })
    }
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
    }
    pub(crate) fn consumed(&self) -> usize {
        self.consumed
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OwnedFrameFailure {
    Language(crate::conformance::NormalizedStatus),
    FuelExhausted,
    HostAbandoned,
    AnswerTypeMismatch,
    EvaluationRejected,
}
pub(crate) struct OwnedFrameArgument {
    plan: CheckedOwnedFramePlan,
    root: Option<Value>,
}
pub(crate) struct OwnedFrameArgumentRejection {
    pub(crate) input: RetainedValue,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct OwnedFrameParked {
    plan: CheckedOwnedFramePlan,
    root: Value,
    environment: Environment,
    request: ArgumentValue,
    next: usize,
}
impl OwnedFrameParked {
    pub(crate) fn request(&self) -> &ArgumentValue {
        &self.request
    }
}
pub(crate) struct OwnedFrameStagedTerminal {
    plan: CheckedOwnedFramePlan,
    root: Value,
    failure: Option<OwnedFrameFailure>,
    provisional: bool,
}
impl OwnedFrameStagedTerminal {
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
}
pub(crate) enum OwnedFrameFoundationStep {
    Parked(OwnedFrameParked),
    Terminal(OwnedFrameStagedTerminal),
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OwnedFrameReleaseReceipt {
    pub(crate) operations: Vec<FinalizeAction>,
}
pub(crate) enum OwnedFrameSettledOutcome {
    Completed(OwnedFrameResult, OwnedFrameReleaseReceipt),
    Failed(OwnedFrameFailure, OwnedFrameReleaseReceipt),
}
pub(crate) struct OwnedFrameSettlementRejection {
    pub(crate) terminal: OwnedFrameStagedTerminal,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) struct OwnedFrameResult {
    plan: CheckedOwnedFramePlan,
    root: Option<Value>,
}
pub(crate) struct OwnedFrameResultRejection {
    pub(crate) result: OwnedFrameResult,
    pub(crate) diagnostic: Diagnostic,
}
fn rejected(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-H006", format!("owned frame: {message}"))
}

/// Inert data only. Cloning this carrier never clones a language owner.
#[derive(Clone, Debug)]
pub(crate) struct OwnedFrameInput {
    pub(crate) declaration: hir::DeclarationId,
    pub(crate) fields: Vec<OwnedFrameInputField>,
}
#[derive(Clone, Debug)]
pub(crate) struct OwnedFrameInputField {
    pub(crate) identity: hir::DeclarationId,
    pub(crate) value: OwnedFrameInputValue,
}
#[derive(Clone, Debug)]
pub(crate) enum OwnedFrameInputValue {
    Bytes(Vec<u8>),
    Scalar(ArgumentValue),
}
pub(crate) struct OwnedFrameInputRejection {
    pub(crate) input: OwnedFrameInput,
    pub(crate) diagnostic: Diagnostic,
}
enum InputRef<'a> {
    Bytes(&'a [u8]),
    Scalar(&'a ArgumentValue),
    Retained(&'a RetainedValue),
}

pub(crate) fn admit_owned_frame_input(
    plan: &CheckedOwnedFramePlan,
    input: OwnedFrameInput,
) -> Result<OwnedFrameArgument, OwnedFrameInputRejection> {
    let checked = validate_fields(
        plan,
        &input.declaration,
        input.fields.len(),
        input.fields.iter().map(|f| {
            (
                &f.identity,
                match &f.value {
                    OwnedFrameInputValue::Bytes(b) => InputRef::Bytes(b),
                    OwnedFrameInputValue::Scalar(s) => InputRef::Scalar(s),
                },
            )
        }),
    );
    if let Err(diagnostic) = checked {
        return Err(OwnedFrameInputRejection { input, diagnostic });
    }
    Ok(stage_input(plan, input))
}
pub(crate) fn admit_owned_frame_argument(
    plan: &CheckedOwnedFramePlan,
    input: RetainedValue,
) -> Result<OwnedFrameArgument, OwnedFrameArgumentRejection> {
    let checked = match &input {
        RetainedValue::Record(record) => validate_fields(
            plan,
            &record.record,
            record.fields.len(),
            record
                .fields
                .iter()
                .map(|f| (&f.field, InputRef::Retained(&f.value))),
        ),
        _ => Err(rejected("expected record carrier")),
    };
    if let Err(diagnostic) = checked {
        return Err(OwnedFrameArgumentRejection { input, diagnostic });
    }
    let RetainedValue::Record(record) = input else {
        unreachable!("checked carrier")
    };
    let input = OwnedFrameInput {
        declaration: record.record,
        fields: record
            .fields
            .into_iter()
            .map(|f| OwnedFrameInputField {
                identity: f.field,
                value: match f.value {
                    RetainedValue::Bytes(b) => OwnedFrameInputValue::Bytes(b),
                    RetainedValue::I64(v) => OwnedFrameInputValue::Scalar(ArgumentValue::Int(v)),
                    RetainedValue::I32(v) => OwnedFrameInputValue::Scalar(ArgumentValue::Int32(v)),
                    RetainedValue::U8(v) => OwnedFrameInputValue::Scalar(ArgumentValue::Uint8(v)),
                    RetainedValue::Usize(v) => {
                        OwnedFrameInputValue::Scalar(ArgumentValue::Usize(v))
                    }
                    RetainedValue::Bool(v) => OwnedFrameInputValue::Scalar(ArgumentValue::Bool(v)),
                    _ => unreachable!("checked flat carrier"),
                },
            })
            .collect(),
    };
    Ok(stage_input(plan, input))
}
fn scalar_valid(ty: &ResolvedType, value: &ArgumentValue) -> bool {
    if matches!(
        value,
        ArgumentValue::BorrowedStr(_) | ArgumentValue::BorrowedSlice(_)
    ) {
        return false;
    }
    if let ArgumentValue::Char(v) = value {
        if char::from_u32(*v).is_none() {
            return false;
        }
    }
    if let ArgumentValue::Usize(v) = value {
        if *v > u32::MAX as u64 {
            return false;
        }
    }
    super::scalar_of(ty, value).is_some()
}
fn leaf_valid(ty: &ResolvedType, value: InputRef<'_>, total: &mut usize) -> bool {
    match value {
        InputRef::Bytes(bytes) => {
            *total += bytes.len();
            *ty == ResolvedType::Bytes && bytes.len() <= 1024
        }
        InputRef::Scalar(value) => scalar_valid(ty, value),
        InputRef::Retained(RetainedValue::Bytes(bytes)) => {
            leaf_valid(ty, InputRef::Bytes(bytes), total)
        }
        InputRef::Retained(value) => {
            let scalar = match value {
                RetainedValue::I64(v) => ArgumentValue::Int(*v),
                RetainedValue::I32(v) => ArgumentValue::Int32(*v),
                RetainedValue::U8(v) => ArgumentValue::Uint8(*v),
                RetainedValue::Usize(v) => ArgumentValue::Usize(*v),
                RetainedValue::Bool(v) => ArgumentValue::Bool(*v),
                _ => return false,
            };
            scalar_valid(ty, &scalar)
        }
    }
}
fn validate_fields<'a>(
    plan: &CheckedOwnedFramePlan,
    nominal: &hir::DeclarationId,
    count: usize,
    fields: impl Iterator<Item = (&'a hir::DeclarationId, InputRef<'a>)>,
) -> Result<(), Diagnostic> {
    let ResolvedType::Nominal { declaration, .. } = &plan.function().params[0].ty else {
        unreachable!()
    };
    let declared = plan
        .program()
        .declarations
        .record_fields(declaration)
        .expect("checked declaration");
    if nominal != declaration || count != declared.len() {
        return Err(rejected("record identity/count mismatch"));
    }
    let mut total = 0;
    for ((identity, value), expected) in fields.zip(declared) {
        if identity != &expected.id || !leaf_valid(&expected.ty, value, &mut total) {
            return Err(rejected("field identity/order/type/capacity mismatch"));
        }
    }
    if total > 8192 {
        return Err(rejected("total byte capacity exceeded"));
    }
    Ok(())
}
fn stage_input(plan: &CheckedOwnedFramePlan, input: OwnedFrameInput) -> OwnedFrameArgument {
    let ResolvedType::Nominal { declaration, .. } = &plan.function().params[0].ty else {
        unreachable!()
    };
    let declared = plan
        .program()
        .declarations
        .record_fields(declaration)
        .expect("checked fields");
    let mut fields = BTreeMap::new();
    let mut allocation = 0;
    for (field, expected) in input.fields.into_iter().zip(declared) {
        let value = match field.value {
            OwnedFrameInputValue::Bytes(bytes) => {
                allocation += 1;
                Value::Bytes(OwnedBytesValue {
                    allocation,
                    bytes: Arc::from(bytes),
                })
            }
            OwnedFrameInputValue::Scalar(scalar) => {
                super::scalar_of(&expected.ty, &scalar).expect("checked scalar")
            }
        };
        fields.insert(field.identity, value);
    }
    OwnedFrameArgument {
        plan: plan.clone(),
        root: Some(Value::Record(Arc::new(OwnedRecordValue {
            record: input.declaration,
            fields,
        }))),
    }
}

pub(crate) fn start_owned_frame(
    plan: &CheckedOwnedFramePlan,
    mut argument: OwnedFrameArgument,
    budget: &mut OwnedFrameBudget,
) -> OwnedFrameFoundationStep {
    let root = argument.root.take().expect("consumed opaque argument");
    let actual_plan = argument.plan.clone();
    if actual_plan.binding() != plan.binding() {
        return terminal(
            actual_plan,
            root,
            OwnedFrameFailure::EvaluationRejected,
            false,
        );
    }
    if budget.cancelled {
        return terminal(actual_plan, root, OwnedFrameFailure::HostAbandoned, false);
    }
    evaluate_phase(
        actual_plan,
        root,
        Environment::from(Vec::new()),
        0,
        None,
        true,
        budget,
    )
}
pub(crate) fn resume_owned_frame(
    parked: OwnedFrameParked,
    answer: ArgumentValue,
    budget: &mut OwnedFrameBudget,
) -> OwnedFrameFoundationStep {
    let OwnedFrameParked {
        plan,
        root,
        environment,
        next,
        ..
    } = parked;
    if budget.cancelled {
        return terminal(plan, root, OwnedFrameFailure::HostAbandoned, false);
    }
    let ty = &plan
        .function()
        .yields
        .as_ref()
        .expect("checked yields")
        .response_type;
    if !scalar_valid(ty, &answer) {
        return terminal(plan, root, OwnedFrameFailure::AnswerTypeMismatch, false);
    }
    let Some(answer) = super::scalar_of(ty, &answer) else {
        return terminal(plan, root, OwnedFrameFailure::AnswerTypeMismatch, false);
    };
    evaluate_phase(plan, root, environment, next, Some(answer), false, budget)
}
fn terminal(
    plan: CheckedOwnedFramePlan,
    root: Value,
    failure: OwnedFrameFailure,
    provisional: bool,
) -> OwnedFrameFoundationStep {
    OwnedFrameFoundationStep::Terminal(OwnedFrameStagedTerminal {
        plan,
        root,
        failure: Some(failure),
        provisional,
    })
}
fn failure(flow: Flow) -> OwnedFrameFailure {
    match flow {
        Flow::Failure(status) => OwnedFrameFailure::Language(status),
        Flow::Exhausted => OwnedFrameFailure::FuelExhausted,
        Flow::Cancelled { .. } => OwnedFrameFailure::HostAbandoned,
        _ => OwnedFrameFailure::EvaluationRejected,
    }
}
fn evaluate_phase(
    plan: CheckedOwnedFramePlan,
    root: Value,
    mut environment: Environment,
    next: usize,
    answer: Option<Value>,
    start: bool,
    budget: &mut OwnedFrameBudget,
) -> OwnedFrameFoundationStep {
    let entry = plan.function();
    let admitted = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&admitted),
        BTreeMap::new(),
        &plan.program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    let mut provisional = false;
    let evaluated: Result<Option<(ArgumentValue, usize)>, Flow> = (|| {
        if start {
            evaluator.charge()?; // frame entry, charged in the consuming lane
            check_contracts(&mut evaluator, entry, &root, &mut environment, true)?;
        }
        let ResolvedExprKind::Block { statements, .. } = &entry.body.kind else {
            return Err(Flow::Guard("checked body changed"));
        };
        let mut answer = answer;
        for (index, statement) in statements.iter().enumerate().skip(next) {
            let ResolvedStatement::Let { binding, value, .. } = statement else {
                return Err(Flow::Guard("checked statement changed"));
            };
            if let ResolvedExprKind::Yield { request } = &value.kind {
                if start {
                    evaluator.charge()?; // yield node
                    let produced =
                        evaluate_copy(&mut evaluator, request, entry, &root, &mut environment)?;
                    let request =
                        super::argument_of(&produced).ok_or(Flow::Guard("non-scalar request"))?;
                    return Ok(Some((request, index)));
                }
                evaluator.charge()?; // resumed yield node
                environment.push((
                    binding.id.clone(),
                    answer.take().ok_or(Flow::Guard("missing answer"))?,
                ));
            } else {
                let value = evaluate_copy(&mut evaluator, value, entry, &root, &mut environment)?;
                environment.push((binding.id.clone(), value));
            }
        }
        evaluator.charge()?; // whole identity tail transfers the retained root
        provisional = true;
        check_contracts(&mut evaluator, entry, &root, &mut environment, false)?;
        Ok(None)
    })();
    let steps = evaluator.steps;
    drop(evaluator); // no owning root was ever installed in its environment
    budget.remaining -= steps;
    budget.consumed += steps;
    match evaluated {
        Ok(Some((request, next))) => OwnedFrameFoundationStep::Parked(OwnedFrameParked {
            plan,
            root,
            environment,
            request,
            next,
        }),
        Ok(None) => OwnedFrameFoundationStep::Terminal(OwnedFrameStagedTerminal {
            plan,
            root,
            failure: None,
            provisional: true,
        }),
        Err(flow) => {
            drop(environment);
            terminal(plan, root, failure(flow), provisional)
        }
    }
}
fn check_contracts(
    evaluator: &mut Evaluator<'_>,
    function: &hir::ResolvedFunction,
    root: &Value,
    environment: &mut Environment,
    requires: bool,
) -> Result<(), Flow> {
    let clauses = if requires {
        &function.requires
    } else {
        &function.ensures
    };
    for (index, clause) in clauses.iter().enumerate() {
        evaluator.charge()?;
        match evaluate_copy(evaluator, clause, function, root, environment)? {
            Value::Bool(true) => {}
            Value::Bool(false) => {
                return Err(evaluator.contract_failure(
                    function,
                    environment,
                    if requires {
                        crate::cleanup_plan::ContractPhase::Requires
                    } else {
                        crate::cleanup_plan::ContractPhase::Ensures
                    },
                    index,
                ))
            }
            _ => return Err(Flow::Guard("non-boolean checked contract")),
        }
    }
    Ok(())
}
/// Materialize only borrowed Copy field observations into checked scalar nodes.
/// The root and Bytes leaves never pass through clone_value or a contract frame.
fn evaluate_copy(
    evaluator: &mut Evaluator<'_>,
    expression: &ResolvedExpr,
    function: &hir::ResolvedFunction,
    root: &Value,
    environment: &mut Environment,
) -> Result<Value, Flow> {
    let mut expression = expression.clone();
    project_copy(&mut expression, function, root)?;
    evaluator.evaluate(&expression, environment, 0)
}
fn project_copy(
    expression: &mut ResolvedExpr,
    function: &hir::ResolvedFunction,
    root: &Value,
) -> Result<(), Flow> {
    match &mut expression.kind {
        ResolvedExprKind::Place(place)
            if place.root == function.params[0].id || place.root == function.result_id =>
        {
            let [hir::PlaceProjection::Field(field)] = place.projections.as_slice() else {
                return Err(Flow::Guard("owning contract projection"));
            };
            let Value::Record(record) = root else {
                return Err(Flow::Guard("missing root"));
            };
            let value = record
                .fields
                .get(field)
                .ok_or(Flow::Guard("missing Copy field"))?;
            expression.kind = match value {
                Value::Int(v) => ResolvedExprKind::Int(*v),
                Value::Int32(v) => ResolvedExprKind::Int32(*v),
                Value::Uint8(v) => ResolvedExprKind::Uint8(*v),
                Value::Usize(v) => ResolvedExprKind::Usize(*v),
                Value::Bool(v) => ResolvedExprKind::Bool(*v),
                Value::Char(v) => ResolvedExprKind::Char(*v),
                Value::Float32(v) => ResolvedExprKind::Float32(v.to_bits()),
                Value::Float64(v) => ResolvedExprKind::Float64(v.to_bits()),
                _ => return Err(Flow::Guard("owned field read")),
            };
        }
        ResolvedExprKind::Unary { value, .. } => project_copy(value, function, root)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            project_copy(left, function, root)?;
            project_copy(right, function, root)?;
        }
        _ => {}
    }
    Ok(())
}
fn exclusive(root: &Value) -> bool {
    let Value::Record(record) = root else {
        return false;
    };
    Arc::strong_count(record) == 1
        && record.fields.values().all(|value| match value {
            Value::Bytes(bytes) => Arc::strong_count(&bytes.bytes) == 1,
            _ => super::clone_scalar(value).is_some(),
        })
}
fn release(
    root: &mut Option<Value>,
    actions: &[FinalizeAction],
) -> Result<OwnedFrameReleaseReceipt, Diagnostic> {
    let value = root
        .as_mut()
        .ok_or_else(|| rejected("root already consumed"))?;
    if !exclusive(value) {
        return Err(rejected("unaccounted root/leaf alias"));
    }
    let Value::Record(record) = value else {
        return Err(rejected("not record"));
    };
    let record = Arc::get_mut(record).expect("exclusive root checked before any cleanup");
    // Validate the complete vector before removing the first physical leaf.
    let expected: Vec<_> = record
        .fields
        .iter()
        .filter(|(_, v)| matches!(v, Value::Bytes(_)))
        .map(|(id, _)| id)
        .collect();
    if actions.len() != expected.len()
        || actions.iter().enumerate().any(|(i, a)| {
            a.source.projections.len() != 1
                || !expected.contains(&&a.source.projections[0])
                || actions[..i]
                    .iter()
                    .any(|old| old.source.projections == a.source.projections)
        })
    {
        return Err(rejected("cleanup leaf inventory differs"));
    }
    drop(expected);
    for action in actions {
        drop(
            record
                .fields
                .remove(&action.source.projections[0])
                .expect("validated canonical leaf"),
        );
        #[cfg(test)]
        RELEASE_OBSERVER.with(|observer| {
            if let Some(observer) = observer.borrow_mut().as_mut() {
                observer(&action.source.projections[0]);
            }
        });
    }
    drop(root.take());
    Ok(OwnedFrameReleaseReceipt {
        operations: actions.to_vec(),
    })
}
pub(crate) fn settle_owned_frame(
    terminal: OwnedFrameStagedTerminal,
) -> Result<OwnedFrameSettledOutcome, OwnedFrameSettlementRejection> {
    if !exclusive(&terminal.root) {
        return Err(OwnedFrameSettlementRejection {
            terminal,
            diagnostic: rejected("unaccounted root/leaf alias"),
        });
    }
    let OwnedFrameStagedTerminal {
        plan,
        root,
        failure,
        provisional,
    } = terminal;
    if let Some(failure) = failure {
        let actions = if provisional {
            &plan.liveness().result_disposal
        } else {
            &plan.liveness().failure_cleanup
        };
        let mut root = Some(root);
        match release(&mut root, actions) {
            Ok(receipt) => Ok(OwnedFrameSettledOutcome::Failed(failure, receipt)),
            Err(diagnostic) => Err(OwnedFrameSettlementRejection {
                terminal: OwnedFrameStagedTerminal {
                    plan,
                    root: root.expect("refused release retains root"),
                    failure: Some(failure),
                    provisional,
                },
                diagnostic,
            }),
        }
    } else {
        let receipt = OwnedFrameReleaseReceipt {
            operations: plan.liveness().completion_cleanup.clone(),
        };
        Ok(OwnedFrameSettledOutcome::Completed(
            OwnedFrameResult {
                plan,
                root: Some(root),
            },
            receipt,
        ))
    }
}
impl OwnedFrameArgument {
    fn dispose_backing(&mut self) {
        if self.root.is_some() {
            let _ = release(&mut self.root, &self.plan.liveness().failure_cleanup);
        }
    }
}
impl Drop for OwnedFrameArgument {
    fn drop(&mut self) {
        self.dispose_backing();
    }
}
impl OwnedFrameResult {
    pub(crate) fn dispose(mut self) -> Result<OwnedFrameReleaseReceipt, OwnedFrameResultRejection> {
        match release(&mut self.root, &self.plan.liveness().result_disposal) {
            Ok(receipt) => Ok(receipt),
            Err(diagnostic) => Err(OwnedFrameResultRejection {
                result: self,
                diagnostic,
            }),
        }
    }
    pub(crate) fn into_argument(
        mut self,
        plan: &CheckedOwnedFramePlan,
    ) -> Result<OwnedFrameArgument, OwnedFrameResultRejection> {
        if self.plan.function().return_type != plan.function().params[0].ty
            || !self
                .root
                .as_ref()
                .is_some_and(|root| exclusive(root) && root_matches(plan, root))
        {
            return Err(OwnedFrameResultRejection {
                result: self,
                diagnostic: rejected("result handoff identity/alias mismatch"),
            });
        }
        Ok(OwnedFrameArgument {
            plan: plan.clone(),
            root: self.root.take(),
        })
    }
}
impl Drop for OwnedFrameResult {
    fn drop(&mut self) {
        if self.root.is_some() {
            let _ = release(&mut self.root, &self.plan.liveness().result_disposal);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
thread_local! { static RELEASE_OBSERVER: std::cell::RefCell<Option<Box<dyn FnMut(&hir::DeclarationId)>>> = std::cell::RefCell::new(None); }

fn root_matches(plan: &CheckedOwnedFramePlan, root: &Value) -> bool {
    let Value::Record(record) = root else {
        return false;
    };
    let ResolvedType::Nominal { declaration, .. } = &plan.function().params[0].ty else {
        return false;
    };
    let Some(fields) = plan.program().declarations.record_fields(declaration) else {
        return false;
    };
    if record.record != *declaration || record.fields.len() != fields.len() {
        return false;
    }
    fields.iter().all(|f| match record.fields.get(&f.id) {
        Some(Value::Bytes(bytes)) => f.ty == ResolvedType::Bytes && bytes.bytes.len() <= 1024,
        Some(value) => super::argument_of(value).is_some_and(|value| scalar_valid(&f.ty, &value)),
        None => false,
    })
}
