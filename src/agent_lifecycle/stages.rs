//! Stage binding: the four deterministic operation identities of one
//! AgentDefinition resolved to actual verified functions in one checked
//! module, validated against the same HIR ordinary execution uses.
//!
//! Nothing here executes. Binding resolves identities, validates signatures,
//! ownership modes, declared effects and the derived stage graph, and retains
//! one authority-free [`PreparedRetainedCall`] per stage. Every rejection is
//! an `SPX-G570` diagnostic naming the exact failing field, and every one of
//! them happens before a lifecycle can run, so no host work is reachable from
//! an unbound or incorrectly bound stage.

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedFunction, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};
use crate::interpreter::retained_call::{prepare_retained_call, PreparedRetainedCall};

/// The four deterministic stage roles this slice compiles and executes, in
/// their normative order.
pub(super) const DETERMINISTIC_ROLES: [&str; 4] = ["initialize", "observe", "authorize", "reduce"];

/// Every stage role of the semantic object, including the two this slice does
/// not compile: `propose` is model-bound and `execute` is the injected read
/// operation.
pub(super) const ALL_ROLES: [&str; 6] = [
    "initialize",
    "observe",
    "propose",
    "authorize",
    "execute",
    "reduce",
];

/// The six type roles in normative order.
pub(super) const TYPE_ROLES: [&str; 6] = [
    "task",
    "state",
    "observation",
    "proposal",
    "outcome",
    "result",
];

pub(super) fn invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G570",
        format!("AgentLifecycle stage binding invariant failed: {field}"),
    )
}

/// One admitted exact scalar the proposal projection can transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScalarKind {
    Bool,
    I32,
    I64,
    U8,
    Usize,
}

impl ScalarKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::Usize => "usize",
        }
    }

    fn of(ty: &ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::Bool => Some(Self::Bool),
            ResolvedType::I32 => Some(Self::I32),
            ResolvedType::I64 => Some(Self::I64),
            ResolvedType::U8 => Some(Self::U8),
            ResolvedType::Usize => Some(Self::Usize),
            _ => None,
        }
    }
}

/// One field of the Proposal role type, projected into the stage parameter
/// list that carries it across the retained seam.
///
/// Proposal Schema v1 admits only records and variants, while a by-value
/// nominal stage parameter must be a Copy `class`. A Proposal carrier
/// therefore cannot cross the retained seam as a nominal value in this slice,
/// so it crosses as its exact ordered scalar projection instead, keyed by the
/// proposal type's own persistent field identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProposalParameter {
    pub(super) field: DeclarationId,
    pub(super) kind: ScalarKind,
}

/// One bound deterministic stage.
pub(super) struct BoundStage {
    role: &'static str,
    operation_id: String,
    function_id: String,
    parameters: Vec<(String, String)>,
    result: String,
    prepared: PreparedRetainedCall,
}

impl BoundStage {
    pub(super) fn role(&self) -> &'static str {
        self.role
    }

    pub(super) fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub(super) fn function_id(&self) -> &str {
        &self.function_id
    }

    /// The ordered `(ownership, type identity)` rows this stage was admitted
    /// against, for the canonical lifecycle document.
    pub(super) fn parameters(&self) -> &[(String, String)] {
        &self.parameters
    }

    pub(super) fn result(&self) -> &str {
        &self.result
    }

    pub(super) fn prepared(&self) -> &PreparedRetainedCall {
        &self.prepared
    }
}

/// The authorizing stage, and the derived decision identities the authorizing
/// transition is admitted against.
///
/// This type has no public constructor and no `pub(crate)` one: only [`bind`]
/// builds it, and only after the authorize role has passed every signature,
/// ownership, effect and decision-shape check. The authorization module
/// requires a reference to one before it will mint an authorization value, so
/// an unvalidated function can never reach the mint.
pub(super) struct AuthorizeStage {
    stage: BoundStage,
    decision_type: DeclarationId,
    grant_case: DeclarationId,
    grant_seal_field: DeclarationId,
    grant_budget_field: DeclarationId,
    refuse_case: DeclarationId,
    refuse_code_field: DeclarationId,
}

impl AuthorizeStage {
    pub(super) fn stage(&self) -> &BoundStage {
        &self.stage
    }

    pub(super) fn decision_type(&self) -> &DeclarationId {
        &self.decision_type
    }

    pub(super) fn grant_case(&self) -> &DeclarationId {
        &self.grant_case
    }

    pub(super) fn grant_seal_field(&self) -> &DeclarationId {
        &self.grant_seal_field
    }

    pub(super) fn grant_budget_field(&self) -> &DeclarationId {
        &self.grant_budget_field
    }

    pub(super) fn refuse_case(&self) -> &DeclarationId {
        &self.refuse_case
    }

    pub(super) fn refuse_code_field(&self) -> &DeclarationId {
        &self.refuse_code_field
    }
}

/// A record carrier of exactly one owned `Bytes` leaf and exactly one `i64`
/// leaf: the admitted shape of the Task and Outcome role types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PayloadShape {
    pub(super) record: DeclarationId,
    pub(super) bytes_field: DeclarationId,
    pub(super) scalar_field: DeclarationId,
}

/// The complete validated binding of one lifecycle.
pub(super) struct StageBinding {
    pub(super) types: Vec<(&'static str, DeclarationId)>,
    pub(super) task: PayloadShape,
    pub(super) outcome: PayloadShape,
    pub(super) proposal: Vec<ProposalParameter>,
    pub(super) initialize: BoundStage,
    pub(super) observe: BoundStage,
    pub(super) authorize: AuthorizeStage,
    pub(super) reduce: BoundStage,
    pub(super) order: Vec<&'static str>,
}

impl StageBinding {
    pub(super) fn type_id(&self, role: &str) -> &DeclarationId {
        &self
            .types
            .iter()
            .find(|(name, _)| *name == role)
            .expect("every type role is bound")
            .1
    }
}

/// One directed stage-graph edge, derived from the validated signatures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StageEdge {
    pub(super) from: &'static str,
    pub(super) to: &'static str,
}

/// The unique deterministic execution order of an acyclic stage graph.
///
/// Returns `None` for a cycle and for any graph whose order is not unique, so
/// an ambiguous lifecycle is rejected rather than silently sequenced.
pub(super) fn topological_order(
    nodes: &[&'static str],
    edges: &[StageEdge],
) -> Option<Vec<&'static str>> {
    let mut order = Vec::with_capacity(nodes.len());
    let mut remaining: Vec<&'static str> = nodes.to_vec();
    while !remaining.is_empty() {
        let mut ready = remaining.iter().copied().filter(|node| {
            !edges
                .iter()
                .any(|edge| edge.to == *node && remaining.contains(&edge.from))
        });
        let next = ready.next()?;
        if ready.next().is_some() {
            // Two independent stages would make the order ambiguous.
            return None;
        }
        remaining.retain(|node| *node != next);
        order.push(next);
    }
    Some(order)
}

/// The fixed stage-graph edges of the semantic object.
pub(super) fn stage_edges() -> Vec<StageEdge> {
    vec![
        StageEdge {
            from: "initialize",
            to: "observe",
        },
        StageEdge {
            from: "observe",
            to: "propose",
        },
        StageEdge {
            from: "propose",
            to: "authorize",
        },
        StageEdge {
            from: "initialize",
            to: "authorize",
        },
        StageEdge {
            from: "authorize",
            to: "execute",
        },
        StageEdge {
            from: "execute",
            to: "reduce",
        },
        StageEdge {
            from: "initialize",
            to: "reduce",
        },
        StageEdge {
            from: "propose",
            to: "reduce",
        },
    ]
}

fn declaration<'a>(
    program: &'a hir::ResolvedProgram,
    id: &str,
    field: &str,
) -> Result<&'a ResolvedTypeDeclaration, Diagnostic> {
    let declaration = program
        .types
        .iter()
        .find(|item| item.id.as_str() == id)
        .ok_or_else(|| invariant(&format!("{field}.unresolved")))?;
    if !declaration.type_parameters.is_empty() {
        return Err(invariant(&format!("{field}.generic")));
    }
    persistent(program, id, field)?;
    Ok(declaration)
}

fn persistent(program: &hir::ResolvedProgram, id: &str, field: &str) -> Result<(), Diagnostic> {
    let item = program
        .declarations
        .declaration(&DeclarationId::new(id))
        .ok_or_else(|| invariant(&format!("{field}.unresolved")))?;
    if !item.identity_origin.is_persistent() {
        return Err(invariant(&format!("{field}.identity_origin")));
    }
    Ok(())
}

fn nominal(id: &DeclarationId) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: id.clone(),
        arguments: Vec::new(),
    }
}

fn function<'a>(
    program: &'a hir::ResolvedProgram,
    id: &str,
    field: &str,
) -> Result<&'a ResolvedFunction, Diagnostic> {
    let function = program
        .functions
        .iter()
        .find(|item| item.id.as_str() == id)
        .ok_or_else(|| invariant(&format!("{field}.unresolved")))?;
    persistent(program, id, field)?;
    if !function.effects.is_empty() {
        // A deterministic stage declares no effect. An effect on one is an
        // undeclared authority for this lifecycle, not a widening.
        return Err(invariant(&format!("{field}.effects")));
    }
    Ok(function)
}

/// Requires one parameter to carry exactly the declared ownership mode and
/// type. Ownership is checked before the type so a `borrow` where the graph
/// says `consumes` names `ownership` rather than a type mismatch.
fn parameter(
    function: &ResolvedFunction,
    index: usize,
    ownership: OwnershipMode,
    ty: &ResolvedType,
    field: &str,
) -> Result<(), Diagnostic> {
    let parameter = function
        .params
        .get(index)
        .ok_or_else(|| invariant(&format!("{field}.arity")))?;
    if parameter.ownership != ownership {
        return Err(invariant(&format!("{field}.ownership")));
    }
    if parameter.ty != *ty {
        return Err(invariant(&format!("{field}.type")));
    }
    Ok(())
}

fn proposal_parameters(
    function: &ResolvedFunction,
    offset: usize,
    proposal: &[ProposalParameter],
    field: &str,
) -> Result<(), Diagnostic> {
    for (index, projected) in proposal.iter().enumerate() {
        let parameter = function
            .params
            .get(offset + index)
            .ok_or_else(|| invariant(&format!("{field}.arity")))?;
        if parameter.ownership != OwnershipMode::Value {
            return Err(invariant(&format!("{field}.proposal.ownership")));
        }
        if ScalarKind::of(&parameter.ty) != Some(projected.kind) {
            return Err(invariant(&format!("{field}.proposal.type")));
        }
    }
    Ok(())
}

fn result(function: &ResolvedFunction, ty: &ResolvedType, field: &str) -> Result<(), Diagnostic> {
    if function.return_type != *ty {
        return Err(invariant(&format!("{field}.result")));
    }
    Ok(())
}

/// The canonical document spelling of one admitted stage type.
///
/// A nominal carrier is named by its persistent declaration identity, never by
/// its display name, so a type rename does not move the lifecycle digest while
/// an identity change does.
fn type_key(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Bool => "bool".to_owned(),
        ResolvedType::I32 => "i32".to_owned(),
        ResolvedType::I64 => "i64".to_owned(),
        ResolvedType::U8 => "u8".to_owned(),
        ResolvedType::Usize => "usize".to_owned(),
        ResolvedType::Bytes => "bytes".to_owned(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => declaration.as_str().to_owned(),
        other => other.identity_key(),
    }
}

fn rows(function: &ResolvedFunction) -> Vec<(String, String)> {
    function
        .params
        .iter()
        .map(|parameter| {
            let ownership = match parameter.ownership {
                OwnershipMode::Value => "value",
                OwnershipMode::Own => "consumes",
                OwnershipMode::Borrow => "borrows",
                OwnershipMode::Shared => "shared",
            };
            (ownership.to_owned(), type_key(&parameter.ty))
        })
        .collect()
}

fn retain(
    program: &hir::ResolvedProgram,
    function: &ResolvedFunction,
    role: &'static str,
    operation_id: &str,
    field: &str,
) -> Result<BoundStage, Vec<Diagnostic>> {
    let prepared = prepare_retained_call(program, function.id.as_str()).map_err(|mut errors| {
        errors.insert(0, invariant(&format!("{field}.retained_call")));
        errors
    })?;
    Ok(BoundStage {
        role,
        operation_id: operation_id.to_owned(),
        function_id: function.id.as_str().to_owned(),
        parameters: rows(function),
        result: type_key(&function.return_type),
        prepared,
    })
}

/// The admitted `{ Bytes, i64 }` record shape of the Task and Outcome roles.
fn payload_shape(
    declaration: &ResolvedTypeDeclaration,
    field: &str,
) -> Result<PayloadShape, Diagnostic> {
    let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Err(invariant(&format!("{field}.kind")));
    };
    if fields.len() != 2 {
        return Err(invariant(&format!("{field}.fields")));
    }
    let bytes = fields
        .iter()
        .find(|item| item.ty == ResolvedType::Bytes)
        .ok_or_else(|| invariant(&format!("{field}.bytes")))?;
    let scalar = fields
        .iter()
        .find(|item| item.ty == ResolvedType::I64)
        .ok_or_else(|| invariant(&format!("{field}.scalar")))?;
    Ok(PayloadShape {
        record: declaration.id.clone(),
        bytes_field: bytes.id.clone(),
        scalar_field: scalar.id.clone(),
    })
}

/// The ordered scalar projection of the Proposal role record.
fn proposal_projection(
    program: &hir::ResolvedProgram,
    proposal_type_id: &str,
) -> Result<Vec<ProposalParameter>, Diagnostic> {
    let declaration = declaration(program, proposal_type_id, "proposal_type")?;
    let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
        // A variant proposal has no single ordered scalar projection.
        return Err(invariant("proposal_type.kind"));
    };
    if fields.is_empty() {
        return Err(invariant("proposal_type.fields"));
    }
    let mut projection = Vec::with_capacity(fields.len());
    for item in fields {
        persistent(program, item.id.as_str(), "proposal_type.field")?;
        let kind = ScalarKind::of(&item.ty)
            .ok_or_else(|| invariant("proposal_type.field.representation"))?;
        projection.push(ProposalParameter {
            field: item.id.clone(),
            kind,
        });
    }
    Ok(projection)
}

/// The decision variant `authorize` returns.
///
/// It is derived from the validated authorize signature rather than authored:
/// exactly two cases, exactly one of which owns a `Bytes` seal, and that case
/// is the grant. The refusal carries one `i64` code and owns nothing.
fn decision(
    program: &hir::ResolvedProgram,
    function: &ResolvedFunction,
    role_types: &[(&'static str, DeclarationId)],
) -> Result<
    (
        DeclarationId,
        DeclarationId,
        DeclarationId,
        DeclarationId,
        DeclarationId,
        DeclarationId,
    ),
    Diagnostic,
> {
    let ResolvedType::Nominal {
        declaration: id,
        arguments,
    } = &function.return_type
    else {
        return Err(invariant("authorize.decision.kind"));
    };
    if !arguments.is_empty() {
        return Err(invariant("authorize.decision.generic"));
    }
    if role_types.iter().any(|(_, role_id)| role_id == id) {
        // The decision is a derived authorization type, never one of the six
        // authored role types.
        return Err(invariant("authorize.decision.role_collision"));
    }
    let declared = declaration(program, id.as_str(), "authorize.decision")?;
    let ResolvedTypeDeclarationKind::Variant { cases } = &declared.kind else {
        return Err(invariant("authorize.decision.kind"));
    };
    if cases.len() != 2 {
        return Err(invariant("authorize.decision.cases"));
    }
    let mut grant = None;
    let mut refuse = None;
    for case in cases {
        persistent(program, case.id.as_str(), "authorize.decision.case")?;
        if case
            .fields
            .iter()
            .any(|item| item.ty == ResolvedType::Bytes)
        {
            if grant.is_some() {
                return Err(invariant("authorize.decision.grant"));
            }
            grant = Some(case);
        } else {
            if refuse.is_some() {
                return Err(invariant("authorize.decision.refusal"));
            }
            refuse = Some(case);
        }
    }
    let grant = grant.ok_or_else(|| invariant("authorize.decision.grant"))?;
    let refuse = refuse.ok_or_else(|| invariant("authorize.decision.refusal"))?;
    if grant.fields.len() != 2 {
        return Err(invariant("authorize.decision.grant.fields"));
    }
    let seal = grant
        .fields
        .iter()
        .find(|item| item.ty == ResolvedType::Bytes)
        .ok_or_else(|| invariant("authorize.decision.grant.seal"))?;
    let budget = grant
        .fields
        .iter()
        .find(|item| item.ty == ResolvedType::I64)
        .ok_or_else(|| invariant("authorize.decision.grant.budget"))?;
    if refuse.fields.len() != 1 || refuse.fields[0].ty != ResolvedType::I64 {
        return Err(invariant("authorize.decision.refusal.code"));
    }
    Ok((
        id.clone(),
        grant.id.clone(),
        seal.id.clone(),
        budget.id.clone(),
        refuse.id.clone(),
        refuse.fields[0].id.clone(),
    ))
}

/// Binds one AgentDefinition's four deterministic operation identities to
/// verified functions in one resolved module.
pub(super) fn bind(
    program: &hir::ResolvedProgram,
    type_ids: &[(&'static str, String)],
    operation_ids: &[(&'static str, String)],
) -> Result<StageBinding, Vec<Diagnostic>> {
    bind_with_step_result(program, type_ids, operation_ids, None)
}

pub(super) fn bind_with_step_result(
    program: &hir::ResolvedProgram,
    type_ids: &[(&'static str, String)],
    operation_ids: &[(&'static str, String)],
    step: Option<&DeclarationId>,
) -> Result<StageBinding, Vec<Diagnostic>> {
    let mut types = Vec::with_capacity(TYPE_ROLES.len());
    for role in TYPE_ROLES {
        let id = type_ids
            .iter()
            .find(|(name, _)| *name == role)
            .map(|(_, id)| id.as_str())
            .ok_or_else(|| vec![invariant(&format!("{role}_type.unresolved"))])?;
        declaration(program, id, &format!("{role}_type")).map_err(|error| vec![error])?;
        types.push((role, DeclarationId::new(id)));
    }
    let task_declaration =
        declaration(program, types[0].1.as_str(), "task_type").map_err(|error| vec![error])?;
    let task = payload_shape(task_declaration, "task_type").map_err(|error| vec![error])?;
    let outcome_declaration =
        declaration(program, types[4].1.as_str(), "outcome_type").map_err(|error| vec![error])?;
    let outcome =
        payload_shape(outcome_declaration, "outcome_type").map_err(|error| vec![error])?;
    let proposal =
        proposal_projection(program, types[3].1.as_str()).map_err(|error| vec![error])?;

    let operation = |role: &str| -> Result<&str, Vec<Diagnostic>> {
        operation_ids
            .iter()
            .find(|(name, _)| *name == role)
            .map(|(_, id)| id.as_str())
            .ok_or_else(|| vec![invariant(&format!("{role}.unresolved"))])
    };

    let state = nominal(&types[1].1);
    let initialize_id = operation("initialize")?;
    let initialize_fn =
        function(program, initialize_id, "initialize").map_err(|error| vec![error])?;
    if initialize_fn.params.len() != 1 {
        return Err(vec![invariant("initialize.arity")]);
    }
    parameter(
        initialize_fn,
        0,
        OwnershipMode::Own,
        &nominal(&types[0].1),
        "initialize",
    )
    .map_err(|error| vec![error])?;
    result(initialize_fn, &state, "initialize").map_err(|error| vec![error])?;

    let observe_id = operation("observe")?;
    let observe_fn = function(program, observe_id, "observe").map_err(|error| vec![error])?;
    if observe_fn.params.len() != 1 {
        return Err(vec![invariant("observe.arity")]);
    }
    parameter(observe_fn, 0, OwnershipMode::Borrow, &state, "observe")
        .map_err(|error| vec![error])?;
    result(observe_fn, &nominal(&types[2].1), "observe").map_err(|error| vec![error])?;

    let authorize_id = operation("authorize")?;
    let authorize_fn = function(program, authorize_id, "authorize").map_err(|error| vec![error])?;
    if authorize_fn.params.len() != 1 + proposal.len() {
        return Err(vec![invariant("authorize.arity")]);
    }
    parameter(authorize_fn, 0, OwnershipMode::Borrow, &state, "authorize")
        .map_err(|error| vec![error])?;
    proposal_parameters(authorize_fn, 1, &proposal, "authorize").map_err(|error| vec![error])?;
    let (
        decision_type,
        grant_case,
        grant_seal_field,
        grant_budget_field,
        refuse_case,
        refuse_code_field,
    ) = decision(program, authorize_fn, &types).map_err(|error| vec![error])?;

    let reduce_id = operation("reduce")?;
    let reduce_fn = function(program, reduce_id, "reduce").map_err(|error| vec![error])?;
    if reduce_fn.params.len() != 2 + proposal.len() {
        return Err(vec![invariant("reduce.arity")]);
    }
    parameter(reduce_fn, 0, OwnershipMode::Own, &state, "reduce").map_err(|error| vec![error])?;
    proposal_parameters(reduce_fn, 1, &proposal, "reduce").map_err(|error| vec![error])?;
    parameter(
        reduce_fn,
        1 + proposal.len(),
        OwnershipMode::Own,
        &nominal(&types[4].1),
        "reduce.outcome",
    )
    .map_err(|error| vec![error])?;
    result(reduce_fn, &nominal(step.unwrap_or(&types[5].1)), "reduce")
        .map_err(|error| vec![error])?;

    let order = topological_order(&ALL_ROLES, &stage_edges())
        .ok_or_else(|| vec![invariant("stage_graph.acyclic")])?;
    if order != ALL_ROLES.to_vec() {
        return Err(vec![invariant("stage_graph.order")]);
    }

    Ok(StageBinding {
        types,
        task,
        outcome,
        proposal,
        initialize: retain(
            program,
            initialize_fn,
            "initialize",
            initialize_id,
            "initialize",
        )?,
        observe: retain(program, observe_fn, "observe", observe_id, "observe")?,
        authorize: AuthorizeStage {
            stage: retain(
                program,
                authorize_fn,
                "authorize",
                authorize_id,
                "authorize",
            )?,
            decision_type,
            grant_case,
            grant_seal_field,
            grant_budget_field,
            refuse_case,
            refuse_code_field,
        },
        reduce: retain(program, reduce_fn, "reduce", reduce_id, "reduce")?,
        order,
    })
}
