//! Rich Proposal stage binding: an additive convention where the checked
//! `authorize` and `reduce` stages accept the [Agent Interaction Schema v1]
//! (issue #109) Proposal as one nominal value, instead of the flat
//! per-field scalar projection [`super::stages::proposal_projection`] uses
//! for Agent Proposal Schema v1.
//!
//! This is genuinely additive, not a replacement: [`super::stages::bind`],
//! its `ProposalParameter`/`ScalarKind` vocabulary, [`super::iterative`]'s
//! whole driver loop and every lifecycle already compiled through them keep
//! their exact existing signatures, arity checks and known answers. A
//! module reaches this convention only by declaring `authorize`/`reduce`
//! functions that this binder's own signature rules admit; nothing here
//! reinterprets or widens the existing scalar-exploded convention.
//!
//! # What is admitted today, and what is not
//!
//! The retained call seam ([`crate::interpreter::retained_call`]) has
//! always carried recursive, nominally-identified `Record`/`Variant`
//! arguments — that is exactly how `initialize`'s Task, `observe`'s State
//! and `reduce`'s Outcome already cross today. This module gives Proposal
//! that same real interpreter argument slot for the first time, decoded
//! through the real #109 schema rather than the flat Agent Proposal Schema
//! v1 grammar, so `authorize`/`reduce` declare exactly one Proposal
//! parameter instead of one scalar parameter per field.
//!
//! **A genuinely nested Proposal (a record field whose type is itself a
//! further record) does not clear this seam yet**, and this module does not
//! resolve that: `interpreter::retained_call`'s
//! `resolved_data_parameter_is_admitted` admits a plain `record`-kind
//! `OwnershipMode::Value` parameter only when every leaf, at every depth, is
//! a direct scalar (`flat_copy_record`); the one route that does admit a
//! non-flat Value-mode nominal parameter requires `DeclarationKind::Class`,
//! and `agent_interaction_schema::shape::derive` refuses a `class` root
//! outright (`type.kind`) as one of its own stated exclusions. So a Proposal
//! that is both (a) a valid Agent Interaction Schema v1 root and (b) an
//! admitted retained-call Value parameter must, today, be flat: this
//! binder's own fixture (`tests.rs`) is a genuine two-field flat Proposal,
//! not a single re-scaffolded scalar, and its top comment records this
//! exact boundary — found empirically while building it, not asserted from
//! documentation. Widening either admission rule is out of this module's
//! file lease (`interpreter::retained_call` and
//! `agent_interaction_schema::shape` are both outside `src/agent_lifecycle/**`).
//!
//! # Composition, not a second execution model
//!
//! Decoding, admission and projection are the real, unmodified
//! [`crate::agent_interaction_schema`] decoder and
//! [`crate::agent_lifecycle_typed_carrier`] carrier (`binding::StageBinding`,
//! `projection::to_retained`) — both read-only dependencies of this module,
//! neither edited here. Dispatch is the real
//! [`crate::interpreter::retained_call::evaluate_retained_call`], the same
//! evaluator every other stage in this crate uses. This module supplies only
//! the missing glue: resolving exact persistent stage-function identities
//! for the new single-argument convention, validating their signatures and
//! decision/transition shapes from real HIR facts, and sequencing
//! decode -> admit -> project -> evaluate for one turn.
//!
//! # Authorization stays a separate opaque object
//!
//! A granted turn is represented by [`RichTurnOutcome::Continue`] /
//! [`RichTurnOutcome::Fail`]; a refused turn
//! ([`RichTurnOutcome::Refused`]) never reaches `reduce` — the decoded
//! Proposal is data admitted into `authorize`, never itself a grant, and
//! `reduce` is dispatched only after `authorize` has actually returned its
//! granting case.
//!
//! # Known limitations
//!
//! - The Proposal-nesting gap described above.
//! - This binder validates a fixed two-case shape for both `authorize`'s
//!   Decision and `reduce`'s Transition (`{ Grant | Refuse }` /
//!   `{ Continue | Fail }`) rather than the four-case
//!   `Continue/Complete/Suspend/Fail` `Step` grammar
//!   [`super::iterative::compile_agent_lifecycle_v2`] compiles, and admits
//!   only a single-scalar-field State (see [`RichProposalStages`]'s
//!   `state_field` documentation).
//! - No wiring into [`super::iterative::CompiledIterativeLifecycle`]'s
//!   existing multi-turn driver loop; [`run_rich_turn`] runs one turn,
//!   standalone. Widening to the full Step grammar and multi-turn driving
//!   is the next slice; see `docs/AGENT-LIFECYCLE-TYPED-CARRIER-V1.md`'s
//!   "Known limitations".

use std::path::Path;

use crate::agent_interaction_schema::{
    compile_agent_interaction_schema, CompiledInteractionSchema,
};
use crate::agent_lifecycle_typed_carrier::binding::{LifecycleStageRole, StageBinding};
use crate::agent_lifecycle_typed_carrier::projection::{to_retained, InteractionTypeGraph};
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedFunction, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use crate::interpreter::retained_call::{
    evaluate_retained_call, prepare_retained_call, PreparedRetainedCall, RetainedCallOutcome,
    RetainedField, RetainedRecord, RetainedValue,
};

fn invariant(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G588",
        format!("Rich proposal stage binding invariant failed: {field}"),
    )
}

fn persistent(program: &ResolvedProgram, id: &str, field: &str) -> Result<(), Diagnostic> {
    let item = program
        .declarations
        .declaration(&DeclarationId::new(id))
        .ok_or_else(|| invariant(&format!("{field}.unresolved")))?;
    if !item.identity_origin.is_persistent() {
        return Err(invariant(&format!("{field}.identity_origin")));
    }
    Ok(())
}

fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    }
}

fn find_type<'a>(
    program: &'a ResolvedProgram,
    id: &str,
    field: &str,
) -> Result<&'a hir::ResolvedTypeDeclaration, Diagnostic> {
    let declared = program
        .types
        .iter()
        .find(|item| item.id.as_str() == id)
        .ok_or_else(|| invariant(&format!("{field}.unresolved")))?;
    if !declared.type_parameters.is_empty() {
        return Err(invariant(&format!("{field}.generic")));
    }
    persistent(program, id, field)?;
    Ok(declared)
}

fn find_function<'a>(
    program: &'a ResolvedProgram,
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
        return Err(invariant(&format!("{field}.effects")));
    }
    Ok(function)
}

fn require_param(
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

/// The exact two-case transition shape both the authorize Decision and the
/// reduce transition are validated against: one case carrying exactly one
/// field of `positive_field_type` (Grant's flag, or Continue's next-state
/// leaf), and one case carrying exactly one field of `negative_field_type`
/// (Refuse's or Fail's code) — the two types must differ, since case
/// selection is disambiguated by field type. Resolved entirely from
/// persistent HIR facts: a display rename of the variant, either case, or
/// either field changes nothing this function reads.
///
/// Source-level "Copy Variants v1" (`SPX-T215`) admits only a direct Copy
/// scalar (or `Bytes`) as a variant case field, never a nested nominal
/// record — so neither case field here can itself be the State record; a
/// State-shaped positive field is one Copy scalar leaf that
/// [`RichTurnOutcome`]'s caller-visible reconstruction wraps back into a
/// real State record (see `run_rich_turn`'s single-field State handling).
struct TwoCaseShape {
    variant: DeclarationId,
    positive_case: DeclarationId,
    positive_field: DeclarationId,
    negative_case: DeclarationId,
    negative_field: DeclarationId,
}

fn two_case_shape(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    positive_field_type: &ResolvedType,
    negative_field_type: &ResolvedType,
    field: &str,
) -> Result<TwoCaseShape, Diagnostic> {
    if positive_field_type == negative_field_type {
        return Err(invariant(&format!("{field}.ambiguous_case_types")));
    }
    let ResolvedType::Nominal {
        declaration: id,
        arguments,
    } = &function.return_type
    else {
        return Err(invariant(&format!("{field}.kind")));
    };
    if !arguments.is_empty() {
        return Err(invariant(&format!("{field}.generic")));
    }
    let declared = find_type(program, id.as_str(), field)?;
    let ResolvedTypeDeclarationKind::Variant { cases } = &declared.kind else {
        return Err(invariant(&format!("{field}.kind")));
    };
    if cases.len() != 2 {
        return Err(invariant(&format!("{field}.cases")));
    }
    let mut positive = None;
    let mut negative = None;
    for case in cases {
        persistent(program, case.id.as_str(), field)?;
        if case.fields.is_empty() {
            return Err(invariant(&format!("{field}.case.fields")));
        }
        // Every case may also carry decorative `Bytes` fields (mirroring
        // `stages::decision`'s own `Grant { seal: Bytes, budget: i64 }`
        // shape): exactly one field of the disambiguating role type per
        // case is required, and every other field must be `Bytes`.
        let mut case_positive = None;
        let mut case_negative = None;
        for leaf in &case.fields {
            persistent(program, leaf.id.as_str(), field)?;
            if leaf.ty == *positive_field_type {
                if case_positive.is_some() {
                    return Err(invariant(&format!("{field}.case.positive.duplicate")));
                }
                case_positive = Some(leaf.id.clone());
            } else if leaf.ty == *negative_field_type {
                if case_negative.is_some() {
                    return Err(invariant(&format!("{field}.case.negative.duplicate")));
                }
                case_negative = Some(leaf.id.clone());
            } else if leaf.ty != ResolvedType::Bytes {
                return Err(invariant(&format!("{field}.case.type")));
            }
        }
        match (case_positive, case_negative) {
            (Some(leaf), None) => {
                if positive.is_some() {
                    return Err(invariant(&format!("{field}.positive.duplicate")));
                }
                positive = Some((case.id.clone(), leaf));
            }
            (None, Some(leaf)) => {
                if negative.is_some() {
                    return Err(invariant(&format!("{field}.negative.duplicate")));
                }
                negative = Some((case.id.clone(), leaf));
            }
            _ => return Err(invariant(&format!("{field}.case.role"))),
        }
    }
    let (positive_case, positive_field) =
        positive.ok_or_else(|| invariant(&format!("{field}.positive.missing")))?;
    let (negative_case, negative_field) =
        negative.ok_or_else(|| invariant(&format!("{field}.negative.missing")))?;
    Ok(TwoCaseShape {
        variant: id.clone(),
        positive_case,
        positive_field,
        negative_case,
        negative_field,
    })
}

/// One validated rich-Proposal `authorize`/`reduce` pair, bound once against
/// exact persistent identities resolved from the checked module and a real
/// derived [Agent Interaction Schema v1](../../docs/AGENT-INTERACTION-SCHEMA-V1.md)
/// schema.
///
/// `authorize` is admitted as `(state: State, proposal: Proposal) ->
/// Decision` where `Decision` is a two-case variant `{ grant: bool flag |
/// refuse: i64 code }`. `reduce` is admitted as `(state: State, proposal:
/// Proposal, outcome: own Bytes) -> Transition` where `Transition` is a
/// two-case variant `{ continue: <State's one scalar leaf> | fail: u8
/// code }` — `run_rich_turn` wraps the `continue` leaf back into a real
/// State record. Both stages declare no effect, matching every other
/// deterministic stage in this crate. State and Proposal parameters carry
/// no `own`/`borrow` annotation because this binder admits only
/// Copy-closed shapes for them (`SPX-O002` reserves those annotations for
/// genuine resource types); `outcome` stays a real owned `Bytes` resource.
pub struct RichProposalStages {
    program: ResolvedProgram,
    schema: CompiledInteractionSchema,
    proposal_graph: InteractionTypeGraph,
    proposal_binding: StageBinding,
    state_type: DeclarationId,
    /// State's sole scalar field. This binder admits only a single-field
    /// State record (see the module's "Known limitation"): `reduce`'s
    /// `continue` case carries that one Copy scalar leaf directly (Copy
    /// Variants v1 forbids a nested State record inside a case), and
    /// [`run_rich_turn`] wraps it back into a real State record keyed by
    /// this exact persistent field identity.
    state_field: DeclarationId,
    authorize: PreparedRetainedCall,
    decision: TwoCaseShape,
    reduce: PreparedRetainedCall,
    transition: TwoCaseShape,
}

impl RichProposalStages {
    /// The derived Agent Interaction Schema v1 this binder decodes untrusted
    /// Proposal bytes against.
    #[must_use]
    pub fn schema(&self) -> &CompiledInteractionSchema {
        &self.schema
    }
}

/// Binds the rich-Proposal `authorize`/`reduce` convention against one
/// checked module.
///
/// `module_path` must name a real, unchanged file on disk containing
/// exactly `module_source` — the same read-then-verify-unchanged discipline
/// [`compile_agent_interaction_schema`] itself requires, since deriving the
/// real #109 schema is `crate::agent_interaction_schema`'s own file-rooted
/// entry point and this binder does not duplicate its derivation.
#[allow(clippy::too_many_arguments)]
pub fn bind_rich_proposal_stages(
    module_source: &str,
    module_path: &Path,
    proposal_type_id: &str,
    state_type_id: &str,
    authorize_fn_id: &str,
    reduce_fn_id: &str,
) -> Result<RichProposalStages, Vec<Diagnostic>> {
    let checked = crate::check(module_source, module_path)?;
    let program = hir::resolve(&checked)?;
    hir::validate(&program).map_err(|error| vec![error])?;

    find_type(&program, proposal_type_id, "proposal_type").map_err(|error| vec![error])?;
    let state_declaration =
        find_type(&program, state_type_id, "state_type").map_err(|error| vec![error])?;
    if proposal_type_id == state_type_id {
        return Err(vec![invariant("proposal_type.state_collision")]);
    }
    let ResolvedTypeDeclarationKind::Record {
        fields: state_fields,
    } = &state_declaration.kind
    else {
        return Err(vec![invariant("state_type.kind")]);
    };
    // A single-field State keeps `reduce`'s `continue` case a direct Copy
    // scalar leaf, admissible under source-level Copy Variants v1
    // (`SPX-T215`); see the module's "Known limitation".
    if state_fields.len() != 1 {
        return Err(vec![invariant("state_type.fields")]);
    }
    let state_field = state_fields[0].id.clone();
    let state_field_type = state_fields[0].ty.clone();
    persistent(&program, state_field.as_str(), "state_type.field").map_err(|error| vec![error])?;

    let schema = compile_agent_interaction_schema(module_path, proposal_type_id)?;
    if schema.schema().root_type_id() != proposal_type_id {
        return Err(vec![invariant("proposal_type.schema_root")]);
    }
    let proposal_graph =
        InteractionTypeGraph::derive(&program, proposal_type_id).map_err(|error| vec![error])?;
    let proposal_binding = StageBinding::new(LifecycleStageRole::Authorize, &schema);

    let state = nominal(state_type_id);
    let proposal = nominal(proposal_type_id);

    // Both State and Proposal are Copy-closed nominal records in this
    // binder's admitted fixtures (every leaf is a direct scalar or a
    // further Copy-closed nested record), so the resolver assigns them
    // `OwnershipMode::Value` rather than `own`/`borrow` -- source-level
    // ownership annotations (`SPX-O002`) are reserved for genuine resource
    // types (a type with an owned `Bytes` leaf somewhere in its tree).
    let authorize_fn =
        find_function(&program, authorize_fn_id, "authorize").map_err(|error| vec![error])?;
    if authorize_fn.params.len() != 2 {
        return Err(vec![invariant("authorize.arity")]);
    }
    require_param(
        authorize_fn,
        0,
        OwnershipMode::Value,
        &state,
        "authorize.state",
    )
    .map_err(|error| vec![error])?;
    require_param(
        authorize_fn,
        1,
        OwnershipMode::Value,
        &proposal,
        "authorize.proposal",
    )
    .map_err(|error| vec![error])?;
    let decision = two_case_shape(
        &program,
        authorize_fn,
        &ResolvedType::Bool,
        &ResolvedType::I64,
        "authorize.decision",
    )
    .map_err(|error| vec![error])?;

    let reduce_fn = find_function(&program, reduce_fn_id, "reduce").map_err(|error| vec![error])?;
    if reduce_fn.params.len() != 3 {
        return Err(vec![invariant("reduce.arity")]);
    }
    require_param(reduce_fn, 0, OwnershipMode::Value, &state, "reduce.state")
        .map_err(|error| vec![error])?;
    require_param(
        reduce_fn,
        1,
        OwnershipMode::Value,
        &proposal,
        "reduce.proposal",
    )
    .map_err(|error| vec![error])?;
    require_param(
        reduce_fn,
        2,
        OwnershipMode::Own,
        &ResolvedType::Bytes,
        "reduce.outcome",
    )
    .map_err(|error| vec![error])?;
    let transition = two_case_shape(
        &program,
        reduce_fn,
        &state_field_type,
        &ResolvedType::U8,
        "reduce.transition",
    )
    .map_err(|error| vec![error])?;
    if transition.variant == decision.variant {
        return Err(vec![invariant("reduce.transition.decision_collision")]);
    }

    let authorize = prepare_retained_call(&program, authorize_fn_id)?;
    let reduce = prepare_retained_call(&program, reduce_fn_id)?;

    Ok(RichProposalStages {
        program,
        schema,
        proposal_graph,
        proposal_binding,
        state_type: DeclarationId::new(state_type_id),
        state_field,
        authorize,
        decision,
        reduce,
        transition,
    })
}

/// The closed outcome of one rich-Proposal turn.
#[derive(Debug)]
pub enum RichTurnOutcome {
    /// `reduce` selected its `continue` case; carries the next State value,
    /// harvested from the real interpreter, not merely digest-equal to it.
    Continue(RetainedValue),
    /// `reduce` selected its `fail` case; carries the exact `i64` code.
    Fail(i64),
    /// `authorize` refused; carries the exact `i64` refusal code. `reduce`
    /// was never dispatched.
    Refused(i64),
}

fn refused(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G588",
        format!("Rich proposal turn refused before dispatch: {field}"),
    )
}

/// Runs one rich-Proposal turn: decode -> admit -> project -> `authorize`
/// -> (if granted) `reduce`.
///
/// `proposal_bytes` is untrusted response bytes, decoded against the real
/// #109 schema before anything else runs. A decode failure (malformed,
/// wrong nominal type/case, stale schema digest, oversized) refuses with no
/// stage ever evaluated and therefore no owned temporary ever constructed —
/// the same ordering [`crate::agent_lifecycle_typed_carrier::ownership`]
/// documents for its own boundary. `reduce` is dispatched only after
/// `authorize` has actually returned its granting case; a refused turn never
/// reaches it.
pub fn run_rich_turn(
    stages: &RichProposalStages,
    state: RetainedValue,
    proposal_bytes: &[u8],
    outcome_bytes: Vec<u8>,
    max_steps: usize,
    cancellation: &AgentCancellation,
) -> Result<RichTurnOutcome, Diagnostic> {
    if cancellation.is_cancelled() {
        return Err(refused("turn.cancelled"));
    }
    if !matches!(&state, RetainedValue::Record(record) if record.record == stages.state_type)
        && !matches!(&state, RetainedValue::Variant(variant) if variant.variant == stages.state_type)
    {
        return Err(refused("turn.state_identity"));
    }
    let decoded = stages
        .schema
        .decode(proposal_bytes)
        .map_err(|_| refused("turn.proposal_malformed"))?;
    let admitted = stages
        .proposal_binding
        .admit(decoded)
        .map_err(|_| refused("turn.proposal_admission"))?;
    let proposal_value = to_retained(&stages.proposal_graph, &admitted)
        .map_err(|_| refused("turn.proposal_projection"))?;

    let authorize_args = [state.clone(), proposal_value.clone()];
    let evaluation = evaluate_retained_call(
        &stages.program,
        &stages.authorize,
        &authorize_args,
        max_steps,
    )
    .map_err(|_| refused("authorize.evaluate"))?;
    let RetainedCallOutcome::Returned(RetainedValue::Variant(decision)) = evaluation.outcome else {
        return Err(refused("authorize.did_not_return"));
    };
    if decision.variant != stages.decision.variant {
        return Err(refused("authorize.decision.identity"));
    }
    if decision.case == stages.decision.negative_case {
        let code = decision
            .fields
            .iter()
            .find(|item| item.field == stages.decision.negative_field)
            .and_then(|item| match &item.value {
                RetainedValue::I64(code) => Some(*code),
                _ => None,
            })
            .ok_or_else(|| refused("authorize.decision.refusal_code"))?;
        return Ok(RichTurnOutcome::Refused(code));
    }
    if decision.case != stages.decision.positive_case {
        return Err(refused("authorize.decision.case"));
    }
    // The Grant seal is verified structurally by `two_case_shape` at bind
    // time (exactly one `Bytes` field); it is not re-inspected here because
    // this slice mints no separate opaque authorization object yet (see the
    // module's "Known limitation").

    let reduce_args = [state, proposal_value, RetainedValue::Bytes(outcome_bytes)];
    let evaluation =
        evaluate_retained_call(&stages.program, &stages.reduce, &reduce_args, max_steps)
            .map_err(|_| refused("reduce.evaluate"))?;
    let RetainedCallOutcome::Returned(RetainedValue::Variant(transition)) = evaluation.outcome
    else {
        return Err(refused("reduce.did_not_return"));
    };
    if transition.variant != stages.transition.variant {
        return Err(refused("reduce.transition.identity"));
    }
    if transition.case == stages.transition.negative_case {
        let code = transition
            .fields
            .iter()
            .find(|item| item.field == stages.transition.negative_field)
            .and_then(|item| match &item.value {
                RetainedValue::U8(code) => Some(i64::from(*code)),
                _ => None,
            })
            .ok_or_else(|| refused("reduce.transition.fail_code"))?;
        return Ok(RichTurnOutcome::Fail(code));
    }
    if transition.case != stages.transition.positive_case {
        return Err(refused("reduce.transition.case"));
    }
    let next_leaf = transition
        .fields
        .iter()
        .find(|item| item.field == stages.transition.positive_field)
        .map(|item| item.value.clone())
        .ok_or_else(|| refused("reduce.transition.next_state"))?;
    // Copy Variants v1 (`SPX-T215`) forbade `Continue` from carrying a
    // nested State record directly, so it carries State's one scalar leaf
    // instead; this is the reconstruction back into a real, correctly
    // identified State record the module documentation describes.
    let next_state = RetainedValue::Record(RetainedRecord {
        record: stages.state_type.clone(),
        fields: vec![RetainedField {
            field: stages.state_field.clone(),
            value: next_leaf,
        }],
    });
    Ok(RichTurnOutcome::Continue(next_state))
}

#[cfg(test)]
mod tests;
