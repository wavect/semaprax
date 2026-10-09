//! Deterministic compiler-owned lowering for the admitted `.spx` resumable
//! effect slice.
//!
//! This module implements bounded pure Copy-scalar replay history, not live
//! frames or liveness-based continuation lowering. It accepts at most eight
//! direct top-level `yield` sites, Copy-scalar state, no
//! ordinary effects, and one explicitly identified free function. Each
//! projection retains only that function's direct-call closure and the valid
//! entrypoint closure required by target validation, so disconnected yielding
//! functions remain independent. The module then derives one target-neutral
//! ordered-state plan and yield-free HIR projections for the interpreter and
//! test-only backend parity lanes.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ExpressionId, FunctionExecutionId, IdentityOrigin, OwnershipMode, Place,
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedParam, ResolvedProgram,
    ResolvedStatement, ResolvedType, ValueId,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) mod control;
#[cfg(test)]
mod control_tests;
mod projection;
mod sequential;
#[cfg(test)]
mod sequential_tests;
use projection::{projection_program, resume_projection, start_projection};

const INVALID_RESUMABLE_PLAN: &str = "SPX-H006";
const PLAN_IDENTITY_DOMAIN: &[u8] = b"semaprax.resumable-plan.v2\0";
const SUSPENSION_BINDING_DOMAIN: &[u8] = b"semaprax.resumable-suspension-binding.v2\0";
pub const MAX_RESUMABLE_YIELDS: usize = 8;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResumablePlanIdentity([u8; 32]);

impl ResumablePlanIdentity {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResumableSuspensionBinding([u8; 32]);

impl ResumableSuspensionBinding {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact target-neutral scalar bits used to bind one suspension to the
/// invocation arguments that produced it. Floats are bits, not IEEE equality.
/// `Record` and `Variant` carry one bounded channel level in declared field
/// order. Function arguments remain Copy-scalar; only request/answer history
/// carries these shapes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResumableScalar {
    I64(i64),
    I32(i32),
    U8(u8),
    Usize(u64),
    Char(u32),
    F32(u32),
    F64(u64),
    Bool(bool),
    Record(Vec<ResumableScalar>),
    Variant {
        case: String,
        fields: Vec<ResumableScalar>,
    },
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResumableStateId(String);

impl ResumableStateId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumableStateKind {
    Entry,
    Suspended,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumableState {
    pub id: ResumableStateId,
    pub kind: ResumableStateKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumableStatementKind {
    Let,
    Assign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumableYieldPosition {
    Statement {
        index: u32,
        kind: ResumableStatementKind,
    },
    Tail,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumableSuspension {
    pub state: ResumableState,
    pub expression: ExpressionId,
    pub request_expression: ExpressionId,
    pub position: ResumableYieldPosition,
    pub request_type: ResolvedType,
    pub response_type: ResolvedType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumableProjection {
    pub function: ResolvedFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumablePlan {
    pub function_id: DeclarationId,
    pub identity: ResumablePlanIdentity,
    pub entry: ResumableState,
    pub suspension: ResumableSuspension,
    pub complete: ResumableState,
    pub start: ResumableProjection,
    pub resume: ResumableProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequentialResumablePlan {
    pub function_id: DeclarationId,
    pub identity: ResumablePlanIdentity,
    pub entry: ResumableState,
    pub suspensions: Vec<ResumableSuspension>,
    pub complete: ResumableState,
    pub start: ResumableProjection,
    pub resumes: Vec<ResumableProjection>,
}

impl ResumablePlan {
    pub fn suspension_binding(&self, arguments: &[ResumableScalar]) -> ResumableSuspensionBinding {
        suspension_binding(&self.identity, &self.suspension, arguments, &[])
    }

    pub fn start_program(&self, program: &ResolvedProgram) -> Result<ResolvedProgram, Diagnostic> {
        self.authenticate_program(program)?;
        projection_program(program, &self.function_id, &self.start.function)
    }

    pub fn resume_program(&self, program: &ResolvedProgram) -> Result<ResolvedProgram, Diagnostic> {
        self.authenticate_program(program)?;
        projection_program(program, &self.function_id, &self.resume.function)
    }

    fn authenticate_program(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == self.function_id)
            .ok_or_else(|| invalid("projection program lacks the plan's function"))?;
        let sites = locate_direct_yields(function)?;
        let observed = plan_identity(program, function, &sites)?;
        if observed != self.identity {
            return Err(invalid(
                "projection program does not match the plan's checked-program identity",
            ));
        }
        let expected = lower(program, function)?;
        if expected != *self {
            return Err(invalid(
                "resumable plan is not the exact deterministic lowering of the checked program",
            ));
        }
        Ok(())
    }
}

impl SequentialResumablePlan {
    pub fn suspension_binding(&self, arguments: &[ResumableScalar]) -> ResumableSuspensionBinding {
        self.suspension_binding_at(0, arguments, &[])
            .expect("a lowered resumable plan has its first suspension")
    }

    /// Bind a site to the exact original arguments and preceding answer bits.
    pub fn suspension_binding_at(
        &self,
        index: usize,
        arguments: &[ResumableScalar],
        prior_answers: &[ResumableScalar],
    ) -> Result<ResumableSuspensionBinding, Diagnostic> {
        let suspension = self
            .suspensions
            .get(index)
            .ok_or_else(|| invalid("resumable suspension index is out of bounds"))?;
        if prior_answers.len() != index {
            return Err(invalid(
                "resumable suspension answer history length disagrees with its site",
            ));
        }
        Ok(suspension_binding(
            &self.identity,
            suspension,
            arguments,
            prior_answers,
        ))
    }

    pub fn suspension_index(&self, state: &ResumableStateId) -> Option<usize> {
        self.suspensions
            .iter()
            .position(|site| site.state.id == *state)
    }

    pub fn start_program(&self, program: &ResolvedProgram) -> Result<ResolvedProgram, Diagnostic> {
        self.authenticate_program(program)?;
        projection_program(program, &self.function_id, &self.start.function)
    }

    pub fn resume_program(&self, program: &ResolvedProgram) -> Result<ResolvedProgram, Diagnostic> {
        self.resume_program_at(program, 0)
    }

    pub fn resume_program_at(
        &self,
        program: &ResolvedProgram,
        index: usize,
    ) -> Result<ResolvedProgram, Diagnostic> {
        self.authenticate_program(program)?;
        let resume = self
            .resumes
            .get(index)
            .ok_or_else(|| invalid("resumable resume index is out of bounds"))?;
        projection_program(program, &self.function_id, &resume.function)
    }

    fn authenticate_program(&self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        let function = program
            .functions
            .iter()
            .find(|function| function.id == self.function_id)
            .ok_or_else(|| invalid("projection program lacks the plan's function"))?;
        let sites = locate_direct_yields(function)?;
        let observed = plan_identity(program, function, &sites)?;
        if observed != self.identity {
            return Err(invalid(
                "projection program does not match the plan's checked-program identity",
            ));
        }
        let expected = lower_sequential(program, function)?;
        if expected != *self {
            return Err(invalid(
                "resumable plan is not the exact deterministic lowering of the checked program",
            ));
        }
        Ok(())
    }
}

fn suspension_binding(
    identity: &ResumablePlanIdentity,
    suspension: &ResumableSuspension,
    arguments: &[ResumableScalar],
    prior_answers: &[ResumableScalar],
) -> ResumableSuspensionBinding {
    let mut hasher = Sha256::new();
    hasher.update(SUSPENSION_BINDING_DOMAIN);
    frame(&mut hasher, identity.as_bytes());
    frame(&mut hasher, suspension.state.id.as_str().as_bytes());
    frame(&mut hasher, suspension.expression.as_str().as_bytes());
    hasher.update((arguments.len() as u64).to_le_bytes());
    for argument in arguments {
        hash_scalar(&mut hasher, argument);
    }
    hasher.update((prior_answers.len() as u64).to_le_bytes());
    for answer in prior_answers {
        hash_scalar(&mut hasher, answer);
    }
    ResumableSuspensionBinding(hasher.finalize().into())
}

/// Lower the original one-site profile into its source-compatible plan.
pub fn lower(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<ResumablePlan, Diagnostic> {
    let plan = lower_sequential(program, function)?;
    if plan.suspensions.len() != 1 {
        return Err(invalid(
            "legacy resumable lowering requires exactly one yield site; use `lower_sequential`",
        ));
    }
    Ok(ResumablePlan {
        function_id: plan.function_id,
        identity: plan.identity,
        entry: plan.entry,
        suspension: plan.suspensions.into_iter().next().expect("one suspension"),
        complete: plan.complete,
        start: plan.start,
        resume: plan.resumes.into_iter().next().expect("one resume"),
    })
}

/// Lower the bounded sequential profile into an ordered plan.
pub fn lower_sequential(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<SequentialResumablePlan, Diagnostic> {
    sequential::lower(program, function, false)
}

/// Lower with the distinct aggregate whole-function carrier. Scalar entry
/// points, checkpoint codecs, and journals remain on [`lower_sequential`].
pub(crate) fn lower_sequential_with_arguments(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<SequentialResumablePlan, Diagnostic> {
    sequential::lower(program, function, true)
}

fn plan_identity(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    sites: &[YieldSite<'_>],
) -> Result<ResumablePlanIdentity, Diagnostic> {
    let encoded = crate::cache_codec::encode(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(|| invalid("checked-program encoding failed without a diagnostic"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(PLAN_IDENTITY_DOMAIN);
    frame(&mut hasher, &encoded);
    frame(&mut hasher, function.id.as_str().as_bytes());
    hasher.update((sites.len() as u64).to_le_bytes());
    for (expression, _, _) in sites {
        frame(&mut hasher, expression.id.as_str().as_bytes());
    }
    Ok(ResumablePlanIdentity(hasher.finalize().into()))
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn hash_scalar(hasher: &mut Sha256, value: &ResumableScalar) {
    match value {
        ResumableScalar::I64(value) => {
            hasher.update([0u8]);
            hasher.update(value.to_le_bytes());
        }
        ResumableScalar::I32(value) => {
            hasher.update([1u8]);
            hasher.update(value.to_le_bytes());
        }
        ResumableScalar::U8(value) => hasher.update([2u8, *value]),
        ResumableScalar::Usize(value) => {
            hasher.update([3u8]);
            hasher.update(value.to_le_bytes());
        }
        ResumableScalar::Char(value) => {
            hasher.update([4u8]);
            hasher.update(value.to_le_bytes());
        }
        ResumableScalar::F32(bits) => {
            hasher.update([5u8]);
            hasher.update(bits.to_le_bytes());
        }
        ResumableScalar::F64(bits) => {
            hasher.update([6u8]);
            hasher.update(bits.to_le_bytes());
        }
        ResumableScalar::Bool(value) => hasher.update([7u8, u8::from(*value)]),
        ResumableScalar::Record(fields) => {
            hasher.update([8u8]);
            hasher.update((fields.len() as u64).to_le_bytes());
            for field in fields {
                hash_scalar(hasher, field);
            }
        }
        ResumableScalar::Variant { case, fields } => {
            hasher.update([9u8]);
            frame(hasher, case.as_bytes());
            hasher.update((fields.len() as u64).to_le_bytes());
            for field in fields {
                hash_scalar(hasher, field);
            }
        }
        ResumableScalar::Bytes(bytes) => {
            hasher.update([10u8]);
            frame(hasher, bytes);
        }
    }
}

fn reject_reachable_resumable_callees(
    program: &ResolvedProgram,
    entry: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![entry.id.clone()];
    while let Some(function_id) = pending.pop() {
        if !visited.insert(function_id.clone()) {
            continue;
        }
        let function = program
            .functions
            .iter()
            .find(|function| function.id == function_id)
            .ok_or_else(|| invalid(format!("reachable function `{function_id}` is absent")))?;
        for callee in called_functions(function)? {
            let resolved = program
                .functions
                .iter()
                .find(|candidate| candidate.id == callee)
                .ok_or_else(|| invalid(format!("reachable callee `{callee}` is absent")))?;
            if resolved.yields.is_some() {
                return Err(invalid(format!(
                    "resumable function `{}` reaches yielding callee `{callee}`; yield propagation is not admitted",
                    entry.id
                )));
            }
            pending.push(callee);
        }
    }
    Ok(())
}

fn called_functions(function: &ResolvedFunction) -> Result<Vec<DeclarationId>, Diagnostic> {
    let mut callees = Vec::new();
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Call {
            callee, instance, ..
        } = &expression.kind
        {
            if instance.is_some() {
                return Err(invalid(format!(
                    "resumable function reaches generic callee `{callee}`; generic call projection is not admitted"
                )));
            }
            // Issue #296, spec section 11.6: a builtin byte operation (e.g.
            // `bytes_copy`, `byte_len`) is a `Call` node to a compiler-owned
            // sentinel id the interpreter dispatches directly; it has no
            // `ResolvedFunction` of its own in `program.functions` and never
            // itself yields, so it is not a "reachable callee" this closure
            // check is about.
            if crate::byte_ops::by_id(callee.as_str()).is_none() {
                callees.push(callee.clone());
            }
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(callees)
}

fn state(
    function: &DeclarationId,
    kind: ResumableStateKind,
    expression: Option<&ExpressionId>,
) -> ResumableState {
    let function = function.as_str();
    let kind_text = match kind {
        ResumableStateKind::Entry => "entry",
        ResumableStateKind::Suspended => "suspended",
        ResumableStateKind::Complete => "complete",
    };
    let mut identity = format!(
        "semaprax.resumable-state.v1|{}:{}|{}",
        function.len(),
        function,
        kind_text
    );
    if let Some(expression) = expression {
        identity.push('|');
        identity.push_str(&expression.as_str().len().to_string());
        identity.push(':');
        identity.push_str(expression.as_str());
    }
    ResumableState {
        id: ResumableStateId(identity),
        kind,
    }
}

type YieldSite<'a> = (&'a ResolvedExpr, &'a ResolvedExpr, ResumableYieldPosition);

fn locate_direct_yields(function: &ResolvedFunction) -> Result<Vec<YieldSite<'_>>, Diagnostic> {
    let mut all_yields = Vec::new();
    let mut pending = vec![&function.body];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Yield { .. } = expression.kind {
            all_yields.push(expression);
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    if !(1..=MAX_RESUMABLE_YIELDS).contains(&all_yields.len()) {
        return Err(invalid(format!(
            "resumable lowering requires between 1 and {MAX_RESUMABLE_YIELDS} yields; found {}",
            all_yields.len()
        )));
    }

    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return Err(invalid(
            "resumable function body is not its expected top-level block",
        ));
    };
    let mut direct = Vec::new();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    for (index, statement) in statements.iter().enumerate() {
        let (value, kind) = match statement {
            ResolvedStatement::Let { value, .. } => (value, ResumableStatementKind::Let),
            ResolvedStatement::Assign { value, .. } => (value, ResumableStatementKind::Assign),
            ResolvedStatement::Unsafe { .. } | ResolvedStatement::While { .. } => continue,
        };
        if let ResolvedExprKind::Yield { request } = &value.kind {
            let index = u32::try_from(index)
                .map_err(|_| invalid("resumable statement index exceeds u32"))?;
            if value.id != ExpressionId::new(&execution, &format!("body.s{index}.value"))
                || request.id
                    != ExpressionId::new(&execution, &format!("body.s{index}.value.request"))
            {
                return Err(invalid("resumable yield has a non-canonical identity"));
            }
            direct.push((
                value,
                request.as_ref(),
                ResumableYieldPosition::Statement { index, kind },
            ));
        }
    }
    if let ResolvedExprKind::Yield { request } = &tail.kind {
        if tail.id != ExpressionId::new(&execution, "body.tail")
            || request.id != ExpressionId::new(&execution, "body.tail.request")
        {
            return Err(invalid("resumable yield has a non-canonical identity"));
        }
        direct.push((
            tail.as_ref(),
            request.as_ref(),
            ResumableYieldPosition::Tail,
        ));
    }
    if direct.len() != all_yields.len() {
        return Err(invalid(
            "resumable yield is nested instead of occupying a direct top-level slot",
        ));
    }
    Ok(direct)
}

fn reject_yield_in_contracts(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    for expression in function.requires.iter().chain(function.ensures.iter()) {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if matches!(expression.kind, ResolvedExprKind::Yield { .. }) {
                return Err(invalid(
                    "resumable lowering found `yield` in a contract expression",
                ));
            }
            hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
    Ok(())
}

/// `admitted_boundary_types`, when present, are the exact declared
/// request/response and whole-function boundary types of a bounded
/// record/variant `yields` channel
/// (`hir::yield_aggregate::bounded_aggregate_refusal` already checked its
/// shape): an intermediate value of *exactly* one of those two types is
/// admitted alongside the ordinary Copy-scalar profile (issue #296 R20). Only
/// `lower_sequential` passes one; `lowering::control::lower_control` passes
/// `None` and so keeps its pre-existing scalar-only re-check unchanged, since
/// the control-dependent lane has no aggregate-channel runtime support.
fn require_scalar_expression_tree(
    root: &ResolvedExpr,
    allow_owned_bytes: bool,
    admitted_boundary_types: Option<&[&ResolvedType]>,
) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        let is_admitted_channel_value = admitted_boundary_types
            .is_some_and(|types| types.iter().any(|ty| expression.ty == **ty));
        let admitted = expression.ty == ResolvedType::Unit
            || hir::is_scalar_resolved_type(&expression.ty)
            || (allow_owned_bytes && expression.ty == ResolvedType::Bytes)
            || is_admitted_channel_value;
        if !admitted {
            return Err(invalid(
                "resumable lowering found a non-scalar intermediate value",
            ));
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    Ok(())
}

/// Move one already-resolved expression to a new canonical path inside the
/// same function. Expression identities are structural, so cloning a request
/// from `yield.request` into the start projection's tail must rederive every
/// nested identity. Locals introduced inside the moved subtree are rederived
/// with the same scope rules; references to parameters and outer-prefix
/// locals deliberately retain their existing identities.
fn relocate_expression(
    expression: &mut ResolvedExpr,
    execution: &FunctionExecutionId,
    path: &str,
    values: &BTreeMap<ValueId, ValueId>,
) -> Result<(), Diagnostic> {
    expression.id = ExpressionId::new(execution, path);
    let expression_id = expression.id.clone();
    match &mut expression.kind {
        ResolvedExprKind::Closure { .. } => {
            return Err(invalid(
                "resumable projection cannot relocate a closure expression",
            ));
        }
        ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_) => {}
        ResolvedExprKind::Invoke { callable, args } => {
            relocate_expression(callable, execution, &format!("{path}.callable"), values)?;
            relocate_arguments(args, execution, path, "arg", values)?;
        }
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
            remap_place(place, values);
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            relocate_expression(source, execution, &format!("{path}.arg.0"), values)?;
            relocate_expression(start, execution, &format!("{path}.arg.1"), values)?;
            relocate_expression(end, execution, &format!("{path}.arg.2"), values)?;
        }
        ResolvedExprKind::Call { args, .. } | ResolvedExprKind::LiteralFormat { args, .. } => {
            relocate_arguments(args, execution, path, "arg", values)?;
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.expression = expression_id;
            relocate_arguments(&mut call.args, execution, path, "native-rust-arg", values)?;
        }
        ResolvedExprKind::HostCommandCall(call) => {
            call.expression = expression_id;
            relocate_arguments(&mut call.args, execution, path, "arg", values)?;
        }
        ResolvedExprKind::Unary { value, .. } => {
            relocate_expression(value, execution, &format!("{path}.value"), values)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            relocate_expression(left, execution, &format!("{path}.left"), values)?;
            relocate_expression(right, execution, &format!("{path}.right"), values)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            let mut block_values = values.clone();
            for (index, statement) in statements.iter_mut().enumerate() {
                let statement_path = format!("{path}.s{index}");
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        relocate_expression(
                            value,
                            execution,
                            &format!("{statement_path}.value"),
                            &block_values,
                        )?;
                        let previous = binding.id.clone();
                        binding.id = ValueId::local(execution, &statement_path);
                        block_values.insert(previous, binding.id.clone());
                    }
                    ResolvedStatement::Assign { binding, value, .. } => {
                        remap_binding(binding, &block_values);
                        relocate_expression(
                            value,
                            execution,
                            &format!("{statement_path}.value"),
                            &block_values,
                        )?;
                    }
                    ResolvedStatement::Unsafe { body, .. } => relocate_expression(
                        body,
                        execution,
                        &format!("{statement_path}.body"),
                        &block_values,
                    )?,
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        relocate_expression(
                            condition,
                            execution,
                            &format!("{statement_path}.condition"),
                            &block_values,
                        )?;
                        relocate_expression(
                            body,
                            execution,
                            &format!("{statement_path}.body"),
                            &block_values,
                        )?;
                    }
                }
            }
            relocate_expression(tail, execution, &format!("{path}.tail"), &block_values)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            relocate_expression(condition, execution, &format!("{path}.condition"), values)?;
            relocate_expression(then_branch, execution, &format!("{path}.then"), values)?;
            relocate_expression(else_branch, execution, &format!("{path}.else"), values)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            relocate_fields(fields, execution, path, values)?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            relocate_expression(scrutinee, execution, &format!("{path}.scrutinee"), values)?;
            for (index, arm) in arms.iter_mut().enumerate() {
                let arm_path = format!("{path}.arm.{index}");
                let mut arm_values = values.clone();
                relocate_pattern(&mut arm.pattern, execution, &arm_path, &mut arm_values);
                if let Some(guard) = &mut arm.guard {
                    relocate_expression(
                        guard,
                        execution,
                        &format!("{arm_path}.guard"),
                        &arm_values,
                    )?;
                }
                relocate_expression(
                    &mut arm.value,
                    execution,
                    &format!("{arm_path}.value"),
                    &arm_values,
                )?;
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            relocate_expression(operand, execution, &format!("{path}.operand"), values)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            relocate_expression(base, execution, &format!("{path}.base"), values)?;
            relocate_fields(fields, execution, path, values)?;
        }
        ResolvedExprKind::Project { base, .. } => {
            relocate_expression(base, execution, &format!("{path}.base"), values)?;
        }
        ResolvedExprKind::Upcast { source } => {
            relocate_expression(source, execution, &format!("{path}.source"), values)?;
        }
        ResolvedExprKind::Yield { .. } => {
            return Err(invalid(
                "resumable request relocation found a nested yield expression",
            ));
        }
    }
    Ok(())
}

fn relocate_arguments(
    arguments: &mut [ResolvedExpr],
    execution: &FunctionExecutionId,
    path: &str,
    segment: &str,
    values: &BTreeMap<ValueId, ValueId>,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter_mut().enumerate() {
        relocate_expression(
            argument,
            execution,
            &format!("{path}.{segment}.{index}"),
            values,
        )?;
    }
    Ok(())
}

fn relocate_fields(
    fields: &mut [hir::ResolvedFieldInitializer],
    execution: &FunctionExecutionId,
    path: &str,
    values: &BTreeMap<ValueId, ValueId>,
) -> Result<(), Diagnostic> {
    for (index, field) in fields.iter_mut().enumerate() {
        relocate_expression(
            &mut field.value,
            execution,
            &format!("{path}.field.{index}.value"),
            values,
        )?;
    }
    Ok(())
}

fn remap_place(place: &mut Place, values: &BTreeMap<ValueId, ValueId>) {
    if let Some(mapped) = values.get(&place.root) {
        place.root = mapped.clone();
    }
}

fn remap_binding(binding: &mut hir::ResolvedBinding, values: &BTreeMap<ValueId, ValueId>) {
    if let Some(mapped) = values.get(&binding.id) {
        binding.id = mapped.clone();
    }
}

fn relocate_pattern(
    pattern: &mut hir::ResolvedMatchPattern,
    execution: &FunctionExecutionId,
    path: &str,
    values: &mut BTreeMap<ValueId, ValueId>,
) {
    match pattern {
        hir::ResolvedMatchPattern::Variant { fields, .. } => {
            for (index, field) in fields.iter_mut().enumerate() {
                let previous = field.binding.id.clone();
                field.binding.id = ValueId::local(execution, &format!("{path}.binding.{index}"));
                values.insert(previous, field.binding.id.clone());
            }
        }
        hir::ResolvedMatchPattern::Record { fields, .. } => {
            relocate_record_pattern_fields(fields, execution, &format!("{path}.record"), values);
        }
        hir::ResolvedMatchPattern::Binding(binding) => {
            let previous = binding.id.clone();
            binding.id = ValueId::local(execution, &format!("{path}.binding"));
            values.insert(previous, binding.id.clone());
        }
        hir::ResolvedMatchPattern::Wildcard
        | hir::ResolvedMatchPattern::Literal(_)
        | hir::ResolvedMatchPattern::Or(_) => {}
    }
}

fn relocate_record_pattern_fields(
    fields: &mut [hir::ResolvedRecordMatchPatternField],
    execution: &FunctionExecutionId,
    path: &str,
    values: &mut BTreeMap<ValueId, ValueId>,
) {
    for (index, field) in fields.iter_mut().enumerate() {
        let field_path = format!("{path}.field.{index}");
        match &mut field.pattern {
            hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                let previous = binding.id.clone();
                binding.id = ValueId::local(execution, &format!("{field_path}.binding"));
                values.insert(previous, binding.id.clone());
            }
            hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                relocate_record_pattern_fields(
                    fields,
                    execution,
                    &format!("{field_path}.record"),
                    values,
                );
            }
            hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        INVALID_RESUMABLE_PLAN,
        format!("invalid resumable HIR plan: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SOURCE: &str = r#"
module test.resumable_lowering;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let before = seed + 1;
    let answer = yield before;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program(source: &str) -> ResolvedProgram {
        let ast = crate::parse(source, Path::new("resumable-lowering.spx")).unwrap();
        hir::resolve(&ast).unwrap()
    }

    fn plan(program: &ResolvedProgram) -> ResumablePlan {
        plan_for(program, "app.ask")
    }

    fn plan_for(program: &ResolvedProgram, id: &str) -> ResumablePlan {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        lower(program, function).unwrap()
    }

    #[test]
    fn lowering_is_deterministic_and_both_projections_are_yield_free() {
        let program = program(SOURCE);
        let first = plan(&program);
        let second = plan(&program);
        assert_eq!(first, second);
        assert_eq!(first.entry.kind, ResumableStateKind::Entry);
        assert_eq!(first.suspension.state.kind, ResumableStateKind::Suspended);
        assert_eq!(first.complete.kind, ResumableStateKind::Complete);
        assert!(first
            .entry
            .id
            .as_str()
            .starts_with("semaprax.resumable-state.v1|"));
        assert_ne!(first.entry.id, first.suspension.state.id);
        assert_ne!(first.suspension.state.id, first.complete.id);
        assert!(matches!(
            first.suspension.position,
            ResumableYieldPosition::Statement {
                index: 1,
                kind: ResumableStatementKind::Let
            }
        ));

        let start = first.start_program(&program).unwrap();
        let resume = first.resume_program(&program).unwrap();
        for projected in [&start, &resume] {
            let function = projected
                .functions
                .iter()
                .find(|function| function.id.as_str() == "app.ask")
                .unwrap();
            assert!(function.yields.is_none());
            let mut pending = vec![&function.body];
            while let Some(expression) = pending.pop() {
                assert!(!matches!(expression.kind, ResolvedExprKind::Yield { .. }));
                hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
            }
        }
        let start = start
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        assert_eq!(start.params.len(), 1);
        assert_eq!(start.return_type, ResolvedType::I64);
        assert!(start.ensures.is_empty());
        let resume = resume
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        assert_eq!(resume.params.len(), 2);
        assert_eq!(resume.params[1].name, "__semaprax_resumable_answer");
        assert!(resume.requires.is_empty());
    }

    #[test]
    fn request_relocation_rederives_nested_block_branch_and_local_identities() {
        let source = r#"
module test.resumable_nested_request;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield {
        let local = seed + 1;
        if local > 1 { local } else { seed }
    };
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = program(source);
        let plan = plan(&program);
        let start = plan.start_program(&program).unwrap();
        let resume = plan.resume_program(&program).unwrap();
        hir::validate(&start).unwrap();
        hir::validate(&resume).unwrap();
    }

    #[test]
    fn projection_validation_and_plan_authentication_reject_identity_or_meaning_mutation() {
        let program = program(SOURCE);
        let plan = plan(&program);

        let mut invalid_identity = plan.start_program(&program).unwrap();
        let function = invalid_identity
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("projection body is a block")
        };
        tail.id = plan.suspension.request_expression.clone();
        let error = hir::validate(&invalid_identity).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains("non-canonical identity"));

        let mut forged_plan = plan.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut forged_plan.start.function.body.kind else {
            panic!("projection body is a block")
        };
        tail.kind = ResolvedExprKind::Int(99);
        let error = forged_plan.start_program(&program).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains("exact deterministic lowering"));
    }

    #[test]
    fn display_renames_preserve_the_compiler_owned_state_identities() {
        let renamed = SOURCE
            .replace("fn ask(seed: i64)", "fn question(input: i64)")
            .replace("let before = seed + 1", "let request = input + 1")
            .replace("let answer = yield before", "let reply = yield request")
            .replace("answer * 2", "reply * 2");
        let before = plan(&program(SOURCE));
        let after = plan(&program(&renamed));
        assert_eq!(before.entry.id, after.entry.id);
        assert_eq!(before.suspension.state.id, after.suspension.state.id);
        assert_eq!(before.complete.id, after.complete.id);
    }

    #[test]
    fn a_forged_duplicate_yield_identity_is_rejected_by_lowering_itself() {
        let mut program = program(SOURCE);
        {
            let function = program
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "app.ask")
                .unwrap();
            let ResolvedExprKind::Block { statements, tail } = &mut function.body.kind else {
                panic!("fixture body is a block")
            };
            let ResolvedStatement::Let { value, .. } = &statements[1] else {
                panic!("fixture yield is a let value")
            };
            *tail = Box::new(value.clone());
        }
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let error = lower(&program, function).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains("non-canonical identity"));
    }

    #[test]
    fn length_framed_state_ids_do_not_alias_punctuation_in_identifiers() {
        let expression = ExpressionId::new(
            &FunctionExecutionId::Monomorphic(DeclarationId::new("b")),
            "yield",
        );
        let left = state(
            &DeclarationId::new("a|1:b"),
            ResumableStateKind::Suspended,
            Some(&expression),
        );
        let right = state(
            &DeclarationId::new("a"),
            ResumableStateKind::Suspended,
            Some(&expression),
        );
        assert_ne!(left.id, right.id);
    }

    #[test]
    fn reachable_yielding_callees_are_rejected_before_projection() {
        let source = r#"
module test.resumable_yielding_callee;
@id("app.inner")
fn inner(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed;
    answer
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let from_inner = inner(seed);
    let answer = yield from_inner;
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = program(source);
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.ask")
            .unwrap();
        let error = lower(&program, function).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains("yielding callee `app.inner`"));
    }

    #[test]
    fn disconnected_yielding_functions_have_independent_closed_projections() {
        let source = r#"
module test.resumable_disconnected;
@id("app.other")
fn other(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed;
    answer
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.dropped_bytes")
fn dropped_bytes(input: borrow Slice<u8>) -> usize { byte_len(input) }
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let program = program(source);
        assert!(program
            .declarations
            .byte_slice_provenances()
            .any(|(value, _)| value.as_str().contains("app.dropped_bytes")));
        let other = plan_for(&program, "app.other");
        let ask = plan_for(&program, "app.ask");

        for (plan, retained, removed) in [
            (&other, "app.other", "app.ask"),
            (&ask, "app.ask", "app.other"),
        ] {
            let start = plan.start_program(&program).unwrap();
            let resume = plan.resume_program(&program).unwrap();
            assert_eq!(start, plan.start_program(&program).unwrap());
            for projected in [&start, &resume] {
                hir::validate(projected).unwrap();
                assert_eq!(
                    projected
                        .functions
                        .iter()
                        .map(|function| function.id.as_str())
                        .collect::<Vec<_>>(),
                    vec![retained, "app.main"]
                );
                assert!(projected
                    .declarations
                    .declaration(&DeclarationId::new(removed))
                    .is_none());
                let removed_name = &program
                    .declarations
                    .declaration(&DeclarationId::new(removed))
                    .unwrap()
                    .name;
                assert!(projected.declarations.function_id(removed_name).is_none());
                assert!(projected
                    .declarations
                    .declaration(&DeclarationId::new("app.dropped_bytes"))
                    .is_none());
                assert!(projected
                    .declarations
                    .function_id("dropped_bytes")
                    .is_none());
                assert_eq!(projected.declarations.byte_slice_provenances().count(), 0);
                for function in &projected.functions {
                    assert_eq!(
                        projected.declarations.declaration(&function.id),
                        program.declarations.declaration(&function.id)
                    );
                    assert_eq!(
                        projected.declarations.function_id(&function.name),
                        Some(&function.id)
                    );
                }
            }
        }

        let mut hostile = ask.start_program(&program).unwrap();
        hostile.functions.push(
            program
                .functions
                .iter()
                .find(|function| function.id.as_str() == "app.other")
                .unwrap()
                .clone(),
        );
        let error = hir::validate(&hostile).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains("absent from the declaration index"));
    }

    #[test]
    fn projection_retains_both_root_closures_in_authored_order() {
        let source = r#"
module test.resumable_projection_order;
@id("app.entry_helper")
fn entry_helper() -> i64 { 0 }
@id("app.other")
fn other(seed: i64) -> i64 yields i64 -> i64 {
    let answer = yield seed;
    answer
}
@id("app.selected_helper")
fn selected_helper(seed: i64) -> i64 { seed + 1 }
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let answer = yield selected_helper(seed);
    answer
}
@id("app.main")
fn main() -> i64 { entry_helper() }
"#;
        let program = program(source);
        let plan = plan(&program);
        for projected in [
            plan.start_program(&program).unwrap(),
            plan.resume_program(&program).unwrap(),
        ] {
            assert_eq!(
                projected
                    .functions
                    .iter()
                    .map(|function| function.id.as_str())
                    .collect::<Vec<_>>(),
                vec![
                    "app.entry_helper",
                    "app.selected_helper",
                    "app.ask",
                    "app.main",
                ]
            );
            hir::validate(&projected).unwrap();
        }
    }

    #[test]
    fn projection_rejects_an_incoming_caller_before_signature_replacement() {
        let source = SOURCE.replace("fn main() -> i64 { 0 }", "fn main() -> i64 { ask(0) }");
        let program = program(&source);
        let plan = plan(&program);
        let error = plan.start_program(&program).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained incoming caller `app.main`"));
    }

    #[test]
    fn a_yielding_function_in_the_retained_entrypoint_closure_is_rejected() {
        let source = r#"
module test.resumable_entrypoint_yield;
@id("app.ask")
fn ask(seed: i64) -> i64 yields i64 -> i64 {
    let answer = yield seed;
    answer
}
@id("app.other")
fn other(seed: i64) -> i64 yields i64 -> i64 {
    let answer = yield seed + 1;
    answer
}
@id("app.main")
fn main() -> i64 { other(0) }
"#;
        let program = program(source);
        let plan = plan(&program);
        let error = plan.start_program(&program).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained projection closure reaches yielding function `app.other`"));
    }

    #[test]
    fn retained_generic_calls_and_function_references_fail_closed() {
        let generic = SOURCE.replace(
            "@id(\"app.main\")\nfn main() -> i64 { 0 }",
            r#"@id("app.identity")
fn identity<T>(value: T) -> T { value }
@id("app.main")
fn main() -> i64 { identity<i64>(0) }"#,
        );
        let generic_program = program(&generic);
        let error = plan(&generic_program)
            .start_program(&generic_program)
            .unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained projection closure reaches generic call `app.identity`"));

        let reference = SOURCE.replace(
            "@id(\"app.main\")\nfn main() -> i64 { 0 }",
            r#"@id("app.identity")
fn identity(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { let callback = identity; callback(0) }"#,
        );
        let reference_program = program(&reference);
        let error = plan(&reference_program)
            .start_program(&reference_program)
            .unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained projection closure reaches function reference `app.identity`"));
    }

    #[test]
    fn nominal_and_authority_surfaces_fail_before_partial_index_pruning() {
        let class = SOURCE.replace(
            "@id(\"app.main\")\nfn main() -> i64 { 0 }",
            r#"@id("data.counter")
class Counter {
    @id("data.counter.value")
    value: i64,
    @id("data.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter {
        Counter { value: self.value + amount }
    }
}
@id("app.main")
fn main() -> i64 { 0 }"#,
        );
        let class_program = program(&class);
        let error = plan(&class_program)
            .start_program(&class_program)
            .unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("does not admit authored nominal or resource declarations"));

        let authority = SOURCE.replace(
            "module test.resumable_lowering;",
            r#"module test.resumable_lowering;
permit { host.echo }
@id("host")
interface Host permits { host.echo } {
    @id("host.echo")
    import rust fn echo(value: i64) -> i64
        effects { host.echo } failure status "host.echo.v1";
}"#,
        );
        let authority_program = program(&authority);
        let error = plan(&authority_program)
            .start_program(&authority_program)
            .unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained scalar projection does not admit module permits"));

        let interface = SOURCE.replace(
            "module test.resumable_lowering;",
            r#"module test.resumable_lowering;
@id("host")
interface Host permits {} {
    @id("host.echo")
    import rust fn echo(value: i64) -> unit effects {} failure infallible;
}"#,
        );
        let interface_program = program(&interface);
        let error = plan(&interface_program)
            .start_program(&interface_program)
            .unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error
            .message
            .contains("retained scalar projection does not admit interfaces or imports"));
    }

    #[test]
    fn retained_entrypoint_must_stay_in_the_copy_scalar_profile() {
        let source = SOURCE.replace(
            "fn main() -> i64 { 0 }",
            "fn main() -> i64 { let message = \"not scalar\"; 0 }",
        );
        let program = program(&source);
        let error = plan(&program).start_program(&program).unwrap_err();
        assert_eq!(error.code, INVALID_RESUMABLE_PLAN);
        assert!(error.message.contains(
            "retained scalar function `app.main` contains an expression outside the value Copy-scalar profile"
        ));
    }

    #[test]
    fn suspension_binding_commits_program_meaning_site_and_exact_argument_bits() {
        let first_program = program(SOURCE);
        let first = plan(&first_program);
        let changed_program = program(&SOURCE.replace("answer * 2", "answer * 3"));
        let changed = plan(&changed_program);
        assert_eq!(first.suspension.state.id, changed.suspension.state.id);
        assert_ne!(first.identity, changed.identity);

        let one = first.suspension_binding(&[ResumableScalar::I64(1)]);
        let two = first.suspension_binding(&[ResumableScalar::I64(2)]);
        let other_program = changed.suspension_binding(&[ResumableScalar::I64(1)]);
        assert_ne!(one, two);
        assert_ne!(one, other_program);
        let error = first.start_program(&changed_program).unwrap_err();
        assert!(error.message.contains("checked-program identity"));
        assert_ne!(
            first.suspension_binding(&[ResumableScalar::F64((-0.0f64).to_bits())]),
            first.suspension_binding(&[ResumableScalar::F64(0.0f64.to_bits())])
        );
    }
}
