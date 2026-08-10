//! First public WebAssembly owned-resource execution slice.
//!
//! Admission is deliberately narrow. The lowering consumes terminal cleanup
//! actions and owned-result publication from the replay-validated
//! `semaprax.cleanup-plan.v2`; unsupported shapes remain behind `SPX-W111`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::ast::BinaryOp;
use crate::cleanup_plan::{
    CleanupPlace, CleanupResultSource, CleanupTransition, ContractPhase, ExitContinuation,
    FinalizeAction, StatusProducer, StatusSourceId, StorageId,
};
use crate::conformance::TraceEventKind;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};

use crate::semantic_trace::{build_semantic_event_dictionary, SemanticEventDictionary};

use super::{write_i64, write_u32, I32, I64};

pub(super) const IMPORT_NAMES: [&str; 11] = [
    "spx_owned_begin",
    "spx_owned_stage",
    "spx_owned_abort",
    "spx_owned_reserve_result",
    "spx_owned_commit",
    "spx_owned_drop",
    "spx_owned_cancel_result",
    "spx_owned_publish",
    "spx_status_record",
    "spx_owned_success",
    "spx_semantic_event",
];

pub(super) const BEGIN_IMPORT: u32 = 7;
pub(super) const STAGE_IMPORT: u32 = 8;
pub(super) const ABORT_IMPORT: u32 = 9;
pub(super) const RESERVE_RESULT_IMPORT: u32 = 10;
pub(super) const COMMIT_IMPORT: u32 = 11;
pub(super) const DROP_IMPORT: u32 = 12;
pub(super) const CANCEL_RESULT_IMPORT: u32 = 13;
pub(super) const PUBLISH_IMPORT: u32 = 14;
pub(super) const STATUS_IMPORT: u32 = 15;
pub(super) const SUCCESS_IMPORT: u32 = 16;
pub(super) const SEMANTIC_IMPORT: u32 = 17;

const STATUS_CLASS_REQUIRES: i64 = 1;
const STATUS_CLASS_ENSURES: i64 = 2;
const STATUS_CLASS_ARITHMETIC: i64 = 3;
const STATUS_CODE_ADD_OVERFLOW: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnedResultKind {
    I64,
    Resource,
}

impl OwnedResultKind {
    pub(super) const fn manifest_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Resource => "resource",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParamKind {
    I64,
    Bool,
    Resource,
}

impl ParamKind {
    const fn wasm_type(self) -> u8 {
        match self {
            Self::I64 => I64,
            Self::Bool | Self::Resource => I32,
        }
    }

    const fn manifest_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Resource => "resource",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ScalarExpr {
    I64(i64),
    Bool(bool),
    Param { index: u32 },
    Ge(Box<Self>, Box<Self>),
    Add(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug)]
struct Guard {
    expression: ScalarExpr,
    failure_ordinal: u32,
    finalizers: Vec<FinalizerPlan>,
    phase: ContractPhase,
}

#[derive(Clone, Debug)]
struct FailurePlan {
    failure_ordinal: u32,
    finalizers: Vec<FinalizerPlan>,
}

#[derive(Clone, Debug)]
struct FinalizerPlan {
    parameter: usize,
    begin_ordinal: u32,
    end_ordinal: u32,
}

#[derive(Clone, Debug)]
enum Body {
    I64(ScalarExpr),
    OwnedParameter(usize),
}

#[derive(Clone, Debug)]
pub(super) struct OwnedPlan {
    pub(super) export: String,
    pub(super) function_index: usize,
    pub(super) result: OwnedResultKind,
    resource: String,
    lifecycle: String,
    params: Vec<ParamKind>,
    owned_params: Vec<usize>,
    function_ordinal: u32,
    function_identity: String,
    dictionary_fingerprint: [u8; 32],
    dictionary_ordinals: Vec<u32>,
    requires: Vec<Guard>,
    body: Body,
    transfer_ordinals: Vec<u32>,
    arithmetic_failure: Option<FailurePlan>,
    ensures: Vec<Guard>,
    success_finalizers: Vec<FinalizerPlan>,
    result_commit_ordinal: u32,
}

impl OwnedPlan {
    pub(super) fn signature(&self) -> (Vec<u8>, Vec<u8>) {
        let mut params = Vec::with_capacity(self.params.len() + 2);
        params.push(I32); // instance context nonce
        params.extend(self.params.iter().copied().map(ParamKind::wasm_type));
        params.push(I32); // result out pointer
        (params, vec![I32]) // status token
    }

    pub(super) fn manifest_json(&self, function: &ResolvedFunction) -> String {
        let params = self
            .params
            .iter()
            .map(|kind| quote_json(kind.manifest_name()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"function\":{},\"export\":{},\"resource\":{},\"lifecycle\":{},\"parameters\":[{}],\"result\":{}}}",
            quote_json(function.id.as_str()),
            quote_json(&self.export),
            quote_json(&self.resource),
            quote_json(&self.lifecycle),
            params,
            quote_json(self.result.manifest_name())
        )
    }

    pub(super) fn runtime_json(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|kind| quote_json(kind.manifest_name()))
            .collect::<Vec<_>>()
            .join(",");
        let mut fingerprint = String::with_capacity(64);
        for byte in &self.dictionary_fingerprint {
            write!(fingerprint, "{byte:02x}").expect("writing to a string cannot fail");
        }
        let ordinals = self
            .dictionary_ordinals
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{}:{{\"parameters\":[{}],\"result\":{},\"function\":{},\"function_ordinal\":{},\"dictionary_schema\":\"semaprax.semantic-event-dictionary.v1\",\"dictionary_fingerprint\":{},\"valid_ordinals\":[{}]}}",
            quote_json(&self.export),
            params,
            quote_json(self.result.manifest_name()),
            quote_json(&self.function_identity),
            self.function_ordinal,
            quote_json(&fingerprint),
            ordinals,
        )
    }

    pub(super) fn emit_body(&self) -> Vec<u8> {
        let original_params = self.params.len() as u32;
        let out_pointer = original_params + 1;
        let first_local = original_params + 2;
        let status_local = first_local;
        let result_local = status_local + 1;
        let left_local = result_local + 1;
        let right_local = left_local + 1;
        let first_live_local = right_local + 1;

        let mut body = Vec::new();
        // status, then result/checked-add operands, then one independent
        // liveness bit per owned parameter. Result/operands are i64 even for
        // resource results; that keeps local layout deterministic.
        write_u32(&mut body, 3);
        write_u32(&mut body, 1);
        body.push(I32);
        write_u32(&mut body, 3);
        body.push(I64);
        write_u32(&mut body, self.owned_params.len() as u32);
        body.push(I32);

        emit_import_status_guard(&mut body, BEGIN_IMPORT, &[0], status_local, None);
        emit_out_pointer_preflight(
            &mut body,
            out_pointer,
            status_local,
            match self.result {
                OwnedResultKind::I64 => 8,
                OwnedResultKind::Resource => 4,
            },
        );
        for parameter in &self.owned_params {
            emit_import_status_guard(
                &mut body,
                STAGE_IMPORT,
                &[0, *parameter as u32 + 1],
                status_local,
                Some(ABORT_IMPORT),
            );
        }
        if self.result == OwnedResultKind::Resource {
            emit_import_status_guard(
                &mut body,
                RESERVE_RESULT_IMPORT,
                &[0],
                status_local,
                Some(ABORT_IMPORT),
            );
        }
        emit_import_status_guard(
            &mut body,
            COMMIT_IMPORT,
            &[0],
            status_local,
            Some(ABORT_IMPORT),
        );
        for ordinal in 0..self.owned_params.len() {
            body.extend([0x41, 0x01, 0x21]);
            write_u32(&mut body, first_live_local + ordinal as u32);
        }

        for guard in &self.requires {
            emit_scalar_expr(&mut body, &guard.expression);
            body.push(0x45); // i32.eqz
            body.extend([0x04, 0x40]);
            emit_status_record(&mut body, guard.phase, status_local);
            emit_semantic_event(&mut body, self.function_ordinal, guard.failure_ordinal);
            emit_cleanup(
                &mut body,
                &guard.finalizers,
                &self.owned_params,
                first_live_local,
                self.function_ordinal,
            );
            emit_cancel_result(&mut body, self.result);
            emit_return_status_local(&mut body, status_local);
            body.push(0x0b);
        }

        match &self.body {
            Body::I64(ScalarExpr::Add(left, right)) => {
                emit_scalar_expr(&mut body, left);
                body.push(0x21);
                write_u32(&mut body, left_local);
                emit_scalar_expr(&mut body, right);
                body.push(0x21);
                write_u32(&mut body, right_local);
                body.push(0x20);
                write_u32(&mut body, left_local);
                body.push(0x20);
                write_u32(&mut body, right_local);
                body.push(0x7c); // i64.add
                body.push(0x21);
                write_u32(&mut body, result_local);
                // Signed overflow iff ((left ^ result) & (right ^ result)) < 0.
                body.push(0x20);
                write_u32(&mut body, left_local);
                body.push(0x20);
                write_u32(&mut body, result_local);
                body.push(0x85); // i64.xor
                body.push(0x20);
                write_u32(&mut body, right_local);
                body.push(0x20);
                write_u32(&mut body, result_local);
                body.push(0x85);
                body.push(0x83); // i64.and
                body.push(0x42);
                write_i64(&mut body, 0);
                body.push(0x53); // i64.lt_s
                body.extend([0x04, 0x40]);
                emit_raw_status_record(
                    &mut body,
                    STATUS_CLASS_ARITHMETIC,
                    STATUS_CODE_ADD_OVERFLOW,
                    status_local,
                );
                let failure = self
                    .arithmetic_failure
                    .as_ref()
                    .expect("checked add admission records a failure exit");
                emit_semantic_event(&mut body, self.function_ordinal, failure.failure_ordinal);
                emit_cleanup(
                    &mut body,
                    &failure.finalizers,
                    &self.owned_params,
                    first_live_local,
                    self.function_ordinal,
                );
                emit_cancel_result(&mut body, self.result);
                emit_return_status_local(&mut body, status_local);
                body.push(0x0b);
            }
            Body::I64(expression) => {
                emit_scalar_expr(&mut body, expression);
                body.push(0x21);
                write_u32(&mut body, result_local);
            }
            Body::OwnedParameter(_) => {}
        }

        for ordinal in &self.transfer_ordinals {
            emit_semantic_event(&mut body, self.function_ordinal, *ordinal);
        }

        for guard in &self.ensures {
            emit_scalar_expr(&mut body, &guard.expression);
            body.push(0x45);
            body.extend([0x04, 0x40]);
            emit_status_record(&mut body, guard.phase, status_local);
            emit_semantic_event(&mut body, self.function_ordinal, guard.failure_ordinal);
            emit_cleanup(
                &mut body,
                &guard.finalizers,
                &self.owned_params,
                first_live_local,
                self.function_ordinal,
            );
            emit_cancel_result(&mut body, self.result);
            emit_return_status_local(&mut body, status_local);
            body.push(0x0b);
        }

        emit_cleanup(
            &mut body,
            &self.success_finalizers,
            &self.owned_params,
            first_live_local,
            self.function_ordinal,
        );
        body.push(0x20);
        write_u32(&mut body, out_pointer);
        match &self.body {
            Body::I64(_) => {
                body.push(0x20);
                write_u32(&mut body, result_local);
                body.extend([0x37, 0x03, 0x00]); // i64.store align=8 offset=0
            }
            Body::OwnedParameter(parameter) => {
                let live_ordinal = self
                    .owned_params
                    .iter()
                    .position(|candidate| candidate == parameter)
                    .expect("admitted owned result parameter has a liveness bit");
                body.extend([0x41, 0x00, 0x21]);
                write_u32(&mut body, first_live_local + live_ordinal as u32);
                body.push(0x20);
                write_u32(&mut body, 0);
                body.push(0x20);
                write_u32(&mut body, *parameter as u32 + 1);
                body.push(0x10);
                write_u32(&mut body, PUBLISH_IMPORT);
                body.extend([0x36, 0x02, 0x00]); // i32.store align=4 offset=0
            }
        }
        emit_semantic_event(&mut body, self.function_ordinal, self.result_commit_ordinal);
        body.push(0x20);
        write_u32(&mut body, 0);
        body.push(0x10);
        write_u32(&mut body, SUCCESS_IMPORT);
        body.extend([0x41, 0x00, 0x0b]);
        body
    }
}

pub(super) fn plan(program: &ResolvedProgram) -> Result<Vec<OwnedPlan>, Diagnostic> {
    let resources = program
        .types
        .iter()
        .filter_map(|declaration| match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                Some((declaration.id.clone(), (drop.id.clone(), &drop.kind)))
            }
            ResolvedTypeDeclarationKind::Record { .. } => None,
            ResolvedTypeDeclarationKind::Variant { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    if resources.is_empty() {
        return Ok(Vec::new());
    }
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(unsupported());
    }
    if resources
        .values()
        .any(|(_, kind)| !matches!(kind, ResolvedResourceDropKind::Trivial))
    {
        return Err(unsupported());
    }
    if resources.len() != 1 {
        return Err(unsupported());
    }

    let mut plans = Vec::new();
    for (function_index, function) in program.functions.iter().enumerate() {
        let signature_contains_resource = function
            .params
            .iter()
            .any(|parameter| is_resource(&parameter.ty, &resources))
            || is_resource(&function.return_type, &resources);
        if !signature_contains_resource {
            continue;
        }
        plans.push(plan_function(
            program,
            function,
            function_index,
            &resources,
            plans.len(),
        )?);
    }
    Ok(plans)
}

fn plan_function(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    function_index: usize,
    resources: &BTreeMap<
        crate::hir::DeclarationId,
        (crate::hir::DeclarationId, &ResolvedResourceDropKind),
    >,
    export_ordinal: usize,
) -> Result<OwnedPlan, Diagnostic> {
    let dictionary = build_semantic_event_dictionary(program, &function.id)?;
    let function_ordinal = u32::try_from(export_ordinal)
        .ok()
        .and_then(|ordinal| ordinal.checked_add(1))
        .filter(|ordinal| *ordinal <= i32::MAX as u32)
        .ok_or_else(unsupported)?;
    let mut params = Vec::new();
    let mut owned_params = Vec::new();
    let mut param_by_id = BTreeMap::<ValueId, usize>::new();
    for (index, parameter) in function.params.iter().enumerate() {
        let kind = match &parameter.ty {
            ResolvedType::I64 if parameter.ownership == OwnershipMode::Value => ParamKind::I64,
            ResolvedType::Bool if parameter.ownership == OwnershipMode::Value => ParamKind::Bool,
            ty if is_resource(ty, resources) && parameter.ownership == OwnershipMode::Own => {
                owned_params.push(index);
                ParamKind::Resource
            }
            _ => return Err(unsupported()),
        };
        params.push(kind);
        param_by_id.insert(parameter.id.clone(), index);
    }
    if owned_params.is_empty() || !function.effects.is_empty() {
        return Err(unsupported());
    }

    let entry_params = function
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .iter()
        .map(|place| param_for_place(place, &param_by_id, None))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if entry_params != owned_params.iter().copied().collect() {
        return Err(unsupported());
    }

    let status_exits = function
        .cleanup_plan
        .exits
        .iter()
        .filter_map(|exit| match &exit.continuation {
            ExitContinuation::ReturnFailure { source } => {
                Some((source.clone(), exit.finalize_in_order.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let success = function
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match &exit.continuation {
            ExitContinuation::CommitResult { source } => Some((source, &exit.finalize_in_order)),
            _ => None,
        })
        .ok_or_else(unsupported)?;

    let result_param = match &function.return_type {
        ResolvedType::I64 => None,
        ty if is_resource(ty, resources) => {
            let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
                return Err(unsupported());
            };
            if !statements.is_empty() {
                return Err(unsupported());
            }
            let ResolvedExprKind::Place(place) = &tail.kind else {
                return Err(unsupported());
            };
            if !place.projections.is_empty() {
                return Err(unsupported());
            }
            let index = param_by_id
                .get(&place.root)
                .copied()
                .ok_or_else(unsupported)?;
            if function.params[index].ownership != OwnershipMode::Own
                || function.params[index].ty != function.return_type
                || !matches!(success.0, CleanupResultSource::Owned { .. })
            {
                return Err(unsupported());
            }
            Some(index)
        }
        _ => return Err(unsupported()),
    };

    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut arithmetic_failure = None;
    for source in &function.cleanup_plan.status_sources {
        let finalizers = status_exits.get(&source.id).ok_or_else(unsupported)?;
        let mapped = map_finalizers(
            finalizers,
            &param_by_id,
            result_param,
            resources,
            function,
            &dictionary,
        )?;
        let failure_ordinal = ordinal_for_failure(&dictionary, &source.id)?;
        match &source.producer {
            StatusProducer::ContractFalse { phase, ordinal } => {
                let expression = match phase {
                    ContractPhase::Requires => function.requires.get(*ordinal as usize),
                    ContractPhase::Ensures => function.ensures.get(*ordinal as usize),
                }
                .ok_or_else(unsupported)?;
                let guard = Guard {
                    expression: scalar_expr(expression, &param_by_id)?,
                    failure_ordinal,
                    finalizers: mapped,
                    phase: *phase,
                };
                match phase {
                    ContractPhase::Requires => requires.push(guard),
                    ContractPhase::Ensures => ensures.push(guard),
                }
            }
            StatusProducer::CheckedArithmetic { operation, .. }
                if source.id.expression == body_tail(function)?.id
                    && matches!(operation, crate::cleanup_plan::CheckedOperation::Add) =>
            {
                if arithmetic_failure
                    .replace(FailurePlan {
                        failure_ordinal,
                        finalizers: mapped,
                    })
                    .is_some()
                {
                    return Err(unsupported());
                }
            }
            StatusProducer::CheckedArithmetic { .. } | StatusProducer::PropagatedCall { .. } => {
                return Err(unsupported())
            }
        }
    }

    let body = match result_param {
        Some(index) => Body::OwnedParameter(index),
        None => {
            let expression = scalar_expr(body_tail(function)?, &param_by_id)?;
            if matches!(&expression, ScalarExpr::Add(_, _)) && arithmetic_failure.is_none() {
                return Err(unsupported());
            }
            Body::I64(expression)
        }
    };
    let transfer_ordinals = transfer_ordinals(function, result_param, &dictionary)?;
    let success_finalizers = map_finalizers(
        success.1,
        &param_by_id,
        result_param,
        resources,
        function,
        &dictionary,
    )?;
    validate_terminal_coverage(&owned_params, result_param, &success_finalizers)?;
    let result_commit_ordinal = dictionary
        .ordinal_for(&TraceEventKind::ResultCommit {
            source: success.0.clone(),
        })
        .ok_or_else(unsupported)?;

    Ok(OwnedPlan {
        export: format!("semaprax_owned_{export_ordinal}"),
        function_index,
        result: if result_param.is_some() {
            OwnedResultKind::Resource
        } else {
            OwnedResultKind::I64
        },
        resource: resources
            .keys()
            .next()
            .expect("owned admission proved one resource")
            .as_str()
            .to_owned(),
        lifecycle: resources
            .values()
            .next()
            .expect("owned admission proved one lifecycle")
            .0
            .as_str()
            .to_owned(),
        params,
        owned_params,
        function_ordinal,
        function_identity: function.id.as_str().to_owned(),
        dictionary_fingerprint: dictionary.fingerprint(),
        dictionary_ordinals: dictionary
            .entries()
            .iter()
            .map(|entry| entry.ordinal)
            .collect(),
        requires,
        body,
        transfer_ordinals,
        arithmetic_failure,
        ensures,
        success_finalizers,
        result_commit_ordinal,
    })
}

fn body_tail(function: &ResolvedFunction) -> Result<&ResolvedExpr, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(unsupported());
    };
    if !statements.is_empty() {
        return Err(unsupported());
    }
    Ok(tail)
}

fn scalar_expr(
    expression: &ResolvedExpr,
    params: &BTreeMap<ValueId, usize>,
) -> Result<ScalarExpr, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(ScalarExpr::I64(*value)),
        ResolvedExprKind::Bool(value) => Ok(ScalarExpr::Bool(*value)),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => {
            let index = params.get(&place.root).copied().ok_or_else(unsupported)?;
            if !matches!(expression.ty, ResolvedType::I64 | ResolvedType::Bool) {
                return Err(unsupported());
            }
            Ok(ScalarExpr::Param {
                index: index as u32,
            })
        }
        ResolvedExprKind::Binary { op, left, right }
            if matches!(op, BinaryOp::Ge | BinaryOp::Add) =>
        {
            let left = Box::new(scalar_expr(left, params)?);
            let right = Box::new(scalar_expr(right, params)?);
            Ok(if *op == BinaryOp::Ge {
                ScalarExpr::Ge(left, right)
            } else {
                ScalarExpr::Add(left, right)
            })
        }
        _ => Err(unsupported()),
    }
}

fn map_finalizers(
    actions: &[FinalizeAction],
    params: &BTreeMap<ValueId, usize>,
    result_param: Option<usize>,
    resources: &BTreeMap<
        crate::hir::DeclarationId,
        (crate::hir::DeclarationId, &ResolvedResourceDropKind),
    >,
    function: &ResolvedFunction,
    dictionary: &SemanticEventDictionary,
) -> Result<Vec<FinalizerPlan>, Diagnostic> {
    let mut mapped = Vec::new();
    let mut seen = BTreeSet::new();
    for action in actions {
        let parameter = param_for_place(&action.source, params, result_param)?;
        if !seen.insert(parameter) {
            return Err(unsupported());
        }
        let declaration = match &function.params[parameter].ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => declaration,
            _ => return Err(unsupported()),
        };
        let (lifecycle, kind) = resources.get(declaration).ok_or_else(unsupported)?;
        if lifecycle != &action.lifecycle_id || !matches!(kind, ResolvedResourceDropKind::Trivial) {
            return Err(unsupported());
        }
        let begin_ordinal = dictionary
            .ordinal_for(&TraceEventKind::FinalizeBegin {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: None,
            })
            .ok_or_else(unsupported)?;
        let end_ordinal = dictionary
            .ordinal_for(&TraceEventKind::FinalizeEnd {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: None,
            })
            .ok_or_else(unsupported)?;
        mapped.push(FinalizerPlan {
            parameter,
            begin_ordinal,
            end_ordinal,
        });
    }
    Ok(mapped)
}

fn param_for_place(
    place: &CleanupPlace,
    params: &BTreeMap<ValueId, usize>,
    result_param: Option<usize>,
) -> Result<usize, Diagnostic> {
    if !place.projections.is_empty() {
        return Err(unsupported());
    }
    match &place.storage {
        StorageId::Value(value) => params.get(value).copied().ok_or_else(unsupported),
        StorageId::ProvisionalResult => result_param.ok_or_else(unsupported),
        StorageId::Temporary(_) | StorageId::CallArgument { .. } => Err(unsupported()),
    }
}

fn validate_terminal_coverage(
    owned: &[usize],
    result: Option<usize>,
    finalized: &[FinalizerPlan],
) -> Result<(), Diagnostic> {
    let expected = owned
        .iter()
        .copied()
        .filter(|parameter| Some(*parameter) != result)
        .collect::<BTreeSet<_>>();
    if finalized
        .iter()
        .map(|finalizer| finalizer.parameter)
        .collect::<BTreeSet<_>>()
        != expected
        || finalized.len() != expected.len()
    {
        return Err(unsupported());
    }
    Ok(())
}

fn ordinal_for_failure(
    dictionary: &SemanticEventDictionary,
    source: &StatusSourceId,
) -> Result<u32, Diagnostic> {
    let mut matches = dictionary.entries().iter().filter_map(|entry| {
        matches!(
            &entry.event,
            TraceEventKind::SelectFailure {
                source: candidate,
                ..
            } if candidate == source
        )
        .then_some(entry.ordinal)
    });
    let ordinal = matches.next().ok_or_else(unsupported)?;
    if matches.next().is_some() {
        return Err(unsupported());
    }
    Ok(ordinal)
}

fn transfer_ordinals(
    function: &ResolvedFunction,
    result_param: Option<usize>,
    dictionary: &SemanticEventDictionary,
) -> Result<Vec<u32>, Diagnostic> {
    let transfers = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::Transfer {
                at,
                source,
                destination,
            } => Some((at, source, destination)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some(parameter_index) = result_param else {
        return if transfers.is_empty() {
            Ok(Vec::new())
        } else {
            Err(unsupported())
        };
    };
    let mut current = CleanupPlace {
        storage: StorageId::Value(function.params[parameter_index].id.clone()),
        projections: Vec::new(),
    };
    let mut ordinals = Vec::with_capacity(transfers.len());
    for (at, source, destination) in transfers {
        if source != &current {
            return Err(unsupported());
        }
        let event = TraceEventKind::Transfer {
            at: at.clone(),
            source: source.clone(),
            destination: destination.clone(),
        };
        ordinals.push(dictionary.ordinal_for(&event).ok_or_else(unsupported)?);
        current = destination.clone();
    }
    if current
        != (CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: Vec::new(),
        })
        || ordinals.is_empty()
    {
        return Err(unsupported());
    }
    Ok(ordinals)
}

fn is_resource(
    ty: &ResolvedType,
    resources: &BTreeMap<
        crate::hir::DeclarationId,
        (crate::hir::DeclarationId, &ResolvedResourceDropKind),
    >,
) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments } if arguments.is_empty() && resources.contains_key(declaration))
}

fn emit_scalar_expr(output: &mut Vec<u8>, expression: &ScalarExpr) {
    match expression {
        ScalarExpr::I64(value) => {
            output.push(0x42);
            write_i64(output, *value);
        }
        ScalarExpr::Bool(value) => output.extend([0x41, u8::from(*value)]),
        ScalarExpr::Param { index } => {
            output.push(0x20);
            write_u32(output, index + 1);
        }
        ScalarExpr::Ge(left, right) => {
            emit_scalar_expr(output, left);
            emit_scalar_expr(output, right);
            output.push(0x59); // i64.ge_s
        }
        ScalarExpr::Add(left, right) => {
            // Only the body add reaches emission; contract-side checked
            // arithmetic remains outside this first slice.
            emit_scalar_expr(output, left);
            emit_scalar_expr(output, right);
            output.push(0x7c);
        }
    }
}

fn emit_import_status_guard(
    output: &mut Vec<u8>,
    import: u32,
    locals: &[u32],
    status_local: u32,
    abort: Option<u32>,
) {
    for local in locals {
        output.push(0x20);
        write_u32(output, *local);
    }
    output.push(0x10);
    write_u32(output, import);
    output.push(0x22);
    write_u32(output, status_local);
    output.extend([0x04, 0x40]);
    if let Some(abort) = abort {
        output.push(0x20);
        write_u32(output, 0);
        output.push(0x10);
        write_u32(output, abort);
    }
    output.push(0x20);
    write_u32(output, status_local);
    output.push(0x0f);
    output.push(0x0b);
}

fn emit_out_pointer_preflight(
    output: &mut Vec<u8>,
    out_pointer: u32,
    status_local: u32,
    width: i64,
) {
    debug_assert!(matches!(width, 4 | 8));
    output.push(0x20); // local.get out_pointer
    write_u32(output, out_pointer);
    output.push(0x41); // i32.const alignment mask
    write_i64(output, width - 1);
    output.extend([0x71, 0x45]); // i32.and; i32.eqz

    output.push(0x20); // local.get out_pointer
    write_u32(output, out_pointer);
    output.push(0xad); // i64.extend_i32_u
    output.push(0x42); // i64.const width
    write_i64(output, width);
    output.push(0x7c); // i64.add
    output.extend([0x3f, 0x00]); // memory.size 0
    output.push(0xad); // i64.extend_i32_u
    output.push(0x42); // i64.const WebAssembly page size
    write_i64(output, 65_536);
    output.push(0x7e); // i64.mul
    output.push(0x58); // i64.le_u
    output.push(0x71); // i32.and
    output.push(0x45); // i32.eqz: enter failure branch when invalid
    output.extend([0x04, 0x40]); // if (no result)
    output.push(0x20);
    write_u32(output, 0); // context
    output.push(0x41);
    write_i64(output, 4); // adapter status class
    output.push(0x41);
    write_i64(output, 6); // invalid out range/alignment
    output.push(0x10);
    write_u32(output, STATUS_IMPORT);
    output.push(0x21);
    write_u32(output, status_local);
    output.push(0x20);
    write_u32(output, 0);
    output.push(0x10);
    write_u32(output, ABORT_IMPORT);
    output.push(0x20);
    write_u32(output, status_local);
    output.push(0x0f); // return
    output.push(0x0b); // end
}

fn emit_cleanup(
    output: &mut Vec<u8>,
    finalizers: &[FinalizerPlan],
    owned_params: &[usize],
    first_live_local: u32,
    function_ordinal: u32,
) {
    for finalizer in finalizers {
        let parameter = finalizer.parameter;
        let ordinal = owned_params
            .iter()
            .position(|candidate| *candidate == parameter)
            .expect("admitted cleanup action has an owned-parameter liveness bit");
        output.push(0x20);
        write_u32(output, first_live_local + ordinal as u32);
        output.extend([0x04, 0x40, 0x41, 0x00, 0x21]);
        write_u32(output, first_live_local + ordinal as u32);
        emit_semantic_event(output, function_ordinal, finalizer.begin_ordinal);
        output.push(0x20);
        write_u32(output, 0);
        output.push(0x20);
        write_u32(output, parameter as u32 + 1);
        output.push(0x10);
        write_u32(output, DROP_IMPORT);
        emit_semantic_event(output, function_ordinal, finalizer.end_ordinal);
        output.push(0x0b);
    }
}

fn emit_semantic_event(output: &mut Vec<u8>, function_ordinal: u32, event_ordinal: u32) {
    debug_assert!(function_ordinal != 0 && event_ordinal != 0);
    output.push(0x20);
    write_u32(output, 0);
    output.push(0x41);
    write_i64(output, i64::from(function_ordinal));
    output.push(0x41);
    write_i64(output, i64::from(event_ordinal));
    output.push(0x10);
    write_u32(output, SEMANTIC_IMPORT);
}

fn emit_status_record(output: &mut Vec<u8>, phase: ContractPhase, status_local: u32) {
    let (class, code) = match phase {
        ContractPhase::Requires => (STATUS_CLASS_REQUIRES, 1),
        ContractPhase::Ensures => (STATUS_CLASS_ENSURES, 2),
    };
    emit_raw_status_record(output, class, code, status_local);
}

fn emit_cancel_result(output: &mut Vec<u8>, result: OwnedResultKind) {
    if result == OwnedResultKind::Resource {
        output.push(0x20);
        write_u32(output, 0);
        output.push(0x10);
        write_u32(output, CANCEL_RESULT_IMPORT);
    }
}

fn emit_raw_status_record(output: &mut Vec<u8>, class: i64, code: i64, status_local: u32) {
    output.push(0x20);
    write_u32(output, 0);
    output.push(0x41);
    write_i64(output, class);
    output.push(0x41);
    write_i64(output, code);
    output.push(0x10);
    write_u32(output, STATUS_IMPORT);
    output.push(0x21);
    write_u32(output, status_local);
}

fn emit_return_status_local(output: &mut Vec<u8>, status_local: u32) {
    output.push(0x20);
    write_u32(output, status_local);
    output.push(0x0f);
}

fn unsupported() -> Diagnostic {
    Diagnostic::io(
        "SPX-W111",
        "WebAssembly resource lowering requires the verified cleanup ABI; semaprax.wasm-owned.v1 currently admits one exact direct trivial-resource identity with its verified signature and cleanup-plan shape",
    )
}
