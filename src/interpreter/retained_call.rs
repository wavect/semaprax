//! Retained multi-argument call seam over one already-resolved program.
//!
//! [`prepare_retained_call`] admits ONE explicitly identified function — the
//! module entrypoint or any other admitted function in the same resolved
//! program — validates its signature against this seam's closed value
//! vocabulary, scans its transitive closure once, and retains the authority-
//! free dispatch index. [`evaluate_retained_call`] then executes that retained
//! product any number of times with different arguments. It reads no source,
//! re-resolves nothing, and re-runs neither `hir::validate` nor the closure
//! scan; it only re-checks that the retained vector positions still name the
//! same identities and that the signature is unchanged, and fails closed when
//! they are not.
//!
//! This is additive. The frozen zero-argument entrypoint product
//! (`prepare_resolved_zero_arg_i64`) keeps its exact admission rules,
//! its `entry_id == program.entrypoint` requirement, and its known answers;
//! both products now share one owner for the retained dispatch index.
//!
//! # Closed value vocabulary
//!
//! Arguments and results are the monomorphic scalar record/variant subset of
//! Agent Proposal Schema v1 that the interpreter can actually execute:
//! `bool`, `i32`, `i64`, `u8`, `usize`, owned `Bytes`, and bounded acyclic
//! records, classes, and owned-byte variants over exactly those leaves.
//!
//! Deliberate exclusions, each an explicit located `SPX-F102` admission
//! diagnostic rather than a panic or a silent widening:
//!
//! - `String`/`str`. Agent Proposal Schema v1 admits a `string` field, but no
//!   interpreter record or variant classifier does: a `String` leaf would be a
//!   new owned cleanup leaf kind, and inventing one here would put the
//!   interpreter's cleanup shape ahead of the shared cleanup machinery and the
//!   native and Wasm backends. Owned UTF-8 stays on its own profile.
//! - `char`, `f32`, `f64`. Excluded by the proposal schema's exact-transport
//!   rule, so this seam does not transport them either.
//! - `Slice<u8>`, fixed byte arrays, and every generic or type-parameter
//!   shape. Borrowed host carriers already have their own public route.
//!
//! # Ownership and cleanup
//!
//! Nothing here reimplements ownership. Admission runs the interpreter's own
//! `admitted_resolved_functions` and `scan_closure`, so an
//! ownership error is a compile-time diagnostic before any evaluation.
//! Execution enters through `Evaluator::call_frame`, the same frame the frozen
//! products use, so contracts, sticky failure selection, fuel, and call depth
//! behave identically. A staged owned `Bytes` argument is charged against the
//! same verified byte-data capacity a `bytes_copy` would consume, so a host
//! argument can never mint capacity the program did not have. Copy carriers
//! alias by `Arc` on this backend, so a Copy subtree is harvested by reference
//! while an owned subtree must be uniquely held and fails closed otherwise.
//! Result carriers are settled leaf by leaf in declared field order — the
//! structural order the record's own inventory records — and each settled
//! `Bytes` carrier produces exactly one boundary cleanup event. Cleanup order
//! is read from that inventory; it is never sorted or repaired here.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ast::Span;
use crate::conformance::NormalizedStatus;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ResolvedFunction, ResolvedType};

use super::prepared::{index_closure, index_matches_program, PreparedFunctionIndex};
use super::{
    admitted_resolved_functions, argument_error, guard_error, is_admitted_owned_byte_record,
    is_admitted_owned_byte_variant, option_error, record_construction_is_admitted, scan_closure,
    selection_error, settle_interpreted_bytes, ContractFailureDetail, Evaluator, Flow,
    FunctionLookup, OwnedBytesValue, OwnedDataCleanupEvent, OwnedRecordValue, OwnedVariantValue,
    PreparedCancellation, Value, EVALUATION_STACK_BYTES, MAX_STEPS_LIMIT,
    REASON_AUTOMATIC_IDENTITY, REASON_UNSUPPORTED_CALLEE, REASON_UNSUPPORTED_PARAMETER_TYPE,
    REASON_UNSUPPORTED_RESULT_TYPE,
};

/// Maximum parameters one retained call may declare.
pub const MAX_RETAINED_CALL_PARAMETERS: usize = 8;

/// One argument or result value in the retained call vocabulary.
///
/// Records and variants are keyed exclusively by persistent stable identity.
/// Source display names never participate, so a display rename cannot select
/// a different field, case, or carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedValue {
    Bool(bool),
    I32(i32),
    I64(i64),
    U8(u8),
    Usize(u64),
    /// One owned byte payload. Staging copies it into a fresh interpreter
    /// allocation; harvesting copies a settled carrier back out.
    Bytes(Vec<u8>),
    Record(RetainedRecord),
    Variant(RetainedVariant),
}

/// One record or class carrier addressed by its stable declaration identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedRecord {
    pub record: DeclarationId,
    /// Every declared field exactly once. Harvested values are in declared
    /// field order; supplied arguments may be in any order.
    pub fields: Vec<RetainedField>,
}

/// One variant carrier: its nominal identity plus exactly one active case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedVariant {
    pub variant: DeclarationId,
    pub case: DeclarationId,
    pub fields: Vec<RetainedField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedField {
    pub field: DeclarationId,
    pub value: RetainedValue,
}

/// Closed outcomes of one retained invocation. A language-level failure is
/// distinct from a fail-closed interpreter capacity limit, and both are
/// distinct from an impossible post-verify state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetainedCallOutcome {
    Returned(RetainedValue),
    LanguageFailure(NormalizedStatus),
    FuelExhausted,
    CallDepthExceeded,
    GuardError(String),
}

/// Deterministic, authority-free facts for one retained invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedCallEvaluation {
    pub function_id: DeclarationId,
    pub outcome: RetainedCallOutcome,
    /// One event per settled owned `Bytes` carrier, in declared field order.
    pub cleanup_events: Vec<OwnedDataCleanupEvent>,
    pub steps_used: usize,
    pub max_steps: usize,
    /// The violated clause and frame when `outcome` is a contract failure.
    pub failure: Option<ContractFailureDetail>,
}

/// The retained, authority-free product of one admitted retained call.
///
/// It holds owned identities, retained vector positions, and the exact
/// signature it was admitted against. It borrows no HIR and grants no
/// filesystem, process, network, or backend authority.
#[derive(Debug)]
pub struct PreparedRetainedCall {
    entry_id: String,
    entry_index: usize,
    function_indices: BTreeMap<String, PreparedFunctionIndex>,
    signature: RetainedSignature,
    origin_nodes: usize,
    index_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RetainedSignature {
    params: Vec<ResolvedType>,
    result: ResolvedType,
}

impl RetainedSignature {
    fn of(function: &ResolvedFunction) -> Self {
        Self {
            params: function
                .params
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect(),
            result: function.return_type.clone(),
        }
    }
}

impl PreparedRetainedCall {
    /// The exact stable identity this product dispatches.
    #[must_use]
    pub fn function_id(&self) -> &str {
        &self.entry_id
    }

    /// Every admitted function identity retained in the dispatch closure.
    pub fn function_ids(&self) -> impl Iterator<Item = &str> {
        self.function_indices.keys().map(String::as_str)
    }

    /// The declared parameter count one invocation must supply exactly.
    #[must_use]
    pub fn parameter_count(&self) -> usize {
        self.signature.params.len()
    }

    #[must_use]
    pub const fn origin_nodes(&self) -> usize {
        self.origin_nodes
    }

    #[must_use]
    pub const fn index_bytes(&self) -> usize {
        self.index_bytes
    }
}

fn located(reason: &str, detail: String, span: Span) -> Diagnostic {
    Diagnostic::error(
        "SPX-F102",
        format!("interpreter admission failed ({reason}): {detail}"),
        span,
    )
}

/// Admit and retain one non-entrypoint or entrypoint call once.
///
/// The caller owns validation and supplies the exact resolved program. Every
/// rejection is an explicit diagnostic naming a closed reason: an unresolved
/// identity, a non-persistent identity, a signature outside the interpreter
/// profile, an argument or result type outside this seam's closed vocabulary,
/// and any body shape the interpreter does not execute all fail closed here,
/// before any evaluation is possible.
pub fn prepare_retained_call(
    program: &hir::ResolvedProgram,
    entry_id: &str,
) -> Result<PreparedRetainedCall, Vec<Diagnostic>> {
    hir::validate(program).map_err(|diagnostic| vec![diagnostic])?;
    hir::analyze_byte_data_capacity(program).map_err(|diagnostic| vec![diagnostic])?;
    let entry_index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == entry_id)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("retained call target `{entry_id}` is absent from the function index"),
            )]
        })?;
    let entry = &program.functions[entry_index];
    let explicit_entry = program
        .declarations
        .declaration(&entry.id)
        .is_some_and(|declaration| declaration.identity_origin == hir::IdentityOrigin::Explicit);
    if !explicit_entry {
        return Err(vec![located(
            REASON_AUTOMATIC_IDENTITY,
            format!("retained call target `{entry_id}` has no explicit stable identity"),
            entry.span,
        )]);
    }
    if entry.params.len() > MAX_RETAINED_CALL_PARAMETERS {
        return Err(vec![located(
            REASON_UNSUPPORTED_PARAMETER_TYPE,
            format!(
                "retained call target `{entry_id}` declares {} parameters; the limit is {MAX_RETAINED_CALL_PARAMETERS}",
                entry.params.len()
            ),
            entry.span,
        )]);
    }
    for (index, parameter) in entry.params.iter().enumerate() {
        if !retained_shape_is_admitted(&program.declarations, &parameter.ty) {
            return Err(vec![located(
                REASON_UNSUPPORTED_PARAMETER_TYPE,
                format!(
                    "retained call target `{entry_id}` parameter {index} (`{}`) has type `{}`, which is outside the retained call vocabulary",
                    parameter.name,
                    parameter.ty.identity_key()
                ),
                parameter.span,
            )]);
        }
    }
    if !retained_shape_is_admitted(&program.declarations, &entry.return_type) {
        return Err(vec![located(
            REASON_UNSUPPORTED_RESULT_TYPE,
            format!(
                "retained call target `{entry_id}` returns `{}`, which is outside the retained call vocabulary",
                entry.return_type.identity_key()
            ),
            entry.span,
        )]);
    }
    let admitted = admitted_resolved_functions(program);
    if !admitted.contains_key(entry_id) {
        return Err(vec![located(
            REASON_UNSUPPORTED_CALLEE,
            format!("retained call target `{entry_id}` is outside the interpreter profile"),
            entry.span,
        )]);
    }
    let closure = scan_closure(entry_id, &admitted, &program.declarations)?;
    let index = index_closure(program, entry_id, &closure)?;
    Ok(PreparedRetainedCall {
        entry_id: entry_id.to_owned(),
        entry_index,
        function_indices: index.function_indices,
        signature: RetainedSignature::of(entry),
        origin_nodes: index.origin_nodes,
        index_bytes: index.index_bytes,
    })
}

/// Execute one retained product with typed arguments.
///
/// No source is read, no module is re-resolved, and neither `hir::validate`
/// nor the closure scan runs again. The retained identities and the admitted
/// signature are re-checked against the supplied program first, so a drifted
/// or substituted program fails closed instead of dispatching to a different
/// function body.
pub fn evaluate_retained_call(
    program: &hir::ResolvedProgram,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
    max_steps: usize,
) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
    if !(1..=MAX_STEPS_LIMIT).contains(&max_steps) {
        return Err(vec![option_error(format!(
            "retained call evaluation requires max_steps 1..={MAX_STEPS_LIMIT}"
        ))]);
    }
    if !index_matches_program(
        program,
        &prepared.entry_id,
        prepared.entry_index,
        &prepared.function_indices,
    ) {
        return Err(vec![guard_error(
            "retained call closure no longer matches its resolved program",
        )]);
    }
    let entry = &program.functions[prepared.entry_index];
    if RetainedSignature::of(entry) != prepared.signature {
        return Err(vec![guard_error(
            "retained call target signature no longer matches its admitted signature",
        )]);
    }
    if arguments.len() != entry.params.len() {
        return Err(vec![argument_error(format!(
            "retained call `{}` takes {} argument(s), {} were provided",
            prepared.entry_id,
            entry.params.len(),
            arguments.len()
        ))]);
    }

    // Stage every argument before the evaluator exists. A shape or capacity
    // mismatch is an argument diagnostic, never an evaluator guard.
    let mut staging = ByteStaging::default();
    let mut values = Vec::with_capacity(entry.params.len());
    for (index, (parameter, argument)) in entry.params.iter().zip(arguments).enumerate() {
        let value = stage(&program.declarations, &parameter.ty, argument, &mut staging).map_err(
            |detail| {
                vec![argument_error(format!(
                    "retained call `{}` argument {index} (`{}`): {detail}",
                    prepared.entry_id, parameter.name
                ))]
            },
        )?;
        values.push((parameter.id.clone(), value));
    }
    let return_type = entry.return_type.clone();
    let lookup = FunctionLookup::Prepared {
        functions: &program.functions,
        function_instances: &program.function_instances,
        indices: &prepared.function_indices,
    };

    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("semaprax-retained-call".to_owned())
            .stack_size(EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, move || {
                let mut evaluator = Evaluator::new_prepared(
                    lookup,
                    &program.declarations,
                    max_steps,
                    0,
                    PreparedCancellation::Never,
                );
                // Host-staged carriers occupy the same verified byte-data
                // capacity a `bytes_copy` would, so an argument can never mint
                // allocation identity or payload the program did not have.
                evaluator.next_byte_allocation = staging.allocations;
                evaluator.allocated_byte_payload = staging.payload;
                let evaluated = evaluator.call_frame(entry, values, 0);
                let mut cleanup_events = Vec::new();
                let outcome = match evaluated {
                    Ok(value) => match harvest(
                        &program.declarations,
                        &return_type,
                        value,
                        &mut cleanup_events,
                    ) {
                        Ok(value) => RetainedCallOutcome::Returned(value),
                        Err(detail) => RetainedCallOutcome::GuardError(detail),
                    },
                    Err(Flow::Failure(status)) => RetainedCallOutcome::LanguageFailure(status),
                    Err(Flow::Exhausted) => RetainedCallOutcome::FuelExhausted,
                    Err(Flow::DepthExceeded) => RetainedCallOutcome::CallDepthExceeded,
                    Err(Flow::Cancelled { .. }) => RetainedCallOutcome::GuardError(
                        "unexpected cancellation in retained call evaluation".to_owned(),
                    ),
                    Err(Flow::Utf8MaterializationLimitExceeded { .. }) => {
                        RetainedCallOutcome::GuardError(
                            "unexpected UTF-8 materialization limit in retained call evaluation"
                                .to_owned(),
                        )
                    }
                    Err(Flow::Residual(_)) => RetainedCallOutcome::GuardError(
                        super::owned_try::ESCAPED_RESIDUAL_GUARD.to_owned(),
                    ),
                    Err(Flow::Guard(detail)) => RetainedCallOutcome::GuardError(detail.to_owned()),
                };
                RetainedCallEvaluation {
                    function_id: entry.id.clone(),
                    outcome,
                    cleanup_events,
                    steps_used: evaluator.steps,
                    max_steps,
                    failure: evaluator.failure_detail.take(),
                }
            })
            .map_err(|error| {
                vec![guard_error(&format!(
                    "retained call evaluation thread failed to start: {error}"
                ))]
            })?;
        worker.join().map_err(|_| {
            vec![guard_error(
                "retained call evaluation thread panicked after HIR validation",
            )]
        })
    })
}

/// The closed direct leaf vocabulary of this seam.
fn retained_leaf_is_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Bool
            | ResolvedType::I32
            | ResolvedType::I64
            | ResolvedType::U8
            | ResolvedType::Usize
    )
}

/// Admit one complete argument or result shape.
///
/// A nominal shape must first be admitted by the interpreter's own classifier
/// — so it is bounded, acyclic, and executable — and only then is every leaf
/// restricted to this seam's closed vocabulary.
fn retained_shape_is_admitted(declarations: &hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if retained_leaf_is_admitted(ty) || *ty == ResolvedType::Bytes {
        return true;
    }
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return false;
    };
    let Some(item) = declarations.declaration(declaration) else {
        return false;
    };
    match item.kind {
        hir::DeclarationKind::Record | hir::DeclarationKind::Class => {
            record_construction_is_admitted(declarations, ty)
                && record_leaves_are_admitted(declarations, ty)
        }
        hir::DeclarationKind::Variant => {
            is_admitted_owned_byte_variant(declarations, ty)
                && variant_leaves_are_admitted(declarations, ty)
        }
        _ => false,
    }
}

/// Walk an already bounded, already acyclic record carrier and require every
/// leaf to be `Bytes` or an admitted direct scalar.
fn record_leaves_are_admitted(declarations: &hir::DeclarationIndex, root: &ResolvedType) -> bool {
    let mut pending = vec![root.clone()];
    let mut visited = 0usize;
    while let Some(ty) = pending.pop() {
        if retained_leaf_is_admitted(&ty) || ty == ResolvedType::Bytes {
            continue;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return false;
        };
        let Some(fields) = declarations.record_fields(declaration) else {
            return false;
        };
        visited = match visited.checked_add(fields.len()) {
            Some(total) if total <= crate::cleanup::MAX_CLEANUP_VISITED_FIELDS => total,
            _ => return false,
        };
        for field in fields {
            let Ok(field_ty) = hir::substitute_type(&field.ty, declaration, arguments) else {
                return false;
            };
            pending.push(field_ty);
        }
    }
    true
}

/// Owned-byte variant case payloads are flat by construction, so each case
/// field must be `Bytes` or an admitted direct scalar.
fn variant_leaves_are_admitted(declarations: &hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let Some(cases) = declarations.variant_cases(declaration) else {
        return false;
    };
    cases.iter().all(|case| {
        case.fields.iter().all(|field| {
            hir::substitute_type(&field.ty, declaration, arguments)
                .is_ok_and(|ty| ty == ResolvedType::Bytes || retained_leaf_is_admitted(&ty))
        })
    })
}

/// Byte-data capacity consumed by host-staged owned carriers.
#[derive(Default)]
struct ByteStaging {
    allocations: u32,
    payload: u64,
}

impl ByteStaging {
    fn allocate(&mut self, bytes: &[u8]) -> Result<Value, String> {
        let length = u64::try_from(bytes.len())
            .map_err(|_| "owned byte payload length does not fit u64".to_owned())?;
        if length > crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES {
            return Err(format!(
                "owned byte payload exceeds the {} byte external root limit",
                crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES
            ));
        }
        let allocation = self
            .allocations
            .checked_add(1)
            .ok_or_else(|| "owned byte allocation count overflowed".to_owned())?;
        if allocation > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
            return Err(format!(
                "staged owned byte carriers exceed the verified capacity of {}",
                crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            ));
        }
        let payload = self
            .payload
            .checked_add(length)
            .ok_or_else(|| "owned byte payload accounting overflowed".to_owned())?;
        if payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
            return Err(format!(
                "staged owned byte payload exceeds the verified capacity of {} bytes",
                crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
            ));
        }
        self.allocations = allocation;
        self.payload = payload;
        Ok(Value::Bytes(OwnedBytesValue {
            allocation,
            bytes: Arc::from(bytes),
        }))
    }
}

fn nominal(ty: &ResolvedType) -> Result<(&DeclarationId, &[ResolvedType]), String> {
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => Ok((declaration, arguments.as_slice())),
        _ => Err(format!(
            "expected a nominal carrier, the declared type is `{}`",
            ty.identity_key()
        )),
    }
}

/// Look up the single supplied value for one declared field identity,
/// rejecting a missing field, a repeat, and any unknown extra.
fn exactly_once<'a>(
    supplied: &'a [RetainedField],
    declared: &DeclarationId,
) -> Result<&'a RetainedValue, String> {
    let mut found = None;
    for field in supplied {
        if field.field == *declared {
            if found.is_some() {
                return Err(format!("field `{declared}` was supplied more than once"));
            }
            found = Some(&field.value);
        }
    }
    found.ok_or_else(|| format!("field `{declared}` was not supplied"))
}

/// Build one interpreter carrier for an argument, driven by the declared type.
fn stage(
    declarations: &hir::DeclarationIndex,
    expected: &ResolvedType,
    value: &RetainedValue,
    staging: &mut ByteStaging,
) -> Result<Value, String> {
    match (expected, value) {
        (ResolvedType::Bool, RetainedValue::Bool(value)) => Ok(Value::Bool(*value)),
        (ResolvedType::I32, RetainedValue::I32(value)) => Ok(Value::Int32(*value)),
        (ResolvedType::I64, RetainedValue::I64(value)) => Ok(Value::Int(*value)),
        (ResolvedType::U8, RetainedValue::U8(value)) => Ok(Value::Uint8(*value)),
        (ResolvedType::Usize, RetainedValue::Usize(value)) => Ok(Value::Usize(*value)),
        (ResolvedType::Bytes, RetainedValue::Bytes(bytes)) => staging.allocate(bytes),
        (ResolvedType::Nominal { .. }, RetainedValue::Record(record)) => {
            let (declaration, arguments) = nominal(expected)?;
            if record.record != *declaration {
                return Err(format!(
                    "record identity `{}` disagrees with the declared `{declaration}`",
                    record.record
                ));
            }
            let declared = declarations
                .record_fields(declaration)
                .ok_or_else(|| format!("`{declaration}` has no record field inventory"))?;
            if record.fields.len() != declared.len() {
                return Err(format!(
                    "record `{declaration}` declares {} field(s), {} were supplied",
                    declared.len(),
                    record.fields.len()
                ));
            }
            let mut fields = BTreeMap::new();
            for field in declared {
                let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
                    .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
                let supplied = exactly_once(&record.fields, &field.id)?;
                fields.insert(
                    field.id.clone(),
                    stage(declarations, &field_ty, supplied, staging)?,
                );
            }
            Ok(Value::Record(Arc::new(OwnedRecordValue {
                record: declaration.clone(),
                fields,
            })))
        }
        (ResolvedType::Nominal { .. }, RetainedValue::Variant(variant)) => {
            let (declaration, arguments) = nominal(expected)?;
            if variant.variant != *declaration {
                return Err(format!(
                    "variant identity `{}` disagrees with the declared `{declaration}`",
                    variant.variant
                ));
            }
            let case = declarations
                .variant_cases(declaration)
                .ok_or_else(|| format!("`{declaration}` has no variant case inventory"))?
                .iter()
                .find(|candidate| candidate.id == variant.case)
                .ok_or_else(|| {
                    format!(
                        "`{}` is not a declared case of variant `{declaration}`",
                        variant.case
                    )
                })?;
            if variant.fields.len() != case.fields.len() {
                return Err(format!(
                    "case `{}` declares {} payload field(s), {} were supplied",
                    case.id,
                    case.fields.len(),
                    variant.fields.len()
                ));
            }
            let mut fields = BTreeMap::new();
            for field in &case.fields {
                let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
                    .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
                let supplied = exactly_once(&variant.fields, &field.id)?;
                fields.insert(
                    field.id.clone(),
                    stage(declarations, &field_ty, supplied, staging)?,
                );
            }
            Ok(Value::Variant(Arc::new(OwnedVariantValue {
                ty: expected.clone(),
                variant: declaration.clone(),
                case: case.id.clone(),
                fields,
            })))
        }
        _ => Err(format!(
            "supplied value does not match the declared type `{}`",
            expected.identity_key()
        )),
    }
}

/// Copy one interpreter result carrier out, settling every owned leaf.
fn harvest(
    declarations: &hir::DeclarationIndex,
    expected: &ResolvedType,
    value: Value,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<RetainedValue, String> {
    match (expected, value) {
        (ResolvedType::Bool, Value::Bool(value)) => Ok(RetainedValue::Bool(value)),
        (ResolvedType::I32, Value::Int32(value)) => Ok(RetainedValue::I32(value)),
        (ResolvedType::I64, Value::Int(value)) => Ok(RetainedValue::I64(value)),
        (ResolvedType::U8, Value::Uint8(value)) => Ok(RetainedValue::U8(value)),
        (ResolvedType::Usize, Value::Usize(value)) => Ok(RetainedValue::Usize(value)),
        (ResolvedType::Bytes, value @ Value::Bytes(_)) => {
            settle_interpreted_bytes(value, cleanup_events)
                .map(RetainedValue::Bytes)
                .map_err(str::to_owned)
        }
        (ResolvedType::Nominal { .. }, Value::Record(record)) => {
            harvest_record(declarations, expected, record, cleanup_events)
        }
        (ResolvedType::Nominal { .. }, Value::Variant(variant)) => {
            harvest_variant(declarations, expected, variant, cleanup_events)
        }
        _ => Err(format!(
            "retained result carrier disagrees with its declared type `{}`",
            expected.identity_key()
        )),
    }
}

fn harvest_record(
    declarations: &hir::DeclarationIndex,
    expected: &ResolvedType,
    record: Arc<OwnedRecordValue>,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<RetainedValue, String> {
    let (declaration, arguments) = nominal(expected)?;
    let declared = declarations
        .record_fields(declaration)
        .ok_or_else(|| format!("`{declaration}` has no record field inventory"))?;
    if record.record != *declaration || record.fields.len() != declared.len() {
        return Err(
            "retained result record identity or field inventory disagrees with its signature"
                .to_owned(),
        );
    }
    let mut fields = Vec::with_capacity(declared.len());
    if is_admitted_owned_byte_record(declarations, expected) {
        // An owned subtree must be uniquely held before any leaf is settled.
        let mut record = Arc::try_unwrap(record)
            .map_err(|_| "retained owned result still has a live alias at copy-out".to_owned())?;
        for field in declared {
            let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
                .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
            let value = record
                .fields
                .remove(&field.id)
                .ok_or_else(|| format!("result carrier omits field `{}`", field.id))?;
            fields.push(RetainedField {
                field: field.id.clone(),
                value: harvest(declarations, &field_ty, value, cleanup_events)?,
            });
        }
        if !record.fields.is_empty() {
            return Err("retained result carrier retained an unauthenticated field".to_owned());
        }
    } else {
        // A Copy carrier owns no cleanup leaf and aliases by handle on this
        // backend, so it is read rather than consumed.
        for field in declared {
            let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
                .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
            let value = record
                .fields
                .get(&field.id)
                .ok_or_else(|| format!("result carrier omits field `{}`", field.id))?;
            fields.push(RetainedField {
                field: field.id.clone(),
                value: harvest_copy(declarations, &field_ty, value)?,
            });
        }
    }
    Ok(RetainedValue::Record(RetainedRecord {
        record: declaration.clone(),
        fields,
    }))
}

fn harvest_copy(
    declarations: &hir::DeclarationIndex,
    expected: &ResolvedType,
    value: &Value,
) -> Result<RetainedValue, String> {
    match (expected, value) {
        (ResolvedType::Bool, Value::Bool(value)) => Ok(RetainedValue::Bool(*value)),
        (ResolvedType::I32, Value::Int32(value)) => Ok(RetainedValue::I32(*value)),
        (ResolvedType::I64, Value::Int(value)) => Ok(RetainedValue::I64(*value)),
        (ResolvedType::U8, Value::Uint8(value)) => Ok(RetainedValue::U8(*value)),
        (ResolvedType::Usize, Value::Usize(value)) => Ok(RetainedValue::Usize(*value)),
        (ResolvedType::Nominal { .. }, Value::Record(record)) => {
            let (declaration, arguments) = nominal(expected)?;
            let declared = declarations
                .record_fields(declaration)
                .ok_or_else(|| format!("`{declaration}` has no record field inventory"))?;
            if record.record != *declaration || record.fields.len() != declared.len() {
                return Err(
                    "retained Copy result identity or field inventory disagrees with its signature"
                        .to_owned(),
                );
            }
            let mut fields = Vec::with_capacity(declared.len());
            for field in declared {
                let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
                    .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
                let value = record
                    .fields
                    .get(&field.id)
                    .ok_or_else(|| format!("Copy result carrier omits field `{}`", field.id))?;
                fields.push(RetainedField {
                    field: field.id.clone(),
                    value: harvest_copy(declarations, &field_ty, value)?,
                });
            }
            Ok(RetainedValue::Record(RetainedRecord {
                record: declaration.clone(),
                fields,
            }))
        }
        _ => Err(format!(
            "retained Copy result carrier disagrees with its declared type `{}`",
            expected.identity_key()
        )),
    }
}

fn harvest_variant(
    declarations: &hir::DeclarationIndex,
    expected: &ResolvedType,
    variant: Arc<OwnedVariantValue>,
    cleanup_events: &mut Vec<OwnedDataCleanupEvent>,
) -> Result<RetainedValue, String> {
    let (declaration, arguments) = nominal(expected)?;
    if variant.ty != *expected || variant.variant != *declaration {
        return Err("retained result variant carrier type disagrees with its signature".to_owned());
    }
    let case = declarations
        .variant_cases(declaration)
        .ok_or_else(|| format!("`{declaration}` has no variant case inventory"))?
        .iter()
        .find(|candidate| candidate.id == variant.case)
        .ok_or_else(|| "retained result variant has an unauthenticated active case".to_owned())?;
    if variant.fields.len() != case.fields.len() {
        return Err("retained result variant payload inventory disagrees with its case".to_owned());
    }
    let case_id = case.id.clone();
    let mut carrier = Arc::try_unwrap(variant)
        .map_err(|_| "retained variant result still has a live alias at copy-out".to_owned())?;
    let mut fields = Vec::with_capacity(case.fields.len());
    for field in &case.fields {
        let field_ty = hir::substitute_type(&field.ty, declaration, arguments)
            .map_err(|_| format!("field `{}` type substitution failed", field.id))?;
        let value = carrier
            .fields
            .remove(&field.id)
            .ok_or_else(|| format!("result variant omits payload field `{}`", field.id))?;
        fields.push(RetainedField {
            field: field.id.clone(),
            value: harvest(declarations, &field_ty, value, cleanup_events)?,
        });
    }
    if !carrier.fields.is_empty() {
        return Err("retained result variant retained an unauthenticated payload".to_owned());
    }
    Ok(RetainedValue::Variant(RetainedVariant {
        variant: declaration.clone(),
        case: case_id,
        fields,
    }))
}
