//! Staged value execution for the first native direct-resource corpus.
//!
//! This module is deliberately disconnected from the public native entry
//! point.  It binds physical C values to an already classified cleanup plan;
//! it never rebuilds ownership transitions from HIR.  The public `SPX-B104`
//! resource gate remains closed until this plan is composed with the cleanup
//! and trace emitters and passes the complete backend conformance gate.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "native resource value lowering remains gated")
)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use crate::ast::BinaryOp;
use crate::cleanup_plan::{
    BlockId, CleanupResultSource, CleanupTerminator, CleanupTransition, ContractPhase,
    EdgeCondition, ExitContinuation, StatusCase, StatusLane, StatusProducer, StatusSourceId,
    StorageId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    ExpressionId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};

use super::native_cleanup::{NativeCleanupAdmission, NativeCleanupIndex, NativeCleanupSlot};
use super::native_cleanup_emit::NativeCleanupBindings;
use super::native_resource::NativeResourceAbi;
use super::native_trace;

/// A typed C declaration required before entering the cleanup CFG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeValueDeclaration {
    ResourceStorage {
        storage: StorageId,
        c_type: String,
        binding: String,
        initializer: NativeResourceInitializer,
    },
    Scalar {
        expression: ExpressionId,
        c_type: &'static str,
        binding: String,
    },
    Status {
        source: StatusSourceId,
        binding: String,
    },
}

/// Resource storage is either seeded from its unique owned parameter or is
/// initially empty.  No arbitrary C expression crosses this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeResourceInitializer {
    OwnedParameter { index: usize, binding: String },
    Zero,
}

/// One value-producing operation attached to the exact cleanup block in which
/// its HIR expression executes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeValueStep {
    I64Literal {
        destination: String,
        value: i64,
    },
    BoolLiteral {
        destination: String,
        value: bool,
    },
    Copy {
        destination: String,
        source: String,
    },
    CheckedAdd {
        destination: String,
        left: String,
        right: String,
        status: String,
    },
    CompareI64Ge {
        destination: String,
        left: String,
        right: String,
    },
    RecordContractFailure {
        condition: String,
        status: String,
        phase: ContractPhase,
        function_name: String,
        expression_label: String,
    },
}

/// Result mapping already proven against the exact HIR and cleanup transfers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NativeValueResult {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        parameter: ValueId,
        owner_ordinal: usize,
    },
}

/// Complete staged value-side input for the cleanup emitter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeValuePlan {
    function_id: crate::hir::DeclarationId,
    cleanup_admission: NativeCleanupAdmission,
    pub(crate) cleanup_bindings: NativeCleanupBindings,
    pub(crate) required_event_capacity: u32,
    pub(crate) declarations: Vec<NativeValueDeclaration>,
    pub(crate) block_steps: BTreeMap<BlockId, Vec<NativeValueStep>>,
    result: NativeValueResult,
}

/// Build the exact value plan for the currently exercised single-frame corpus.
///
/// Supported forms are intentionally narrow: direct trivial resources passed
/// by `own`, scalar value parameters, an `i64` or direct-resource result, an
/// empty root block, scalar `requires` observations, checked `i64` addition,
/// scalar literals, owned-parameter identity, and literal-false `ensures`.
pub(crate) fn plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    cleanup: &NativeCleanupIndex<'_>,
    abi: &NativeResourceAbi,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<NativeValuePlan, Diagnostic> {
    if !cleanup.belongs_to(function) {
        return Err(value_error("cleanup index belongs to a different function"));
    }
    validate_signature(program, function, abi)?;

    let mut planner = Planner {
        program,
        function,
        cleanup,
        abi,
        contract_labels,
        current: cleanup.entry(),
        cleanup_bindings: NativeCleanupBindings {
            context: "spx_bind_context".to_owned(),
            result_out: Some("spx_bind_result_out".to_owned()),
            ..NativeCleanupBindings::default()
        },
        declarations: Vec::new(),
        block_steps: BTreeMap::new(),
        values: BTreeMap::new(),
        expression_ids: BTreeSet::new(),
        consumed_statuses: BTreeSet::new(),
        next_scalar: 0,
    };
    planner.seed_parameters_and_storage()?;

    for (ordinal, contract) in function.requires.iter().enumerate() {
        planner.lower_contract(contract, ContractPhase::Requires, ordinal)?;
    }

    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(value_error("function body is not the canonical root block"));
    };
    if !statements.is_empty() {
        return Err(value_error("root block contains unsupported statements"));
    }
    let body_value = planner.lower_body_tail(tail)?;

    let result = match &function.return_type {
        ResolvedType::Unit => {
            return Err(value_error(
                "unit result is outside the ordinary native value corpus",
            ));
        }
        ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::F32
        | ResolvedType::F64 => {
            return Err(value_error(
                "non-i64 scalar result is outside the staged single-frame value corpus",
            ));
        }
        ResolvedType::String | ResolvedType::Str => {
            return Err(value_error(
                "text result is outside the staged single-frame value corpus",
            ));
        }
        ResolvedType::I64 => {
            let result = planner.new_scalar(&function.body.id, "int64_t")?;
            planner.push(NativeValueStep::Copy {
                destination: result.clone(),
                source: body_value,
            });
            planner
                .cleanup_bindings
                .scalar_results
                .insert(function.body.id.clone(), result);
            NativeValueResult::ScalarI64
        }
        ResolvedType::Nominal { .. } => {
            let (parameter_index, parameter, owner_ordinal) = planner.validate_owned_tail(tail)?;
            planner.validate_owned_body_transfers(tail)?;
            NativeValueResult::OwnedInput {
                parameter_index,
                parameter,
                owner_ordinal,
            }
        }
        ResolvedType::Bool | ResolvedType::TypeParameter { .. } => {
            return Err(value_error("result type is outside the staged corpus"));
        }
    };

    for (ordinal, contract) in function.ensures.iter().enumerate() {
        if !matches!(contract.kind, ResolvedExprKind::Bool(false)) {
            return Err(value_error(
                "only literal-false ensures is staged in this corpus",
            ));
        }
        planner.lower_contract(contract, ContractPhase::Ensures, ordinal)?;
    }
    planner.validate_success_exit()?;

    if planner.consumed_statuses.len() != cleanup.status_sources().len() {
        return Err(value_error(
            "cleanup plan contains an unconsumed status source",
        ));
    }

    Ok(NativeValuePlan {
        function_id: function.id.clone(),
        cleanup_admission: cleanup.admission(),
        cleanup_bindings: planner.cleanup_bindings,
        required_event_capacity: native_trace::required_event_capacity(program, function)?,
        declarations: planner.declarations,
        block_steps: planner.block_steps,
        result,
    })
}

impl NativeValuePlan {
    /// Prove that this value proof shares the private cleanup admission
    /// capability for this canonical function. The capability never enters a
    /// template or generated artifact.
    pub(crate) fn belongs_to(
        &self,
        function: &ResolvedFunction,
        cleanup: &NativeCleanupIndex<'_>,
    ) -> bool {
        self.function_id == function.id && self.cleanup_admission.matches(&cleanup.admission())
    }

    pub(crate) fn result(&self) -> &NativeValueResult {
        &self.result
    }
}

/// Emit deterministic top-of-function declarations. Parameter bindings named
/// by [`NativeResourceInitializer::OwnedParameter`] belong to the surrounding
/// function signature and are not redeclared here.
pub(crate) fn emit_declarations(plan: &NativeValuePlan) -> String {
    let mut output = String::new();
    for declaration in &plan.declarations {
        match declaration {
            NativeValueDeclaration::ResourceStorage {
                c_type,
                binding,
                initializer,
                ..
            } => {
                let initializer = match initializer {
                    NativeResourceInitializer::OwnedParameter { binding, .. } => binding.as_str(),
                    NativeResourceInitializer::Zero => "{0}",
                };
                writeln!(output, "{c_type} {binding} = {initializer};")
                    .expect("writing to a string cannot fail");
            }
            NativeValueDeclaration::Scalar {
                c_type, binding, ..
            } => {
                writeln!(output, "{c_type} {binding} = {{0}};")
                    .expect("writing to a string cannot fail");
            }
            NativeValueDeclaration::Status { binding, .. } => {
                writeln!(output, "spx_status_token {binding} = SPX_STATUS_SUCCESS;")
                    .expect("writing to a string cannot fail");
            }
        }
    }
    output
}

/// Emit the operations that must execute on entry to one cleanup block.
pub(crate) fn emit_block_prologue(plan: &NativeValuePlan, block: BlockId) -> String {
    let mut output = String::new();
    for step in plan.block_steps.get(&block).into_iter().flatten() {
        match step {
            NativeValueStep::I64Literal { destination, value } => {
                writeln!(output, "{destination} = {};", super::c_i64(*value))
                    .expect("writing to a string cannot fail");
            }
            NativeValueStep::BoolLiteral { destination, value } => {
                writeln!(output, "{destination} = {value};")
                    .expect("writing to a string cannot fail");
            }
            NativeValueStep::Copy {
                destination,
                source,
            } => {
                writeln!(output, "{destination} = {source};")
                    .expect("writing to a string cannot fail");
            }
            NativeValueStep::CheckedAdd {
                destination,
                left,
                right,
                status,
            } => {
                writeln!(
                    output,
                    "{status} = spx_rt_add({}, {left}, {right}, &{destination});",
                    plan.cleanup_bindings.context
                )
                .expect("writing to a string cannot fail");
            }
            NativeValueStep::CompareI64Ge {
                destination,
                left,
                right,
            } => {
                writeln!(output, "{destination} = ({left} >= {right});")
                    .expect("writing to a string cannot fail");
            }
            NativeValueStep::RecordContractFailure {
                condition,
                status,
                phase,
                function_name,
                expression_label,
            } => {
                let (code, kind) = match phase {
                    ContractPhase::Requires => ("SPX_STATUS_CONTRACT_REQUIRES_FALSE", "requires"),
                    ContractPhase::Ensures => ("SPX_STATUS_CONTRACT_ENSURES_FALSE", "ensures"),
                };
                writeln!(output, "if (!{condition}) {{").expect("writing cannot fail");
                writeln!(
                    output,
                    "    {status} = spx_rt_contract({}, {code}, \"{kind}\", \"{}\", \"{}\");",
                    plan.cleanup_bindings.context,
                    c_string(function_name),
                    c_string(expression_label)
                )
                .expect("writing to a string cannot fail");
                output.push_str("}\n");
            }
        }
    }
    output
}

struct BoundValue {
    name: String,
    ty: ResolvedType,
}

struct Planner<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    cleanup: &'a NativeCleanupIndex<'a>,
    abi: &'a NativeResourceAbi,
    contract_labels: &'a HashMap<ExpressionId, String>,
    current: BlockId,
    cleanup_bindings: NativeCleanupBindings,
    declarations: Vec<NativeValueDeclaration>,
    block_steps: BTreeMap<BlockId, Vec<NativeValueStep>>,
    values: BTreeMap<ValueId, BoundValue>,
    expression_ids: BTreeSet<ExpressionId>,
    consumed_statuses: BTreeSet<StatusSourceId>,
    next_scalar: usize,
}

impl Planner<'_> {
    fn seed_parameters_and_storage(&mut self) -> Result<(), Diagnostic> {
        for (index, parameter) in self.function.params.iter().enumerate() {
            let name = format!("spx_bind_param_{index}");
            if self
                .values
                .insert(
                    parameter.id.clone(),
                    BoundValue {
                        name,
                        ty: parameter.ty.clone(),
                    },
                )
                .is_some()
            {
                return Err(value_error("parameter identities are not unique"));
            }
        }

        for indexed in self.cleanup.slots() {
            let binding = format!("spx_bind_slot_{}", indexed.slot.id.0);
            let initializer = self.storage_initializer(indexed)?;
            self.cleanup_bindings
                .storage_values
                .insert(indexed.slot.storage.clone(), binding.clone());
            self.declarations
                .push(NativeValueDeclaration::ResourceStorage {
                    storage: indexed.slot.storage.clone(),
                    c_type: self.abi.c_type(self.program, &indexed.slot.ty)?.to_owned(),
                    binding,
                    initializer,
                });
        }
        Ok(())
    }

    fn storage_initializer(
        &self,
        indexed: &NativeCleanupSlot<'_>,
    ) -> Result<NativeResourceInitializer, Diagnostic> {
        match &indexed.slot.storage {
            StorageId::Value(value) => {
                let (index, parameter) = self
                    .function
                    .params
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.id == *value)
                    .ok_or_else(|| {
                        value_error("value cleanup storage is not an owned parameter")
                    })?;
                if parameter.ownership != OwnershipMode::Own || parameter.ty != indexed.slot.ty {
                    return Err(value_error(
                        "value cleanup storage disagrees with its owned parameter",
                    ));
                }
                Ok(NativeResourceInitializer::OwnedParameter {
                    index,
                    binding: format!("spx_bind_param_{index}"),
                })
            }
            StorageId::Temporary(_) | StorageId::ProvisionalResult => {
                Ok(NativeResourceInitializer::Zero)
            }
            StorageId::CallArgument { .. } => Err(value_error(
                "call-argument storage is outside the single-frame corpus",
            )),
        }
    }

    fn lower_contract(
        &mut self,
        contract: &ResolvedExpr,
        phase: ContractPhase,
        ordinal: usize,
    ) -> Result<(), Diagnostic> {
        self.current = self.follow_always_goto(self.current)?;
        let condition = self.lower_contract_value(contract)?;
        let source = StatusSourceId {
            expression: contract.id.clone(),
            lane: StatusLane::ContractFalse,
        };
        let semantic = self
            .cleanup
            .status_sources()
            .iter()
            .find(|candidate| candidate.id == source)
            .ok_or_else(|| value_error("contract has no exact cleanup status source"))?;
        if semantic.producer
            != (StatusProducer::ContractFalse {
                phase,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| value_error("contract ordinal does not fit the runtime ABI"))?,
            })
        {
            return Err(value_error(
                "contract status producer disagrees with HIR order",
            ));
        }
        let status = self.status_binding(&source)?;
        self.cleanup_bindings
            .boolean_values
            .insert(contract.id.clone(), condition.clone());
        self.push(NativeValueStep::RecordContractFailure {
            condition,
            status,
            phase,
            function_name: self.function.name.clone(),
            expression_label: self
                .contract_labels
                .get(&contract.id)
                .cloned()
                .unwrap_or_else(|| contract.id.as_str().to_owned()),
        });
        let success = self.boolean_branch(self.current, &contract.id, &source)?;
        self.current = self.follow_success_continuation(success)?;
        Ok(())
    }

    fn lower_contract_value(&mut self, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
        if expression.ty != ResolvedType::Bool || expression.ownership != OwnershipMode::Value {
            return Err(value_error(
                "contract observation is not a scalar bool value",
            ));
        }
        match &expression.kind {
            ResolvedExprKind::Bool(value) => {
                let destination = self.new_scalar(&expression.id, "bool")?;
                self.push(NativeValueStep::BoolLiteral {
                    destination: destination.clone(),
                    value: *value,
                });
                Ok(destination)
            }
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                let source = self.scalar_place(place.root.clone(), &ResolvedType::Bool)?;
                let destination = self.new_scalar(&expression.id, "bool")?;
                self.push(NativeValueStep::Copy {
                    destination: destination.clone(),
                    source,
                });
                Ok(destination)
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::Ge,
                left,
                right,
            } => {
                let left = self.lower_i64_operand(left)?;
                let right = self.lower_i64_operand(right)?;
                let destination = self.new_scalar(&expression.id, "bool")?;
                self.push(NativeValueStep::CompareI64Ge {
                    destination: destination.clone(),
                    left,
                    right,
                });
                Ok(destination)
            }
            _ => Err(value_error(
                "contract expression is outside the bool-parameter/literal corpus",
            )),
        }
    }

    fn lower_body_tail(&mut self, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::Int(value)
                if expression.ty == ResolvedType::I64
                    && expression.ownership == OwnershipMode::Value =>
            {
                let destination = self.new_scalar(&expression.id, "int64_t")?;
                self.push(NativeValueStep::I64Literal {
                    destination: destination.clone(),
                    value: *value,
                });
                Ok(destination)
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } if expression.ty == ResolvedType::I64
                && expression.ownership == OwnershipMode::Value =>
            {
                let left = self.lower_i64_operand(left)?;
                let right = self.lower_i64_operand(right)?;
                let destination = self.new_scalar(&expression.id, "int64_t")?;
                let source = StatusSourceId {
                    expression: expression.id.clone(),
                    lane: StatusLane::OperationFailure,
                };
                let semantic = self
                    .cleanup
                    .status_sources()
                    .iter()
                    .find(|candidate| candidate.id == source)
                    .ok_or_else(|| value_error("checked add has no exact cleanup status source"))?;
                if semantic.producer
                    != (StatusProducer::CheckedArithmetic {
                        operation: crate::cleanup_plan::CheckedOperation::Add,
                        normalized_cases: vec![StatusCase::AddOverflow],
                    })
                {
                    return Err(value_error("checked-add status producer is inconsistent"));
                }
                let status = self.status_binding(&source)?;
                self.push(NativeValueStep::CheckedAdd {
                    destination: destination.clone(),
                    left,
                    right,
                    status,
                });
                self.current = self.status_success_branch(self.current, &source)?;
                Ok(destination)
            }
            ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && expression.ownership == OwnershipMode::Own
                    && matches!(expression.ty, ResolvedType::Nominal { .. }) =>
            {
                self.values
                    .get(&place.root)
                    .filter(|value| value.ty == expression.ty)
                    .map(|value| value.name.clone())
                    .ok_or_else(|| value_error("owned tail is not an exact parameter place"))
            }
            _ => Err(value_error("body tail is outside the staged value corpus")),
        }
    }

    fn lower_i64_operand(&mut self, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
        if expression.ty != ResolvedType::I64 || expression.ownership != OwnershipMode::Value {
            return Err(value_error("checked-add operand is not an i64 value"));
        }
        match &expression.kind {
            ResolvedExprKind::Int(value) => {
                let destination = self.new_scalar(&expression.id, "int64_t")?;
                self.push(NativeValueStep::I64Literal {
                    destination: destination.clone(),
                    value: *value,
                });
                Ok(destination)
            }
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                let source = self.scalar_place(place.root.clone(), &ResolvedType::I64)?;
                let destination = self.new_scalar(&expression.id, "int64_t")?;
                self.push(NativeValueStep::Copy {
                    destination: destination.clone(),
                    source,
                });
                Ok(destination)
            }
            _ => Err(value_error(
                "checked-add operand is outside the parameter/literal corpus",
            )),
        }
    }

    fn validate_owned_tail(
        &self,
        tail: &ResolvedExpr,
    ) -> Result<(usize, ValueId, usize), Diagnostic> {
        let ResolvedExprKind::Place(place) = &tail.kind else {
            return Err(value_error(
                "owned result is not an owned-parameter identity",
            ));
        };
        if !place.projections.is_empty() || tail.ownership != OwnershipMode::Own {
            return Err(value_error(
                "owned result uses a projection or non-own place",
            ));
        }
        let parameter = self
            .function
            .params
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.id == place.root)
            .ok_or_else(|| value_error("owned result does not originate from a parameter"))?;
        if parameter.1.ownership != OwnershipMode::Own
            || parameter.1.ty != self.function.return_type
        {
            return Err(value_error("owned result parameter has the wrong contract"));
        }
        let owner_ordinal = self.function.params[..parameter.0]
            .iter()
            .filter(|candidate| {
                candidate.ownership == OwnershipMode::Own
                    && matches!(candidate.ty, ResolvedType::Nominal { .. })
            })
            .count();
        Ok((parameter.0, parameter.1.id.clone(), owner_ordinal))
    }

    fn validate_owned_body_transfers(&self, tail: &ResolvedExpr) -> Result<(), Diagnostic> {
        let ResolvedExprKind::Place(place) = &tail.kind else {
            return Err(value_error("owned result tail is not a place"));
        };
        let block = self
            .cleanup
            .block(self.current)
            .ok_or_else(|| value_error("owned-result block is absent from cleanup plan"))?;
        let expected_source = StorageId::Value(place.root.clone());
        let expected_temporary = StorageId::Temporary(self.function.body.id.clone());
        let mut transitions = block.transitions.iter();
        let first = transitions.next();
        let second = transitions.next();
        if transitions.next().is_some()
            || !matches!(
                first,
                Some(CleanupTransition::Transfer { at, source, destination })
                    if *at == self.function.body.id
                        && source.storage == expected_source
                        && source.projections.is_empty()
                        && destination.storage == expected_temporary
                        && destination.projections.is_empty()
            )
            || !matches!(
                second,
                Some(CleanupTransition::Transfer { at, source, destination })
                    if *at == self.function.body.id
                        && source.storage == expected_temporary
                        && source.projections.is_empty()
                        && destination.storage == StorageId::ProvisionalResult
                        && destination.projections.is_empty()
            )
        {
            return Err(value_error(
                "owned body does not have the exact two-stage cleanup transfer",
            ));
        }
        Ok(())
    }

    fn validate_success_exit(&self) -> Result<(), Diagnostic> {
        let block = self
            .cleanup
            .block(self.current)
            .ok_or_else(|| value_error("success block is absent from cleanup plan"))?;
        let CleanupTerminator::Exit(exit_id) = block.block.terminator else {
            return Err(value_error("success block does not terminate at cleanup"));
        };
        let exit = self
            .cleanup
            .exit(exit_id)
            .ok_or_else(|| value_error("success cleanup exit is absent"))?;
        match (&self.function.return_type, &exit.exit.continuation) {
            (
                ResolvedType::I64,
                ExitContinuation::CommitResult {
                    source: CleanupResultSource::Scalar { expression },
                },
            ) if *expression == self.function.body.id => Ok(()),
            (
                ResolvedType::Nominal { .. },
                ExitContinuation::CommitResult {
                    source: CleanupResultSource::Owned { storage },
                },
            ) if storage.storage == StorageId::ProvisionalResult
                && storage.projections.is_empty() =>
            {
                Ok(())
            }
            _ => Err(value_error(
                "success exit publishes a different result source",
            )),
        }
    }

    fn scalar_place(&self, root: ValueId, ty: &ResolvedType) -> Result<String, Diagnostic> {
        self.values
            .get(&root)
            .filter(|value| &value.ty == ty)
            .map(|value| value.name.clone())
            .ok_or_else(|| value_error("scalar place is not a matching parameter/result binding"))
    }

    fn new_scalar(
        &mut self,
        expression: &ExpressionId,
        c_type: &'static str,
    ) -> Result<String, Diagnostic> {
        if !self.expression_ids.insert(expression.clone()) {
            return Err(value_error(
                "expression identity is evaluated more than once",
            ));
        }
        let binding = format!("spx_bind_scalar_{}", self.next_scalar);
        self.next_scalar += 1;
        self.declarations.push(NativeValueDeclaration::Scalar {
            expression: expression.clone(),
            c_type,
            binding: binding.clone(),
        });
        Ok(binding)
    }

    fn status_binding(&mut self, source: &StatusSourceId) -> Result<String, Diagnostic> {
        if !self.consumed_statuses.insert(source.clone()) {
            return Err(value_error("status source is evaluated more than once"));
        }
        let binding = format!("spx_bind_status_{}", self.consumed_statuses.len() - 1);
        self.cleanup_bindings
            .status_tokens
            .insert(source.clone(), binding.clone());
        self.declarations.push(NativeValueDeclaration::Status {
            source: source.clone(),
            binding: binding.clone(),
        });
        Ok(binding)
    }

    fn push(&mut self, step: NativeValueStep) {
        self.block_steps.entry(self.current).or_default().push(step);
    }

    fn follow_always_goto(&self, block: BlockId) -> Result<BlockId, Diagnostic> {
        let block = self
            .cleanup
            .block(block)
            .ok_or_else(|| value_error("contract predecessor block is absent"))?;
        let CleanupTerminator::Goto(edge_id) = block.block.terminator else {
            return Err(value_error(
                "contract predecessor is not an unconditional goto",
            ));
        };
        let edge = self
            .cleanup
            .edge(edge_id)
            .filter(|edge| edge.from == block.block.id)
            .ok_or_else(|| value_error("contract entry edge has the wrong owner"))?;
        if edge.condition != EdgeCondition::Always {
            return Err(value_error("contract entry edge is conditional"));
        }
        Ok(edge.to)
    }

    fn boolean_branch(
        &self,
        block: BlockId,
        expression: &ExpressionId,
        source: &StatusSourceId,
    ) -> Result<BlockId, Diagnostic> {
        let block = self
            .cleanup
            .block(block)
            .ok_or_else(|| value_error("contract observation block is absent"))?;
        let CleanupTerminator::Branch(edges) = &block.block.terminator else {
            return Err(value_error("contract observation does not branch"));
        };
        if edges.len() != 2 {
            return Err(value_error("contract observation does not have two edges"));
        }
        let mut success = None;
        let mut failure = None;
        for edge_id in edges {
            let edge = self
                .cleanup
                .edge(*edge_id)
                .filter(|edge| edge.from == block.block.id)
                .ok_or_else(|| value_error("contract edge has the wrong owner"))?;
            match &edge.condition {
                EdgeCondition::BooleanResult(candidate, true) if candidate == expression => {
                    success = Some(edge.to)
                }
                EdgeCondition::BooleanResult(candidate, false) if candidate == expression => {
                    failure = Some(edge.to)
                }
                _ => return Err(value_error("contract branch condition is inconsistent")),
            }
        }
        let success = success.ok_or_else(|| value_error("contract has no success edge"))?;
        let failure = failure.ok_or_else(|| value_error("contract has no failure edge"))?;
        let failure_block = self
            .cleanup
            .block(failure)
            .ok_or_else(|| value_error("contract failure block is absent"))?;
        if !matches!(
            failure_block.transitions,
            [CleanupTransition::SelectFailure { source: candidate }] if candidate == source
        ) {
            return Err(value_error(
                "contract failure block does not select failure once",
            ));
        }
        Ok(success)
    }

    fn status_success_branch(
        &self,
        block: BlockId,
        source: &StatusSourceId,
    ) -> Result<BlockId, Diagnostic> {
        let block = self
            .cleanup
            .block(block)
            .ok_or_else(|| value_error("checked-operation block is absent"))?;
        let CleanupTerminator::Branch(edges) = &block.block.terminator else {
            return Err(value_error("checked operation does not branch on status"));
        };
        if edges.len() != 2 {
            return Err(value_error(
                "checked operation does not have two status edges",
            ));
        }
        let mut success = None;
        let mut failure = None;
        for edge_id in edges {
            let edge = self
                .cleanup
                .edge(*edge_id)
                .filter(|edge| edge.from == block.block.id)
                .ok_or_else(|| value_error("checked-operation edge has the wrong owner"))?;
            match &edge.condition {
                EdgeCondition::StatusZero(candidate) if candidate == source => {
                    success = Some(edge.to)
                }
                EdgeCondition::StatusNonzero(candidate) if candidate == source => {
                    failure = Some(edge.to)
                }
                _ => return Err(value_error("checked-operation status edge is inconsistent")),
            }
        }
        let failure =
            failure.ok_or_else(|| value_error("checked operation has no failure edge"))?;
        let failure_block = self
            .cleanup
            .block(failure)
            .ok_or_else(|| value_error("checked-operation failure block is absent"))?;
        if !matches!(
            failure_block.transitions,
            [CleanupTransition::SelectFailure { source: candidate }] if candidate == source
        ) {
            return Err(value_error(
                "checked-operation failure block selects a different source",
            ));
        }
        success.ok_or_else(|| value_error("checked operation has no success edge"))
    }

    fn follow_success_continuation(&self, block: BlockId) -> Result<BlockId, Diagnostic> {
        let block = self
            .cleanup
            .block(block)
            .ok_or_else(|| value_error("contract success block is absent"))?;
        let CleanupTerminator::Exit(exit_id) = block.block.terminator else {
            return Err(value_error(
                "contract success block does not exit its region",
            ));
        };
        let exit = self
            .cleanup
            .exit(exit_id)
            .ok_or_else(|| value_error("contract success exit is absent"))?;
        if !exit.finalizers.is_empty() {
            return Err(value_error(
                "contract success continuation finalizes resources",
            ));
        }
        let ExitContinuation::Continue(edge_id) = exit.exit.continuation else {
            return Err(value_error("contract success exit does not continue"));
        };
        let edge = self
            .cleanup
            .edge(edge_id)
            .filter(|edge| edge.from == block.block.id && edge.condition == EdgeCondition::Always)
            .ok_or_else(|| value_error("contract continuation edge is inconsistent"))?;
        Ok(edge.to)
    }
}

fn validate_signature(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    abi: &NativeResourceAbi,
) -> Result<(), Diagnostic> {
    if program
        .interfaces
        .iter()
        .any(|interface| !interface.imports.is_empty())
    {
        return Err(value_error(
            "imports are outside the staged single-frame value corpus",
        ));
    }
    for parameter in &function.params {
        match &parameter.ty {
            ResolvedType::Unit => {
                return Err(value_error("unit is not an ordinary native parameter"));
            }
            ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::F32
            | ResolvedType::F64 => {
                return Err(value_error(
                    "non-i64 scalar parameter is outside the staged single-frame value corpus",
                ));
            }
            ResolvedType::String | ResolvedType::Str => {
                return Err(value_error(
                    "text parameter is outside the staged single-frame value corpus",
                ));
            }
            ResolvedType::I64 | ResolvedType::Bool => {
                if parameter.ownership != OwnershipMode::Value {
                    return Err(value_error("scalar parameter is not passed by value"));
                }
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if parameter.ownership != OwnershipMode::Own || !arguments.is_empty() {
                    return Err(value_error(
                        "resource parameter is not a direct owned nominal value",
                    ));
                }
                require_trivial_resource(program, declaration)?;
                let _ = abi.c_type(program, &parameter.ty)?;
            }
            ResolvedType::TypeParameter { .. } => {
                return Err(value_error(
                    "generic parameter is outside the staged corpus",
                ));
            }
        }
    }
    match &function.return_type {
        ResolvedType::I64 => Ok(()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => {
            require_trivial_resource(program, declaration)?;
            let _ = abi.c_type(program, &function.return_type)?;
            Ok(())
        }
        ResolvedType::Unit
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::Str
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::Nominal { .. } => {
            Err(value_error("result type is outside the staged corpus"))
        }
    }
}

fn require_trivial_resource(
    program: &ResolvedProgram,
    declaration: &crate::hir::DeclarationId,
) -> Result<(), Diagnostic> {
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| value_error("resource type declaration is absent"))?;
    match &item.kind {
        ResolvedTypeDeclarationKind::Resource { drop }
            if matches!(drop.kind, ResolvedResourceDropKind::Trivial) =>
        {
            Ok(())
        }
        ResolvedTypeDeclarationKind::Resource { .. } => {
            Err(value_error("imported resource lifecycle is not staged"))
        }
        ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. } => {
            Err(value_error("record type is outside the staged corpus"))
        }
        ResolvedTypeDeclarationKind::Variant { .. } => {
            Err(value_error("variant type is outside the staged corpus"))
        }
    }
}

fn c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped
}

fn value_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B104", format!("native value plan: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{hir, parse};

    use super::super::native_cleanup;
    use super::super::native_resource;
    use super::*;

    const SOURCE: &str = r#"module test.native_values;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{ number + 1 }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token ensures false { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("native-value-plan.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn planned(program: &ResolvedProgram, id: &str) -> NativeValuePlan {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        let cleanup = native_cleanup::classify(program, function).unwrap();
        let abi = native_resource::build_resource_abi(program).unwrap();
        plan(program, function, &cleanup, &abi, &HashMap::new()).unwrap()
    }

    #[test]
    fn checked_plan_places_real_value_steps_in_exact_cfg_blocks() {
        let program = program();
        let planned = planned(&program, "token.checked");
        let source = planned
            .block_steps
            .keys()
            .map(|block| emit_block_prologue(&planned, *block))
            .collect::<String>();
        assert!(source.contains("spx_rt_contract("));
        assert!(source.contains("spx_rt_add("));
        assert_eq!(source.matches("spx_rt_add(").count(), 1);
        assert_eq!(planned.cleanup_bindings.status_tokens.len(), 2);
        assert_eq!(planned.cleanup_bindings.boolean_values.len(), 1);
        assert!(planned.required_event_capacity > 0);
    }

    #[test]
    fn identity_uses_only_cleanup_owned_storage_and_two_transfers() {
        let program = program();
        let planned = planned(&program, "token.identity");
        assert_eq!(planned.cleanup_bindings.storage_values.len(), 3);
        assert!(planned.cleanup_bindings.scalar_results.is_empty());
        assert!(planned
            .declarations
            .iter()
            .all(|declaration| !matches!(declaration, NativeValueDeclaration::Scalar { .. })));
        let declarations = emit_declarations(&planned);
        assert!(declarations.contains("= spx_bind_param_0;"));
        assert_eq!(declarations.matches("= {0};").count(), 2);
    }

    #[test]
    fn literal_result_and_literal_false_ensure_are_bound_deterministically() {
        let program = program();
        let discard = planned(&program, "token.discard");
        assert_eq!(discard.cleanup_bindings.scalar_results.len(), 1);
        assert!(discard
            .block_steps
            .values()
            .flatten()
            .any(|step| matches!(step, NativeValueStep::I64Literal { value: 0, .. })));

        let failed = planned(&program, "token.ensures-false");
        let source = failed
            .block_steps
            .keys()
            .map(|block| emit_block_prologue(&failed, *block))
            .collect::<String>();
        assert!(source.contains("SPX_STATUS_CONTRACT_ENSURES_FALSE"));
        assert!(source.contains("= false;"));
    }

    #[test]
    fn rejects_let_and_lazy_shapes_without_widening_the_gate() {
        for source in [
            r#"module bad.let;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("bad.f") fn f(value: own Token) -> i64 { let number = 1; number }
@id("app.main") fn main() -> i64 { 0 }
"#,
            r#"module bad.lazy;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("bad.f") fn f(value: own Token, allowed: bool) -> i64 requires allowed && true { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#,
        ] {
            let parsed = parse(source, Path::new("native-value-reject.spx")).unwrap();
            let program = hir::resolve(&parsed).unwrap();
            let function = program
                .functions
                .iter()
                .find(|function| function.id.as_str() == "bad.f")
                .unwrap();
            let abi = native_resource::build_resource_abi(&program).unwrap();
            match native_cleanup::classify(&program, function) {
                Ok(cleanup) => {
                    let diagnostic =
                        plan(&program, function, &cleanup, &abi, &HashMap::new()).unwrap_err();
                    assert_eq!(diagnostic.code, "SPX-B104");
                }
                Err(diagnostic) => assert_eq!(diagnostic.code, "SPX-B104"),
            }
        }
    }
}
