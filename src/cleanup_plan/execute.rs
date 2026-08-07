//! Deterministic reference execution for an attached cleanup plan.
//!
//! This is deliberately a cleanup/control-flow oracle, not a HIR value
//! interpreter. A [`CleanupScenario`] supplies the boolean values, operation
//! outcomes, and final result that ordinary expression evaluation would have
//! produced. Calls to SEMAPRAX functions are represented by supplied outcomes;
//! this first acyclic slice does not recursively execute callees.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::conformance::{
    ConformanceTrace, ImportSite, InvocationPath, NormalizedStatus, OperationOutcome, TraceEvent,
    TraceEventKind, TraceOutcome, TraceResult,
};
use crate::hir::{
    self, DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram, ResolvedResourceDropKind,
    ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::runtime_status::{ScopedStatusToken, StatusArena, StatusArenaError, StatusContextId};

use super::{
    BlockId, CleanupBlock, CleanupEdge, CleanupPlace, CleanupResultSource, CleanupTerminator,
    CleanupTransition, EdgeCondition, EdgeId, ExitContinuation, ExitTarget, StatusLane,
    StatusProducer, StatusSourceId, StorageId,
};

/// All target-dependent expression observations needed by the cleanup oracle.
///
/// Maps are keyed by semantic identities and are consumed only when their
/// decision point is reached. Supplying an unused decision is an error, which
/// keeps scenarios precise when a lazy or conditional path changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupScenario {
    pub scenario_id: String,
    pub booleans: BTreeMap<ExpressionId, bool>,
    pub operations: BTreeMap<StatusSourceId, OperationOutcome>,
    /// A value for `CommitResult`; failure scenarios use `None`. `ReturnUnit`
    /// is reserved for a future source-level unit return type and is rejected
    /// for every function the current source language can declare.
    pub result: Option<TraceResult>,
    /// Adapter bindings available to this invocation.
    ///
    /// Unlike boolean and operation outcomes, bindings are configuration:
    /// known bindings may remain unused on the selected execution path.
    pub available_finalizer_imports: BTreeSet<DeclarationId>,
    pub context_nonce: u64,
    pub status_capacity: u32,
}

impl CleanupScenario {
    pub fn new(scenario_id: impl Into<String>, result: Option<TraceResult>) -> Self {
        Self {
            scenario_id: scenario_id.into(),
            booleans: BTreeMap::new(),
            operations: BTreeMap::new(),
            result,
            available_finalizer_imports: BTreeSet::new(),
            context_nonce: 0,
            // One frame has write-once failure selection, so one record is
            // sufficient unless a hostile plan is being exercised.
            status_capacity: 1,
        }
    }
}

/// Harness failures are distinct from language-level status failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupExecutionError {
    InvalidProgram(String),
    FunctionNotFound(DeclarationId),
    UnsupportedResultType(String),
    MissingBooleanDecision(ExpressionId),
    MissingOperationOutcome(StatusSourceId),
    UnusedBooleanDecisions(Vec<ExpressionId>),
    UnusedOperationOutcomes(Vec<StatusSourceId>),
    CycleDetected(BlockId),
    UnknownFinalizerBinding(DeclarationId),
    MissingFinalizerBinding(DeclarationId),
    UnsupportedCallableImport(DeclarationId),
    StatusArena(StatusArenaError),
    HarnessInvariant(String),
}

impl fmt::Display for CleanupExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProgram(detail) => write!(formatter, "invalid resolved program: {detail}"),
            Self::FunctionNotFound(function) => {
                write!(formatter, "cleanup function `{function}` does not exist")
            }
            Self::UnsupportedResultType(result) => write!(
                formatter,
                "cleanup conformance executor does not support result type `{result}`"
            ),
            Self::MissingBooleanDecision(expression) => write!(
                formatter,
                "cleanup scenario has no boolean decision for `{expression}`"
            ),
            Self::MissingOperationOutcome(source) => write!(
                formatter,
                "cleanup scenario has no operation outcome for `{}`",
                source.expression
            ),
            Self::UnusedBooleanDecisions(expressions) => write!(
                formatter,
                "cleanup scenario has unused boolean decisions: {expressions:?}"
            ),
            Self::UnusedOperationOutcomes(sources) => write!(
                formatter,
                "cleanup scenario has unused operation outcomes: {sources:?}"
            ),
            Self::CycleDetected(block) => write!(
                formatter,
                "cleanup plan revisited block {}; cyclic execution is not supported",
                block.0
            ),
            Self::UnknownFinalizerBinding(import) => {
                write!(
                    formatter,
                    "finalizer binding `{import}` is not a resolved import"
                )
            }
            Self::MissingFinalizerBinding(import) => {
                write!(
                    formatter,
                    "finalizer import `{import}` has no scenario binding"
                )
            }
            Self::UnsupportedCallableImport(import) => write!(
                formatter,
                "callable import `{import}` is outside the attached-plan oracle slice"
            ),
            Self::StatusArena(error) => write!(formatter, "status arena error: {error}"),
            Self::HarnessInvariant(detail) => {
                write!(formatter, "cleanup execution invariant failed: {detail}")
            }
        }
    }
}

impl Error for CleanupExecutionError {}

impl From<StatusArenaError> for CleanupExecutionError {
    fn from(error: StatusArenaError) -> Self {
        Self::StatusArena(error)
    }
}

/// Execute one function's validated, attached cleanup plan.
///
/// The executor seeds owned parameters from `entry_state`, observes supplied
/// expression decisions, applies transitions in plan order, clears finalizer
/// guards before invocation, and emits only target-neutral conformance events.
pub fn execute_for_conformance(
    program: &ResolvedProgram,
    function: &DeclarationId,
    scenario: CleanupScenario,
) -> Result<ConformanceTrace, CleanupExecutionError> {
    hir::validate_core(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    crate::cleanup::validate_program(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    super::replay::validate_program(program)
        .map_err(|diagnostic| CleanupExecutionError::InvalidProgram(diagnostic.to_string()))?;
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.id == *function)
        .ok_or_else(|| CleanupExecutionError::FunctionNotFound(function.clone()))?;
    validate_public_result_type(program, function)?;
    Executor::new(program, function, scenario)?.run()
}

fn validate_public_result_type(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), CleanupExecutionError> {
    let ResolvedType::Nominal { declaration, .. } = &function.return_type else {
        return if matches!(function.return_type, ResolvedType::I64 | ResolvedType::Bool) {
            Ok(())
        } else {
            Err(CleanupExecutionError::UnsupportedResultType(
                function.return_type.identity_key(),
            ))
        };
    };
    match program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .map(|item| &item.kind)
    {
        Some(ResolvedTypeDeclarationKind::Resource { .. }) => Ok(()),
        Some(ResolvedTypeDeclarationKind::Record { .. }) | None => Err(
            CleanupExecutionError::UnsupportedResultType(function.return_type.identity_key()),
        ),
    }
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
    lifecycle: DeclarationId,
}

#[derive(Clone)]
struct SelectedFailure {
    source: StatusSourceId,
    token: ScopedStatusToken,
}

/// Semantic caller out-slot state. No payload, address, alignment, or target
/// representation is stored in the reference executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultSlotState {
    Uninitialized,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalKind {
    CommitResult,
    ReturnFailure,
    /// Reserved for a future source-level unit return type.
    ReturnUnit,
}

struct Executor<'a> {
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    scenario: CleanupScenario,
    leaves: BTreeMap<LivenessFlagId, Leaf>,
    lifecycle_bindings: BTreeMap<DeclarationId, Option<DeclarationId>>,
    live: BTreeSet<LivenessFlagId>,
    used_booleans: BTreeSet<ExpressionId>,
    used_operations: BTreeSet<StatusSourceId>,
    visited: BTreeSet<BlockId>,
    selected: Option<SelectedFailure>,
    result_slot: ResultSlotState,
    status_arena: StatusArena,
    events: Vec<TraceEvent>,
}

impl<'a> Executor<'a> {
    fn new(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        scenario: CleanupScenario,
    ) -> Result<Self, CleanupExecutionError> {
        let mut leaves = BTreeMap::new();
        for slot in &function.cleanup_plan.slots {
            collect_leaves(
                &slot.storage,
                &mut Vec::new(),
                &slot.field_liveness_shape,
                &mut leaves,
            )?;
        }
        // Adapter configuration is validated before entry guards are seeded
        // and before execution can emit any event. A binding is required for
        // every imported lifecycle present anywhere in this function's plan,
        // even when the selected path would leave its guard false.
        let lifecycle_bindings = preflight_finalizer_bindings(program, function, &scenario)?;
        let status_arena = StatusArena::new(
            StatusContextId::new(scenario.context_nonce),
            scenario.status_capacity,
        )?;
        let mut executor = Self {
            program,
            function,
            scenario,
            leaves,
            lifecycle_bindings,
            live: BTreeSet::new(),
            used_booleans: BTreeSet::new(),
            used_operations: BTreeSet::new(),
            visited: BTreeSet::new(),
            selected: None,
            result_slot: ResultSlotState::Uninitialized,
            status_arena,
            events: Vec::new(),
        };
        for place in executor
            .function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .clone()
        {
            executor.initialize_flags(&place, "owned entry parameter")?;
        }
        Ok(executor)
    }

    fn run(mut self) -> Result<ConformanceTrace, CleanupExecutionError> {
        let mut current = self.function.cleanup_plan.entry;
        let (outcome, terminal) = loop {
            if !self.visited.insert(current) {
                return Err(CleanupExecutionError::CycleDetected(current));
            }
            let block = self.block(current)?.clone();
            for transition in block.transitions {
                self.execute_transition(transition)?;
            }
            match block.terminator {
                CleanupTerminator::Goto(edge) => current = self.follow_goto(current, edge)?,
                CleanupTerminator::Branch(edges) => {
                    current = self.follow_branch(current, &edges)?;
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = self.exit(exit, current)?.clone();
                    self.execute_finalizers(&exit)?;
                    match exit.continuation {
                        ExitContinuation::Continue(edge) => {
                            current = self.follow_goto(current, edge)?;
                        }
                        ExitContinuation::CommitResult { source } => {
                            break (self.commit_result(source)?, TerminalKind::CommitResult);
                        }
                        ExitContinuation::ReturnFailure { source } => {
                            break (self.return_failure(source)?, TerminalKind::ReturnFailure);
                        }
                        ExitContinuation::ReturnUnit => {
                            break (self.return_unit()?, TerminalKind::ReturnUnit);
                        }
                    }
                }
            }
        };
        self.finish(outcome, terminal)
    }

    fn block(&self, id: BlockId) -> Result<&CleanupBlock, CleanupExecutionError> {
        self.function
            .cleanup_plan
            .blocks
            .iter()
            .find(|block| block.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup block {}", id.0)))
    }

    fn edge(&self, id: EdgeId, from: BlockId) -> Result<&CleanupEdge, CleanupExecutionError> {
        let edge = self
            .function
            .cleanup_plan
            .edges
            .iter()
            .find(|edge| edge.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup edge {}", id.0)))?;
        if edge.from != from {
            return Err(invariant(format!(
                "edge {} starts at block {}, not block {}",
                id.0, edge.from.0, from.0
            )));
        }
        Ok(edge)
    }

    fn exit(
        &self,
        id: super::ExitTargetId,
        from: BlockId,
    ) -> Result<&ExitTarget, CleanupExecutionError> {
        let exit = self
            .function
            .cleanup_plan
            .exits
            .iter()
            .find(|exit| exit.id == id)
            .ok_or_else(|| invariant(format!("missing cleanup exit {}", id.0)))?;
        if exit.from != from {
            return Err(invariant(format!(
                "exit {} starts at block {}, not block {}",
                id.0, exit.from.0, from.0
            )));
        }
        Ok(exit)
    }

    fn execute_transition(
        &mut self,
        transition: CleanupTransition,
    ) -> Result<(), CleanupExecutionError> {
        match transition {
            CleanupTransition::Initialize { at, destination } => {
                self.initialize_flags(&destination, "initialize transition")?;
                self.emit(TraceEventKind::Initialize { at, destination });
            }
            CleanupTransition::Transfer {
                at,
                source,
                destination,
            } => {
                self.transfer_flags(&source, &destination)?;
                self.emit(TraceEventKind::Transfer {
                    at,
                    source,
                    destination,
                });
            }
            CleanupTransition::CallCommit { call, arguments } => {
                let callee = self.callee_for_call(&call)?;
                let mut consumed = BTreeSet::new();
                for argument in &arguments {
                    for flag in self.flags_under(&argument.source)? {
                        if !self.live.contains(&flag) {
                            return Err(invariant(format!(
                                "call `{call}` consumes dead argument flag {}",
                                flag.0
                            )));
                        }
                        if !consumed.insert(flag) {
                            return Err(invariant(format!(
                                "call `{call}` consumes flag {} more than once",
                                flag.0
                            )));
                        }
                    }
                }
                // Clear the complete group only after every argument epoch has
                // been checked, preserving atomic call commit.
                self.live.retain(|flag| !consumed.contains(flag));
                self.emit(TraceEventKind::CallCommit {
                    call,
                    callee,
                    arguments,
                });
            }
            CleanupTransition::SelectFailure { source } => self.select_failure(source)?,
        }
        Ok(())
    }

    fn callee_for_call(&self, call: &ExpressionId) -> Result<DeclarationId, CleanupExecutionError> {
        let source = self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| {
                source.id.expression == *call && source.id.lane == StatusLane::OperationFailure
            })
            .ok_or_else(|| invariant(format!("call `{call}` has no propagated status source")))?;
        let StatusProducer::PropagatedCall { callee } = &source.producer else {
            return Err(invariant(format!(
                "call `{call}` status source is not a propagated call"
            )));
        };
        if self.program.functions.iter().any(|item| item.id == *callee) {
            return Ok(callee.clone());
        }
        if self
            .program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.id == *callee)
        {
            return Err(CleanupExecutionError::UnsupportedCallableImport(
                callee.clone(),
            ));
        }
        Err(invariant(format!("call target `{callee}` does not exist")))
    }

    fn select_failure(&mut self, source: StatusSourceId) -> Result<(), CleanupExecutionError> {
        if self.selected.is_some() {
            return Err(invariant("failure selection is write-once"));
        }
        let producer = self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .find(|candidate| candidate.id == source)
            .map(|candidate| candidate.producer.clone())
            .ok_or_else(|| invariant(format!("unknown failure source `{}`", source.expression)))?;
        let status = match producer {
            StatusProducer::ContractFalse { phase, .. } => NormalizedStatus::contract(phase),
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => {
                let status = self.failure_outcome(&source)?;
                if !normalized_cases
                    .iter()
                    .any(|case| status == NormalizedStatus::arithmetic(*case))
                {
                    return Err(invariant(format!(
                        "checked operation `{}` supplied a status outside its normalized cases",
                        source.expression
                    )));
                }
                status
            }
            StatusProducer::PropagatedCall { .. } => self.failure_outcome(&source)?,
        };
        let token = self.status_arena.record(status.clone())?;
        self.selected = Some(SelectedFailure {
            source: source.clone(),
            token,
        });
        self.emit(TraceEventKind::SelectFailure { source, status });
        Ok(())
    }

    fn failure_outcome(
        &mut self,
        source: &StatusSourceId,
    ) -> Result<NormalizedStatus, CleanupExecutionError> {
        self.used_operations.insert(source.clone());
        match self
            .scenario
            .operations
            .get(source)
            .ok_or_else(|| CleanupExecutionError::MissingOperationOutcome(source.clone()))?
        {
            OperationOutcome::Failure(status) => Ok(status.clone()),
            OperationOutcome::Success => Err(invariant(format!(
                "failure source `{}` selected a successful operation",
                source.expression
            ))),
        }
    }

    fn follow_goto(&mut self, from: BlockId, id: EdgeId) -> Result<BlockId, CleanupExecutionError> {
        let edge = self.edge(id, from)?.clone();
        if !self.condition_matches(&edge.condition)? {
            return Err(invariant(format!("goto edge {} condition is false", id.0)));
        }
        Ok(edge.to)
    }

    fn follow_branch(
        &mut self,
        from: BlockId,
        ids: &[EdgeId],
    ) -> Result<BlockId, CleanupExecutionError> {
        let mut selected = None;
        for id in ids {
            let edge = self.edge(*id, from)?.clone();
            if self.condition_matches(&edge.condition)? && selected.replace(edge.to).is_some() {
                return Err(invariant(format!(
                    "branch from block {} selects multiple edges",
                    from.0
                )));
            }
        }
        selected.ok_or_else(|| invariant(format!("branch from block {} selects no edge", from.0)))
    }

    fn condition_matches(
        &mut self,
        condition: &EdgeCondition,
    ) -> Result<bool, CleanupExecutionError> {
        match condition {
            EdgeCondition::Always => Ok(true),
            EdgeCondition::BooleanResult(expression, expected) => {
                self.used_booleans.insert(expression.clone());
                let actual = self.scenario.booleans.get(expression).ok_or_else(|| {
                    CleanupExecutionError::MissingBooleanDecision(expression.clone())
                })?;
                Ok(actual == expected)
            }
            EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
                self.used_operations.insert(source.clone());
                let outcome = self.scenario.operations.get(source).ok_or_else(|| {
                    CleanupExecutionError::MissingOperationOutcome(source.clone())
                })?;
                let success = matches!(outcome, OperationOutcome::Success);
                Ok(match condition {
                    EdgeCondition::StatusZero(_) => success,
                    EdgeCondition::StatusNonzero(_) => !success,
                    EdgeCondition::Always | EdgeCondition::BooleanResult(_, _) => unreachable!(),
                })
            }
        }
    }

    fn execute_finalizers(&mut self, exit: &ExitTarget) -> Result<(), CleanupExecutionError> {
        for action in &exit.finalize_in_order {
            let leaf = self
                .leaves
                .get(&action.guard_flag)
                .cloned()
                .ok_or_else(|| {
                    invariant(format!(
                        "finalizer references unknown guard {}",
                        action.guard_flag.0
                    ))
                })?;
            if leaf.place != action.source || leaf.lifecycle != action.lifecycle_id {
                return Err(invariant(format!(
                    "finalizer guard {} disagrees with its cleanup leaf",
                    action.guard_flag.0
                )));
            }
            let binding_import = self
                .lifecycle_bindings
                .get(&action.lifecycle_id)
                .cloned()
                .ok_or_else(|| {
                    invariant(format!(
                        "lifecycle `{}` was not preflighted",
                        action.lifecycle_id
                    ))
                })?;
            if !self.live.remove(&action.guard_flag) {
                continue;
            }
            self.emit(TraceEventKind::FinalizeBegin {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import: binding_import.clone(),
            });
            if let Some(import_id) = binding_import.clone() {
                self.emit(TraceEventKind::ImportBegin {
                    site: ImportSite::Finalizer {
                        source: action.source.clone(),
                        lifecycle_id: action.lifecycle_id.clone(),
                    },
                    import_id: import_id.clone(),
                });
                self.emit(TraceEventKind::FinalizerImportEnd {
                    source: action.source.clone(),
                    lifecycle_id: action.lifecycle_id.clone(),
                    import_id,
                });
            }
            self.emit(TraceEventKind::FinalizeEnd {
                source: action.source.clone(),
                lifecycle_id: action.lifecycle_id.clone(),
                guard_flag: action.guard_flag,
                binding_import,
            });
        }
        Ok(())
    }

    fn commit_result(
        &mut self,
        source: CleanupResultSource,
    ) -> Result<TraceOutcome, CleanupExecutionError> {
        if self.result_slot != ResultSlotState::Uninitialized {
            return Err(invariant("caller result slot is already published"));
        }
        if self.selected.is_some() {
            return Err(invariant("result publication follows failure selection"));
        }
        let result = self
            .scenario
            .result
            .as_ref()
            .ok_or_else(|| invariant("result commit has no supplied result"))?;
        self.validate_result(&source, result)?;
        let published_flags = match &source {
            CleanupResultSource::Scalar { .. } => {
                if !self.live.is_empty() {
                    return Err(invariant(
                        "result publication occurs before non-result cleanup",
                    ));
                }
                BTreeSet::new()
            }
            CleanupResultSource::Owned { storage } => {
                let flags = self.flags_under(storage)?;
                if flags.iter().any(|flag| !self.live.contains(flag)) {
                    return Err(invariant("owned result is incomplete at publication"));
                }
                if self.live != flags {
                    return Err(invariant(
                        "result publication occurs before non-result cleanup",
                    ));
                }
                flags
            }
        };

        let result = self
            .scenario
            .result
            .take()
            .expect("validated result observation remains present");
        self.live.retain(|flag| !published_flags.contains(flag));
        self.result_slot = ResultSlotState::Published;
        self.emit(TraceEventKind::ResultCommit { source });
        Ok(TraceOutcome::Success { result })
    }

    fn validate_result(
        &self,
        source: &CleanupResultSource,
        result: &TraceResult,
    ) -> Result<(), CleanupExecutionError> {
        let matches_type = match (&self.function.return_type, result) {
            (ResolvedType::I64, TraceResult::I64(_))
            | (ResolvedType::Bool, TraceResult::Bool(_)) => true,
            (ResolvedType::Nominal { declaration, .. }, TraceResult::Owned { type_id }) => {
                declaration == type_id
            }
            (ResolvedType::TypeParameter { .. }, _) | (_, _) => false,
        };
        let source_matches = match (source, &self.function.return_type) {
            (CleanupResultSource::Scalar { .. }, ResolvedType::I64 | ResolvedType::Bool) => true,
            (CleanupResultSource::Owned { storage }, ResolvedType::Nominal { .. }) => {
                storage.storage == StorageId::ProvisionalResult && storage.projections.is_empty()
            }
            (CleanupResultSource::Scalar { .. }, ResolvedType::Nominal { .. })
            | (CleanupResultSource::Owned { .. }, ResolvedType::I64 | ResolvedType::Bool)
            | (_, ResolvedType::TypeParameter { .. }) => false,
        };
        if !matches_type || !source_matches {
            return Err(invariant(
                "supplied trace result disagrees with function result",
            ));
        }
        Ok(())
    }

    fn return_failure(
        &mut self,
        source: StatusSourceId,
    ) -> Result<TraceOutcome, CleanupExecutionError> {
        if self.result_slot != ResultSlotState::Uninitialized {
            return Err(invariant("failure return follows result publication"));
        }
        let selected = self
            .selected
            .as_ref()
            .ok_or_else(|| invariant("failure return has no selected status"))?;
        if selected.source != source {
            return Err(invariant("failure return changes the selected source"));
        }
        let status = self.status_arena.resolve(selected.token)?.clone();
        Ok(TraceOutcome::Failure {
            selected_source: source,
            status,
        })
    }

    fn return_unit(&mut self) -> Result<TraceOutcome, CleanupExecutionError> {
        Err(invariant(
            "ReturnUnit is invalid for source functions without a unit return type",
        ))
    }

    fn initialize_flags(
        &mut self,
        place: &CleanupPlace,
        operation: &str,
    ) -> Result<(), CleanupExecutionError> {
        let flags = self.flags_under(place)?;
        if flags.iter().any(|flag| self.live.contains(flag)) {
            return Err(invariant(format!(
                "{operation} targets a live cleanup place"
            )));
        }
        self.live.extend(flags);
        Ok(())
    }

    fn transfer_flags(
        &mut self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
    ) -> Result<(), CleanupExecutionError> {
        let source_flags = self.flags_under(source)?;
        let destination_flags = self.flags_under(destination)?;
        if source_flags.len() != destination_flags.len() {
            return Err(invariant("cleanup transfer has unequal leaf counts"));
        }
        if source_flags.iter().any(|flag| !self.live.contains(flag)) {
            return Err(invariant("cleanup transfer reads a dead source"));
        }
        if destination_flags
            .iter()
            .any(|flag| self.live.contains(flag))
        {
            return Err(invariant("cleanup transfer initializes a live destination"));
        }
        self.live.retain(|flag| !source_flags.contains(flag));
        self.live.extend(destination_flags);
        Ok(())
    }

    fn flags_under(
        &self,
        place: &CleanupPlace,
    ) -> Result<BTreeSet<LivenessFlagId>, CleanupExecutionError> {
        let flags = self
            .leaves
            .iter()
            .filter_map(|(flag, leaf)| {
                (leaf.place.storage == place.storage
                    && leaf.place.projections.starts_with(&place.projections))
                .then_some(*flag)
            })
            .collect::<BTreeSet<_>>();
        if flags.is_empty() {
            return Err(invariant(format!(
                "cleanup place `{place:?}` has no liveness flags"
            )));
        }
        Ok(flags)
    }

    fn emit(&mut self, event: TraceEventKind) {
        self.events.push(TraceEvent {
            function: self.function.id.clone(),
            invocation: InvocationPath::default(),
            event,
        });
    }

    fn finish(
        self,
        outcome: TraceOutcome,
        terminal: TerminalKind,
    ) -> Result<ConformanceTrace, CleanupExecutionError> {
        let unused_booleans = self
            .scenario
            .booleans
            .keys()
            .filter(|expression| !self.used_booleans.contains(*expression))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_booleans.is_empty() {
            return Err(CleanupExecutionError::UnusedBooleanDecisions(
                unused_booleans,
            ));
        }
        let unused_operations = self
            .scenario
            .operations
            .keys()
            .filter(|source| !self.used_operations.contains(*source))
            .cloned()
            .collect::<Vec<_>>();
        if !unused_operations.is_empty() {
            return Err(CleanupExecutionError::UnusedOperationOutcomes(
                unused_operations,
            ));
        }
        if self.scenario.result.is_some() {
            return Err(invariant("cleanup scenario supplied an unused result"));
        }
        if !self.live.is_empty() {
            return Err(invariant(format!(
                "terminal cleanup state retains live flags {:?}",
                self.live
            )));
        }
        let terminal_state_is_valid = matches!(
            (&outcome, terminal, self.result_slot),
            (
                TraceOutcome::Success { .. },
                TerminalKind::CommitResult,
                ResultSlotState::Published,
            ) | (
                TraceOutcome::Success {
                    result: TraceResult::Unit,
                },
                TerminalKind::ReturnUnit,
                ResultSlotState::Uninitialized,
            ) | (
                TraceOutcome::Failure { .. },
                TerminalKind::ReturnFailure,
                ResultSlotState::Uninitialized,
            )
        );
        if !terminal_state_is_valid {
            return Err(invariant(
                "terminal outcome disagrees with caller result-slot publication",
            ));
        }
        Ok(ConformanceTrace::new(
            self.scenario.scenario_id,
            self.function.id.clone(),
            self.events,
            outcome,
        ))
    }
}

fn preflight_finalizer_bindings(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    scenario: &CleanupScenario,
) -> Result<BTreeMap<DeclarationId, Option<DeclarationId>>, CleanupExecutionError> {
    let known_imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .map(|import| import.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = scenario
        .available_finalizer_imports
        .iter()
        .find(|import| !known_imports.contains(*import))
    {
        return Err(CleanupExecutionError::UnknownFinalizerBinding(
            unknown.clone(),
        ));
    }

    let mut bindings = BTreeMap::new();
    for action in function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
    {
        if bindings.contains_key(&action.lifecycle_id) {
            continue;
        }
        let binding = resolve_lifecycle_binding(program, &action.lifecycle_id)?;
        if let Some(import) = &binding {
            if !scenario.available_finalizer_imports.contains(import) {
                return Err(CleanupExecutionError::MissingFinalizerBinding(
                    import.clone(),
                ));
            }
        }
        bindings.insert(action.lifecycle_id.clone(), binding);
    }
    Ok(bindings)
}

fn resolve_lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Result<Option<DeclarationId>, CleanupExecutionError> {
    let mut binding = None;
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        if drop.id != *lifecycle {
            continue;
        }
        if binding.is_some() {
            return Err(invariant(format!(
                "lifecycle `{lifecycle}` resolves more than once"
            )));
        }
        binding = Some(match &drop.kind {
            ResolvedResourceDropKind::Trivial => None,
            ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
        });
    }
    binding.ok_or_else(|| invariant(format!("unknown lifecycle `{lifecycle}`")))
}

fn collect_leaves(
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), CleanupExecutionError> {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            let leaf = Leaf {
                place: CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.clone(),
                },
                lifecycle: lifecycle.clone(),
            };
            if leaves.insert(*flag, leaf).is_some() {
                return Err(invariant(format!(
                    "cleanup flag {} is declared more than once",
                    flag.0
                )));
            }
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                collect_leaves(storage, projections, &field.shape, leaves)?;
                projections.pop();
            }
        }
    }
    Ok(())
}

fn invariant(detail: impl Into<String>) -> CleanupExecutionError {
    CleanupExecutionError::HarnessInvariant(detail.into())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::conformance::TraceEventKind;
    use crate::{hir, parse};

    use super::*;

    const SOURCE: &str = r#"module test.cleanup_execute;
permit { io.release }

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("pair.type")
record Pair {
    @id("pair.first")
    first: Token,

    @id("pair.second")
    second: Token,
}

@id("file.type")
resource File {
    @id("file.drop")
    drop import "file.finalize";
}

@id("file.host")
interface FileHost permits { io.release } {
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { io.release }
        failure infallible
        consumes file always;
}

@id("scalar.success")
fn scalar_success() -> i64 { 42 }

@id("contract.failure")
fn contract_failure(value: i64) -> i64
    requires value > 0
{
    value
}

@id("token.discard")
fn discard_token(value: own Token) -> i64 { 0 }

@id("pair.identity")
fn identity_pair(value: own Pair) -> Pair { value }

@id("file.discard")
fn discard_file(value: own File) -> i64 uses { io.release } { 0 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("cleanup-execute.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }

    #[test]
    fn scalar_success_commits_the_supplied_result() {
        let program = program();
        let function = function(&program, "scalar.success");
        let trace = execute_for_conformance(
            &program,
            &function.id,
            CleanupScenario::new("scalar-success", Some(TraceResult::I64(42))),
        )
        .unwrap();

        assert_eq!(
            trace.outcome,
            TraceOutcome::Success {
                result: TraceResult::I64(42)
            }
        );
        assert!(matches!(
            trace.events.as_slice(),
            [TraceEvent {
                event: TraceEventKind::ResultCommit {
                    source: CleanupResultSource::Scalar { .. }
                },
                ..
            }]
        ));
    }

    #[test]
    fn false_contract_selects_and_returns_the_exact_status() {
        let program = program();
        let function = function(&program, "contract.failure");
        let contract = function.requires[0].id.clone();
        let source = StatusSourceId {
            expression: contract.clone(),
            lane: StatusLane::ContractFalse,
        };
        let mut scenario = CleanupScenario::new("contract-failure", None);
        scenario.booleans.insert(contract, false);
        scenario.context_nonce = 9;
        let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();

        assert_eq!(
            trace.outcome,
            TraceOutcome::Failure {
                selected_source: source.clone(),
                status: NormalizedStatus::contract(super::super::ContractPhase::Requires),
            }
        );
        assert!(matches!(
            trace.events.as_slice(),
            [TraceEvent {
                event: TraceEventKind::SelectFailure {
                    source: actual,
                    ..
                },
                ..
            }] if actual == &source
        ));
    }

    #[test]
    fn owned_entry_is_finalized_before_scalar_publication() {
        let program = program();
        let function = function(&program, "token.discard");
        let trace = execute_for_conformance(
            &program,
            &function.id,
            CleanupScenario::new("owned-finalizer", Some(TraceResult::I64(0))),
        )
        .unwrap();

        assert!(matches!(
            trace.events.as_slice(),
            [
                TraceEvent {
                    event: TraceEventKind::FinalizeBegin {
                        binding_import: None,
                        ..
                    },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::FinalizeEnd {
                        binding_import: None,
                        ..
                    },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::ResultCommit { .. },
                    ..
                }
            ]
        ));
    }

    #[test]
    fn imported_finalizer_emits_split_success_completion() {
        let program = program();
        let function = function(&program, "file.discard");
        let mut scenario = CleanupScenario::new("imported-finalizer", Some(TraceResult::I64(0)));
        scenario
            .available_finalizer_imports
            .insert(DeclarationId::new("file.finalize"));
        let trace = execute_for_conformance(&program, &function.id, scenario).unwrap();

        assert!(matches!(
            trace.events.as_slice(),
            [
                TraceEvent {
                    event: TraceEventKind::FinalizeBegin {
                        binding_import: Some(import),
                        ..
                    },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::ImportBegin { .. },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::FinalizerImportEnd { .. },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::FinalizeEnd { .. },
                    ..
                },
                TraceEvent {
                    event: TraceEventKind::ResultCommit { .. },
                    ..
                }
            ] if import.as_str() == "file.finalize"
        ));
    }

    #[test]
    fn finalizer_bindings_are_preflighted_but_need_not_be_path_used() {
        let program = program();
        let file = function(&program, "file.discard");
        assert!(matches!(
            execute_for_conformance(
                &program,
                &file.id,
                CleanupScenario::new("missing-binding", Some(TraceResult::I64(0))),
            ),
            Err(CleanupExecutionError::MissingFinalizerBinding(import))
                if import.as_str() == "file.finalize"
        ));

        let scalar = function(&program, "scalar.success");
        let mut unknown = CleanupScenario::new("unknown-binding", Some(TraceResult::I64(42)));
        unknown
            .available_finalizer_imports
            .insert(DeclarationId::new("missing.finalizer"));
        assert!(matches!(
            execute_for_conformance(&program, &scalar.id, unknown),
            Err(CleanupExecutionError::UnknownFinalizerBinding(import))
                if import.as_str() == "missing.finalizer"
        ));

        // Bindings are adapter configuration, not execution outcomes. A known
        // binding configured for a path/function that does not use it is valid.
        let mut configured = CleanupScenario::new("configured-unused", Some(TraceResult::I64(42)));
        configured
            .available_finalizer_imports
            .insert(DeclarationId::new("file.finalize"));
        assert!(execute_for_conformance(&program, &scalar.id, configured).is_ok());
    }

    #[test]
    fn scalar_source_functions_reject_return_unit() {
        let program = program();
        let function = function(&program, "scalar.success");
        let mut executor =
            Executor::new(&program, function, CleanupScenario::new("unit-none", None)).unwrap();
        assert_eq!(
            executor.return_unit(),
            Err(CleanupExecutionError::HarnessInvariant(
                "ReturnUnit is invalid for source functions without a unit return type".to_owned(),
            ))
        );
        assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);
        assert!(executor.events.is_empty());
    }

    #[test]
    fn owned_publication_rejects_projected_and_incomplete_provisional_results() {
        let program = program();
        let function = function(&program, "pair.identity");
        let result = TraceResult::Owned {
            type_id: DeclarationId::new("pair.type"),
        };
        let projected_source = CleanupResultSource::Owned {
            storage: CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: vec![DeclarationId::new("pair.first")],
            },
        };
        let mut projected = Executor::new(
            &program,
            function,
            CleanupScenario::new("projected-result", Some(result.clone())),
        )
        .unwrap();
        assert_eq!(
            projected.commit_result(projected_source),
            Err(CleanupExecutionError::HarnessInvariant(
                "supplied trace result disagrees with function result".to_owned(),
            ))
        );

        let whole_source = CleanupResultSource::Owned {
            storage: CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            },
        };
        let mut incomplete = Executor::new(
            &program,
            function,
            CleanupScenario::new("incomplete-result", Some(result)),
        )
        .unwrap();
        let one_result_flag = incomplete
            .leaves
            .iter()
            .find_map(|(flag, leaf)| {
                (leaf.place.storage == StorageId::ProvisionalResult).then_some(*flag)
            })
            .expect("pair result must have provisional liveness flags");
        incomplete.live.clear();
        incomplete.live.insert(one_result_flag);
        assert_eq!(
            incomplete.commit_result(whole_source),
            Err(CleanupExecutionError::HarnessInvariant(
                "owned result is incomplete at publication".to_owned(),
            ))
        );
        assert_eq!(incomplete.result_slot, ResultSlotState::Uninitialized);
        assert!(incomplete.scenario.result.is_some());
        assert!(incomplete.events.is_empty());
    }

    #[test]
    fn public_executor_rejects_aggregate_result_projection_until_trace_v2() {
        let program = program();
        let function = function(&program, "pair.identity");
        assert!(matches!(
            execute_for_conformance(
                &program,
                &function.id,
                CleanupScenario::new(
                    "aggregate-result",
                    Some(TraceResult::Owned {
                        type_id: DeclarationId::new("pair.type"),
                    }),
                ),
            ),
            Err(CleanupExecutionError::UnsupportedResultType(result))
                if result.contains("pair.type")
        ));
    }

    #[test]
    fn contract_failure_keeps_the_caller_result_slot_poisoned() {
        let program = program();
        let function = function(&program, "contract.failure");
        let source = function
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| source.id.lane == StatusLane::ContractFalse)
            .unwrap()
            .id
            .clone();
        let mut executor = Executor::new(
            &program,
            function,
            CleanupScenario::new("contract-poison", None),
        )
        .unwrap();
        assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);

        executor.select_failure(source.clone()).unwrap();
        let outcome = executor.return_failure(source).unwrap();
        assert!(matches!(outcome, TraceOutcome::Failure { .. }));
        assert_eq!(executor.result_slot, ResultSlotState::Uninitialized);
        assert!(!executor
            .events
            .iter()
            .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
    }

    #[test]
    fn result_publication_rejects_early_cleanup_and_duplicate_commits() {
        let program = program();
        let owned = function(&program, "token.discard");
        let owned_source = owned
            .cleanup_plan
            .exits
            .iter()
            .find_map(|exit| match &exit.continuation {
                ExitContinuation::CommitResult { source } => Some(source.clone()),
                ExitContinuation::Continue(_)
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => None,
            })
            .unwrap();
        let mut early = Executor::new(
            &program,
            owned,
            CleanupScenario::new("early-publication", Some(TraceResult::I64(0))),
        )
        .unwrap();
        assert_eq!(
            early.commit_result(owned_source),
            Err(CleanupExecutionError::HarnessInvariant(
                "result publication occurs before non-result cleanup".to_owned(),
            ))
        );
        assert_eq!(early.result_slot, ResultSlotState::Uninitialized);
        assert!(early.scenario.result.is_some());
        assert!(early.events.is_empty());

        let scalar = function(&program, "scalar.success");
        let scalar_source = scalar
            .cleanup_plan
            .exits
            .iter()
            .find_map(|exit| match &exit.continuation {
                ExitContinuation::CommitResult { source } => Some(source.clone()),
                ExitContinuation::Continue(_)
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => None,
            })
            .unwrap();
        let mut duplicate = Executor::new(
            &program,
            scalar,
            CleanupScenario::new("duplicate-publication", Some(TraceResult::I64(42))),
        )
        .unwrap();
        duplicate.commit_result(scalar_source.clone()).unwrap();
        assert_eq!(duplicate.result_slot, ResultSlotState::Published);
        duplicate.scenario.result = Some(TraceResult::I64(42));
        assert_eq!(
            duplicate.commit_result(scalar_source),
            Err(CleanupExecutionError::HarnessInvariant(
                "caller result slot is already published".to_owned(),
            ))
        );
        assert_eq!(duplicate.result_slot, ResultSlotState::Published);
        assert_eq!(
            duplicate
                .events
                .iter()
                .filter(|event| matches!(event.event, TraceEventKind::ResultCommit { .. }))
                .count(),
            1
        );
    }
}
