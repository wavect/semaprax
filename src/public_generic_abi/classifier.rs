//! The pure [Public Generic Boundary Profile
//! v1](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) admission
//! classifier — issue #150's implementation half, which no module in this
//! repository implemented before this one.
//!
//! [`classify`] takes trusted checked programme facts (a `ResolvedProgram`)
//! and one selected export identity, and returns either a typed
//! [`AdmittedSubject`] naming the exact admitted input/result instances,
//! their reachable record closure, and the owned parameter's settlement
//! obligations, or one closed [`Refusal`] reason. It never emits descriptor
//! or carrier bytes and never performs target lowering; it is the one
//! trusted, common input a future descriptor producer, candidate-delta
//! integration, or physical adapter selection would share, so none of them
//! has to re-derive or silently redefine admission.
//!
//! This module composes existing hosted-green projections rather than
//! re-deriving their logic:
//!
//! - [`crate::public_generic_type`] (PG-1/PG-2) already classifies every
//!   reachable type into the closed grammar vocabulary and already refuses a
//!   borrowed view, a compiler-owned nominal, an authored class/variant/
//!   resource, an unresolved type parameter, an arity mismatch, and an
//!   ambiguous or missing type declaration — each with its own closed
//!   [`grammar::Rejection`], which this module recovers with
//!   [`grammar::Rejection::of`] rather than parsing prose.
//! - [`crate::public_generic_surface::CandidateSurface`] (PG-3) selects the
//!   export by persistent identity and computes the complete substituted
//!   record closure reachable from its signature.
//! - [`crate::public_generic_settlement::plan`] (PG-7) derives the owned
//!   parameter's settlement obligations and already fails closed when the
//!   grammar's owned-leaf paths disagree with the compiler's own cleanup
//!   inventory or cleanup plan.
//!
//! What this module adds, that none of the above provide on their own:
//!
//! - the v1 profile's own **export-shape** predicate (exactly one owned
//!   input, exactly one owned result, both concrete record instances, no
//!   runtime type-argument input, no declared effect);
//! - **acyclic-closure detection** performed directly over the raw
//!   declaration graph, before the grammar's depth-bounded projection is
//!   asked to describe anything. A genuinely self-referential or mutually
//!   recursive record declaration is refused with the dedicated
//!   [`Refusal::RecursiveClosure`] reason at the exact declaration that
//!   closes the cycle, rather than surfacing indistinguishably as a
//!   depth-bound overrun once the grammar gives up at
//!   [`grammar::MAX_RECORD_DEPTH`] (see [Known
//!   limitations](#known-limitations) below for the one case this cannot
//!   yet separate from the width bound);
//! - **per-record field-count admission** against
//!   [`crate::public_generic_abi::boundary_profile::MAX_FIELDS_PER_RECORD`],
//!   a bound the frozen specification declares but that no other module in
//!   this repository enforces today (the grammar bounds the *total* visited
//!   node count across a whole closure, never one record's own field
//!   count in isolation);
//! - a **closed [`Refusal`] enum**, covering every reason the frozen
//!   specification's [Reserved refusal
//!   vocabulary](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#reserved-refusal-vocabulary)
//!   names, each backed by one real `SPX-PG6xx` diagnostic code. Those
//!   codes are genuinely allocated by this module — [Installed Diagnostics
//!   v1](../../docs/INSTALLED-DIAGNOSTICS-V1.md)'s static source scan finds
//!   them the moment this file is compiled — closing the gap the frozen
//!   specification's own table left open ("no code below is defined in
//!   source yet").
//!
//! # Known limitations
//!
//! - **Cleanup-inventory vs. settlement-obligation mismatch.**
//!   [`crate::public_generic_settlement`] reports every internal
//!   disagreement between the grammar's owned-leaf paths and the compiler's
//!   cleanup facts under one code
//!   ([`crate::public_generic_settlement::SETTLEMENT_DISAGREEMENT`]). This
//!   module maps that single code to
//!   [`Refusal::CleanupInventoryMismatch`] uniformly; it cannot honestly
//!   report [`Refusal::SettlementObligationMismatch`] as a *distinct*,
//!   independently observed condition without parsing the settlement
//!   module's message text, which the owning issue explicitly forbids.
//!   [`Refusal::SettlementObligationMismatch`] exists in the closed
//!   vocabulary and carries its own diagnostic code, but no fixture in this
//!   module's own tests can produce it as distinct from
//!   [`Refusal::CleanupInventoryMismatch`] today; only a change to
//!   `public_generic_settlement` (outside this file's lease) could split
//!   the two apart.
//! - **A record that owns no leaf at all.** A record built only from
//!   admitted Copy-scalar fields (no `Bytes` field anywhere in its closure)
//!   is a valid grammar instance, but
//!   [`crate::public_generic_settlement::plan`] refuses it as "nothing to
//!   settle" (there is no owned leaf, so there is no settlement plan to
//!   derive). This module surfaces that refusal as
//!   [`Refusal::CleanupInventoryMismatch`] via the same settlement-error
//!   translation, since the settlement module does not distinguish "no
//!   leaf" from a structural disagreement with its own dedicated code. In
//!   practice this scenario cannot even be authored as real front-end
//!   source: the resolver's own `SPX-O002` refuses declaring `own` on a
//!   value-only (leafless) type before this classifier ever runs, so the
//!   module's own test for this reason mutates an already-checked program
//!   rather than compiling one from scratch. A scalar-only owned instance is
//!   consequently never admitted by this classifier even though the frozen
//!   specification's own IN/DEFERRED/EXCLUDED table does not explicitly
//!   exclude it; this is an existing constraint inherited from the resolver
//!   and PG-7, not a boundary this classifier chose.
//! - **A compiler-owned nominal's refusal reason depends on where it is
//!   reached.** [`grammar::classify_with`] checks `is_compiler_owned_id`
//!   before it ever looks a declaration up, so a compiler-owned nominal
//!   (`Option`, `Result`, `Vec`, `Box`) reached as an instance *argument* —
//!   a top-level generic parameter position — is
//!   [`grammar::Rejection::CompilerOwnedNominal`], which this module maps to
//!   [`Refusal::TypeOutsideGrammar`]. [`grammar::describe`]'s owned-leaf walk
//!   over a record's *fields* two or more levels deep does not repeat that
//!   check; it validates a nested field purely by the found declaration's
//!   own kind. Since `Option` and `Result` are themselves implemented as
//!   authored variant declarations internally, a compiler-owned nominal
//!   reached through a nested field surfaces as
//!   [`grammar::Rejection::UnadmittedNominalKind`] instead, which this
//!   module maps to [`Refusal::VariantResourceOrFunctionValue`]. Both refuse
//!   the export either way; only the reported reason differs by position,
//!   and this module cannot correct that without changing `grammar` itself
//!   (outside this file's lease).
//! - **Width vs. depth on a self-referential template.** A record directly
//!   or mutually self-referential in its own template declaration (for
//!   example `record Node<T> { child: Node<T> }`) is caught by this
//!   module's own acyclic-closure walk before the grammar is ever asked to
//!   describe it, and reported as [`Refusal::RecursiveClosure`]. A record
//!   family that is a genuine acyclic DAG of declarations but whose
//!   substituted instance closure still exceeds
//!   [`grammar::MAX_RECORD_DEPTH`] or the shared node-visit budget (for
//!   example importing the same finite template at strictly increasing
//!   argument nesting on every level) is not a cycle in the declaration
//!   graph this module walks, so it is refused only once the grammar's own
//!   budget is exceeded, reported as [`Refusal::BoundExceeded`] — correct,
//!   but a coarser signal than a dedicated "unbounded nesting" reason would
//!   give.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    OwnershipMode, ResolvedFunction, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};
use crate::public_generic_abi::boundary_profile::{
    MAX_FIELDS_PER_RECORD, MAX_VISITED_NODES_PER_INSTANCE, OWNED_INPUT_PARAMETER_COUNT,
};
use crate::public_generic_settlement::{self as settlement, SettlementPlan};
use crate::public_generic_surface::CandidateSurface;
use crate::public_generic_type::{self as grammar, InstanceFacts, TypeInventory};

/// The frozen admission-profile schema this classifier implements. Must
/// stay byte-identical to
/// [`crate::public_generic_abi::boundary_profile::BOUNDARY_PROFILE_SCHEMA`];
/// pinned again here so a reader of this module does not have to cross-check
/// the constant's definition to know which specification it answers to.
pub const BOUNDARY_PROFILE_SCHEMA: &str = "semaprax.public-generic-boundary-profile.v1";

const SUBJECT_DOMAIN: &[u8] = b"semaprax.public-generic-boundary-profile.v1.admitted-subject\0";

/// The selected export identity does not name a checked declaration.
pub const EXPORT_NOT_FOUND: &str = "SPX-PG601";
/// The selected identity names a generic function template, never a
/// candidate export.
pub const GENERIC_FUNCTION_TEMPLATE: &str = "SPX-PG602";
/// The export does not have exactly one parameter.
pub const WRONG_PARAMETER_COUNT: &str = "SPX-PG603";
/// The export's one parameter is not owned (`own`).
pub const WRONG_OWNERSHIP_MODE: &str = "SPX-PG604";
/// The result type is not a fully concrete authored record instance.
pub const UNSUPPORTED_RESULT_SHAPE: &str = "SPX-PG605";
/// A reachable position names an unsubstituted function-level type
/// parameter: there is no runtime type-argument input in v1.
pub const UNRESOLVED_TYPE_ARGUMENT: &str = "SPX-PG606";
/// A nominal's argument count does not equal its declared arity.
pub const ARITY_MISMATCH: &str = "SPX-PG607";
/// A reachable type is outside the admitted grammar vocabulary.
pub const TYPE_OUTSIDE_GRAMMAR: &str = "SPX-PG608";
/// A reachable position is a borrowed view (`str` or `Slice<u8>`).
pub const BORROWED_FIELD_PRESENT: &str = "SPX-PG609";
/// A reachable nominal is an authored class, variant, or resource
/// declaration, or the position is a function value.
pub const VARIANT_RESOURCE_OR_FUNCTION_VALUE: &str = "SPX-PG610";
/// The declaration graph reachable from the input or result closes a cycle.
pub const RECURSIVE_CLOSURE: &str = "SPX-PG611";
/// Two checked declarations share one persistent identity.
pub const AMBIGUOUS_STABLE_IDENTITY: &str = "SPX-PG612";
/// A record, field, depth, leaf, or payload bound was exceeded.
pub const BOUND_EXCEEDED: &str = "SPX-PG613";
/// The derived obligations disagree with the compiler's own cleanup
/// inventory or cleanup plan (including "the instance owns no leaf, so
/// nothing settles" — see the module's Known limitations).
pub const CLEANUP_INVENTORY_MISMATCH: &str = "SPX-PG614";
/// Reserved for a settlement-obligation disagreement independently
/// distinguished from [`CLEANUP_INVENTORY_MISMATCH`]. No fixture in this
/// module produces it; see the module's Known limitations.
pub const SETTLEMENT_OBLIGATION_MISMATCH: &str = "SPX-PG615";
/// The export declares one or more effects; v1 requires synchronous,
/// effect-free functions.
pub const EFFECTFUL_EXPORT: &str = "SPX-PG616";
/// A retained fact needed to classify the export could not be reconciled
/// (internal consistency failure between two checked projections).
pub const INCOMPATIBLE_RETAINED_FACTS: &str = "SPX-PG617";
/// The input type is not a fully concrete authored record instance. Beyond
/// the seventeen codes the frozen specification reserves (`SPX-PG601`
/// through `SPX-PG617`): the specification's own table only names
/// "unsupported result shape," so this classifier allocates the next free
/// `SPX-PG6xx` code for the symmetric input-side case rather than
/// overloading [`UNSUPPORTED_RESULT_SHAPE`] for a position it does not
/// name.
pub const UNSUPPORTED_INPUT_SHAPE: &str = "SPX-PG618";

/// Why one export was refused admission under this profile. Closed: a new
/// reason is a new classifier version, exactly like
/// [`grammar::Rejection`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    ExportNotFound,
    GenericFunctionTemplate,
    WrongParameterCount { found: usize },
    WrongOwnershipMode,
    UnsupportedInputShape,
    UnsupportedResultShape,
    UnresolvedTypeArgument,
    ArityMismatch,
    TypeOutsideGrammar,
    BorrowedFieldPresent,
    VariantResourceOrFunctionValue,
    RecursiveClosure,
    AmbiguousStableIdentity,
    BoundExceeded,
    CleanupInventoryMismatch,
    SettlementObligationMismatch,
    EffectfulExport,
    IncompatibleRetainedFacts,
}

impl Refusal {
    /// The real, allocated diagnostic code for this reason.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ExportNotFound => EXPORT_NOT_FOUND,
            Self::GenericFunctionTemplate => GENERIC_FUNCTION_TEMPLATE,
            Self::WrongParameterCount { .. } => WRONG_PARAMETER_COUNT,
            Self::WrongOwnershipMode => WRONG_OWNERSHIP_MODE,
            Self::UnsupportedInputShape => UNSUPPORTED_INPUT_SHAPE,
            Self::UnsupportedResultShape => UNSUPPORTED_RESULT_SHAPE,
            Self::UnresolvedTypeArgument => UNRESOLVED_TYPE_ARGUMENT,
            Self::ArityMismatch => ARITY_MISMATCH,
            Self::TypeOutsideGrammar => TYPE_OUTSIDE_GRAMMAR,
            Self::BorrowedFieldPresent => BORROWED_FIELD_PRESENT,
            Self::VariantResourceOrFunctionValue => VARIANT_RESOURCE_OR_FUNCTION_VALUE,
            Self::RecursiveClosure => RECURSIVE_CLOSURE,
            Self::AmbiguousStableIdentity => AMBIGUOUS_STABLE_IDENTITY,
            Self::BoundExceeded => BOUND_EXCEEDED,
            Self::CleanupInventoryMismatch => CLEANUP_INVENTORY_MISMATCH,
            Self::SettlementObligationMismatch => SETTLEMENT_OBLIGATION_MISMATCH,
            Self::EffectfulExport => EFFECTFUL_EXPORT,
            Self::IncompatibleRetainedFacts => INCOMPATIBLE_RETAINED_FACTS,
        }
    }

    /// The closed wire spelling, stable across releases of this classifier.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::ExportNotFound => "export_not_found",
            Self::GenericFunctionTemplate => "generic_function_template",
            Self::WrongParameterCount { .. } => "wrong_parameter_count",
            Self::WrongOwnershipMode => "wrong_ownership_mode",
            Self::UnsupportedInputShape => "unsupported_input_shape",
            Self::UnsupportedResultShape => "unsupported_result_shape",
            Self::UnresolvedTypeArgument => "unresolved_type_argument",
            Self::ArityMismatch => "arity_mismatch",
            Self::TypeOutsideGrammar => "type_outside_grammar",
            Self::BorrowedFieldPresent => "borrowed_field_present",
            Self::VariantResourceOrFunctionValue => "variant_resource_or_function_value",
            Self::RecursiveClosure => "recursive_closure",
            Self::AmbiguousStableIdentity => "ambiguous_stable_identity",
            Self::BoundExceeded => "bound_exceeded",
            Self::CleanupInventoryMismatch => "cleanup_inventory_mismatch",
            Self::SettlementObligationMismatch => "settlement_obligation_mismatch",
            Self::EffectfulExport => "effectful_export",
            Self::IncompatibleRetainedFacts => "incompatible_retained_facts",
        }
    }

    /// Render as the [`Diagnostic`] a caller can surface directly.
    pub fn diagnostic(&self) -> Diagnostic {
        let mut message = format!(
            "{BOUNDARY_PROFILE_SCHEMA} refuses this export: {}",
            self.reason()
        );
        if let Self::WrongParameterCount { found } = self {
            message.push_str(&format!(
                " (found {found}, v1 requires exactly {OWNED_INPUT_PARAMETER_COUNT})"
            ));
        }
        Diagnostic::io(self.code(), message)
    }
}

/// The complete typed result of one successful classification: the exact
/// admitted input and result instances, the reachable record closure, and
/// the owned parameter's settlement obligations. Never a descriptor or
/// carrier value; those are derived from this by a later stage.
#[derive(Clone, Debug, PartialEq)]
pub struct AdmittedSubject {
    export_id: String,
    export_name: String,
    input: InstanceFacts,
    result: InstanceFacts,
    record_closure: BTreeMap<String, InstanceFacts>,
    settlement: SettlementPlan,
}

impl AdmittedSubject {
    /// The export's persistent declaration identity.
    pub fn export_id(&self) -> &str {
        &self.export_id
    }

    /// The export's presentation name. Never identity-bearing.
    pub fn export_name(&self) -> &str {
        &self.export_name
    }

    /// The admitted owned input instance's complete grammar facts.
    pub fn input(&self) -> &InstanceFacts {
        &self.input
    }

    /// The admitted owned result instance's complete grammar facts.
    pub fn result(&self) -> &InstanceFacts {
        &self.result
    }

    /// Every reachable record instance in the signature's substituted
    /// closure, keyed by canonical term, including the input and result
    /// instances themselves.
    pub fn record_closure(&self) -> &BTreeMap<String, InstanceFacts> {
        &self.record_closure
    }

    /// The owned input parameter's settlement obligations, already checked
    /// to agree with the compiler's own cleanup inventory and cleanup plan.
    pub fn settlement(&self) -> &SettlementPlan {
        &self.settlement
    }

    /// A domain-separated digest over every identity-bearing fact this
    /// classification depends on: the export identity and the input,
    /// result, and settlement digests. Two classifications of the same
    /// checked export against the same program always agree; a display
    /// rename of the export, its parameter, or any reachable record or
    /// field never moves it.
    pub fn digest(&self) -> String {
        let mut preimage = Vec::new();
        crate::public_generic_abi::frame(&mut preimage, self.export_id.as_bytes());
        crate::public_generic_abi::frame(&mut preimage, self.input.instance_digest.as_bytes());
        crate::public_generic_abi::frame(&mut preimage, self.result.instance_digest.as_bytes());
        crate::public_generic_abi::frame(&mut preimage, self.settlement.digest().as_bytes());
        crate::public_generic_abi::digest(SUBJECT_DOMAIN, &preimage)
    }
}

/// Classify `export_id` against `program` under [Public Generic Boundary
/// Profile v1](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md).
///
/// `export_id` is a persistent declaration identity, never a display name.
/// Every fact the returned [`AdmittedSubject`] carries is re-derived from
/// `program`; nothing is read back from a previously emitted artifact, and
/// nothing here allocates, transfers, executes, or grants any authority.
pub fn classify(program: &ResolvedProgram, export_id: &str) -> Result<AdmittedSubject, Refusal> {
    if program
        .function_templates
        .iter()
        .any(|template| template.id.as_str() == export_id)
    {
        return Err(Refusal::GenericFunctionTemplate);
    }

    let mut function: Option<&ResolvedFunction> = None;
    for candidate in &program.functions {
        if candidate.id.as_str() == export_id {
            if function.is_some() {
                return Err(Refusal::AmbiguousStableIdentity);
            }
            function = Some(candidate);
        }
    }
    let function = function.ok_or(Refusal::ExportNotFound)?;

    if function.params.len() != OWNED_INPUT_PARAMETER_COUNT {
        return Err(Refusal::WrongParameterCount {
            found: function.params.len(),
        });
    }
    let parameter = &function.params[0];
    if parameter.ownership != OwnershipMode::Own {
        return Err(Refusal::WrongOwnershipMode);
    }
    if !function.effects.is_empty() {
        return Err(Refusal::EffectfulExport);
    }
    if !matches!(parameter.ty, ResolvedType::Nominal { .. }) {
        return Err(Refusal::UnsupportedInputShape);
    }
    if !matches!(function.return_type, ResolvedType::Nominal { .. }) {
        return Err(Refusal::UnsupportedResultShape);
    }

    check_acyclic(program, &parameter.ty)?;
    check_acyclic(program, &function.return_type)?;
    check_field_counts(program, &parameter.ty)?;
    check_field_counts(program, &function.return_type)?;

    let inventory = TypeInventory::of(program);
    let input = grammar::describe(&inventory, &parameter.ty).map_err(translate_grammar_error)?;
    let result =
        grammar::describe(&inventory, &function.return_type).map_err(translate_grammar_error)?;

    let surface = CandidateSurface::derive(program, &[export_id.to_owned()])
        .map_err(translate_grammar_error)?;

    let settlement =
        settlement::plan(&inventory, function, 0).map_err(translate_settlement_error)?;

    Ok(AdmittedSubject {
        export_id: export_id.to_owned(),
        export_name: function.name.clone(),
        input,
        result,
        record_closure: surface.instances().clone(),
        settlement,
    })
}

/// Map one [`grammar::Rejection`] (or an unrecovered grammar/surface
/// capacity diagnostic) onto this module's closed [`Refusal`] vocabulary.
/// Never parses prose: every case is decided by the diagnostic's typed
/// identity ([`grammar::Rejection::of`]) or its stable code, exactly as the
/// owning issue requires.
fn translate_grammar_error(diagnostic: Diagnostic) -> Refusal {
    if let Some(rejection) = grammar::Rejection::of(&diagnostic) {
        return match rejection {
            grammar::Rejection::TypeParameter => Refusal::UnresolvedTypeArgument,
            grammar::Rejection::OwnedString
            | grammar::Rejection::Unit
            | grammar::Rejection::InlineByteArray
            | grammar::Rejection::FunctionType
            | grammar::Rejection::CompilerOwnedNominal => Refusal::TypeOutsideGrammar,
            grammar::Rejection::BorrowedStr | grammar::Rejection::BorrowedByteView => {
                Refusal::BorrowedFieldPresent
            }
            grammar::Rejection::UnadmittedNominalKind => Refusal::VariantResourceOrFunctionValue,
            grammar::Rejection::MissingDeclaration => Refusal::IncompatibleRetainedFacts,
            grammar::Rejection::AmbiguousDeclaration => Refusal::AmbiguousStableIdentity,
            grammar::Rejection::ArityMismatch => Refusal::ArityMismatch,
        };
    }
    if diagnostic.code == grammar::GRAMMAR_CAPACITY
        || diagnostic.code == crate::public_generic_surface::SURFACE_CAPACITY
    {
        return Refusal::BoundExceeded;
    }
    Refusal::IncompatibleRetainedFacts
}

/// Map one [`settlement`] diagnostic onto this module's closed vocabulary.
/// See the module's Known limitations: the settlement module does not
/// itself distinguish "no owned leaf" from a structural disagreement, so
/// both collapse to [`Refusal::CleanupInventoryMismatch`] here.
fn translate_settlement_error(diagnostic: Diagnostic) -> Refusal {
    match diagnostic.code {
        settlement::UNSUPPORTED_PARAMETER | settlement::SETTLEMENT_DISAGREEMENT => {
            Refusal::CleanupInventoryMismatch
        }
        _ => Refusal::IncompatibleRetainedFacts,
    }
}

/// Detect a genuine cycle in the raw declaration graph reachable from `ty`,
/// before the grammar's depth-bounded projection is asked to describe
/// anything. Walks *unsubstituted* field types: a declaration identity that
/// reappears on the current path — directly self-referential or through
/// mutual recursion — is a cycle regardless of what concrete arguments
/// would eventually be substituted, since substitution never removes a
/// field occurrence, only renames the type parameters inside it.
pub(crate) fn check_acyclic(program: &ResolvedProgram, ty: &ResolvedType) -> Result<(), Refusal> {
    let mut active: Vec<&str> = Vec::new();
    let mut budget = 0usize;
    walk_acyclic(program, ty, &mut active, &mut budget)
}

fn walk_acyclic<'a>(
    program: &'a ResolvedProgram,
    ty: &'a ResolvedType,
    active: &mut Vec<&'a str>,
    budget: &mut usize,
) -> Result<(), Refusal> {
    *budget += 1;
    if *budget > MAX_VISITED_NODES_PER_INSTANCE {
        return Err(Refusal::BoundExceeded);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    let id = declaration.as_str();
    if active.contains(&id) {
        return Err(Refusal::RecursiveClosure);
    }
    active.push(id);
    for argument in arguments {
        walk_acyclic(program, argument, active, budget)?;
    }
    if let Some(found) = program.types.iter().find(|d| d.id.as_str() == id) {
        if let ResolvedTypeDeclarationKind::Record { fields } = &found.kind {
            for field in fields {
                walk_acyclic(program, &field.ty, active, budget)?;
            }
        }
    }
    active.pop();
    Ok(())
}

/// Enforce [`MAX_FIELDS_PER_RECORD`] on every record declaration reachable
/// from `ty`. No other module in this repository independently reimplements
/// this bound (the grammar bounds the *total* visited node count across a
/// whole closure, never one record's own field count in isolation); the
/// frozen specification declares the bound, so this classifier is the first
/// to admit or refuse by it, and
/// [`crate::public_generic_abi::descriptor::producer`] reuses this exact
/// `pub(crate)` function rather than re-deriving its own copy.
///
/// Assumes [`check_acyclic`] has already run over the same root: a genuine
/// cycle would make "already visited" ambiguous with "still expanding," so
/// this walk must never be reachable from a value the acyclic check would
/// have refused.
pub(crate) fn check_field_counts(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<(), Refusal> {
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut budget = 0usize;
    walk_field_counts(program, ty, &mut visited, &mut budget)
}

fn walk_field_counts<'a>(
    program: &'a ResolvedProgram,
    ty: &'a ResolvedType,
    visited: &mut BTreeSet<&'a str>,
    budget: &mut usize,
) -> Result<(), Refusal> {
    *budget += 1;
    if *budget > MAX_VISITED_NODES_PER_INSTANCE {
        return Err(Refusal::BoundExceeded);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(());
    };
    let id = declaration.as_str();
    for argument in arguments {
        walk_field_counts(program, argument, visited, budget)?;
    }
    if !visited.insert(id) {
        return Ok(());
    }
    if let Some(found) = program.types.iter().find(|d| d.id.as_str() == id) {
        if let ResolvedTypeDeclarationKind::Record { fields } = &found.kind {
            if fields.len() > MAX_FIELDS_PER_RECORD {
                return Err(Refusal::BoundExceeded);
            }
            for field in fields {
                walk_field_counts(program, &field.ty, visited, budget)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
