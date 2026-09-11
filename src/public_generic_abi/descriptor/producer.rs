//! Derive a [`DescriptorV1`] from a real checked
//! `ResolvedProgram`, instead of the hand-built fixtures the codec's own
//! tests use.
//!
//! This closes the gap the [Public Generic Descriptor
//! v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md) specification names in
//! its own nonclaims: "it is not derived from real checked HIR in this
//! round." This module's own v1 export-shape predicate below predates
//! [Public Generic Boundary Profile
//! v1](../../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s own classifier
//! (issue #150's implementation half, [`crate::public_generic_abi::classifier`]),
//! which did not exist anywhere in this repository when this predicate was
//! first written. That classifier exists now; see [Sharing the classifier's
//! bound checks](#sharing-the-classifiers-bound-checks-issue-150-follow-up)
//! below for exactly how much of it this module reuses today and what
//! converging onto it fully would still require.
//! [`generate_public_generic_descriptor`] performs the same shape checks
//! locally, reusing the existing hosted-green projections that already read
//! checked HIR for the closely related gates rather than re-deriving them:
//!
//! - [`crate::public_generic_surface::CandidateSurface`] (PG-3) selects the
//!   export by persistent identity, refuses a generic template or an unknown
//!   or ambiguous selection, and computes the complete substituted record
//!   closure reachable from the signature — the descriptor's input, result,
//!   and reachable-instance facts are read from it unchanged.
//! - [`crate::public_generic_settlement::plan`] (PG-7) derives the owned
//!   parameter's settlement obligations and already fails closed if the
//!   grammar's owned-leaf paths disagree with the compiler's own cleanup
//!   inventory or cleanup plan.
//!
//! What this module adds is the v1 profile's own export-shape predicate
//! (exactly one owned input, exactly one owned result, both concrete record
//! instances, no declared effect) and the three programme-subject digests
//! the codec treats as opaque: `program_root_digest` (a domain-separated
//! digest over the sorted, deduplicated set of every persistent declaration
//! identity in `program` — every checked type, function, function template,
//! and function instance — which names exactly which checked program the
//! export was classified in without depending on any display name),
//! `source_projection_digest` (a domain-separated digest of the caller
//! supplied canonical source revision), and `public_surface_digest` (the
//! candidate surface's own digest, reused unchanged).
//!
//! `program_root_digest`'s declaration-identity inventory is deliberately
//! the minimal real binding that is still rename-invariant and re-derived
//! from checked facts rather than guessed: it changes if a declaration is
//! added, removed, or renamed identity, but two programs that declare the
//! identical set of persistent identities with different field types or
//! bodies are not distinguished by it alone. Widening it into a full
//! structural digest (field and parameter type terms, ownership modes,
//! effects) is straightforward future work using the same `TypeInventory`
//! this module already builds, and was not required to satisfy this round's
//! rename-invariance, determinism, and cross-pair replay requirements, which
//! are satisfied by the input/result instance digests and the settlement
//! digests already bound here.
//!
//! Cleanup-inventory, cleanup-plan, and settlement-obligation digests are
//! computed and exposed on [`GeneratedDescriptor`] for a caller to inspect or
//! bind separately; they are **not** part of the frozen `DescriptorV1` wire
//! bytes, whose exact eleven-field identity preimage is fixed and must not be
//! widened in place (see [Public Generic Descriptor
//! v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#canonical-bytes)). A
//! future wire-format version, not this one, is where those digests would
//! need to move if the milestone decides they must ride inside the wire
//! bytes rather than beside them.
//!
//! Generation performs no build, filesystem write, registry access, network
//! call, or code execution. Public generic ownership remains unsupported and
//! unpublished; a [`GeneratedDescriptor`] is not a public ABI.
//!
//! ## Sharing the classifier's bound checks (issue #150 follow-up)
//!
//! Issue #150 has since landed [`crate::public_generic_abi::classifier`],
//! the boundary profile's own admission classifier this module's own
//! documentation above once said "does not exist anywhere in this
//! repository yet." This module still performs its own local export-shape
//! predicate rather than calling [`classifier::classify`] wholesale — the
//! two modules' refusal vocabularies (`SPX-PG7xx` here, `SPX-PG6xx` there)
//! and precedence order are independently documented and tested, and
//! converging them onto one call path is a real, separately reviewable
//! design change (it would retire this module's own `SPX-PG705`-`SPX-PG708`
//! diagnostics and every test asserting them), not this round's job. What
//! this round does close, because it is a strict, additive correctness gap
//! rather than a vocabulary change: this module now calls the classifier's
//! own `pub(crate)` [`classifier::check_field_counts`] helper on the
//! admitted input and result types, rather than leaving
//! [`crate::public_generic_abi::boundary_profile::MAX_FIELDS_PER_RECORD`]
//! completely unenforced here as before — no other check in this module's
//! own predicate or in `CandidateSurface::derive` bounds one record's own
//! field count in isolation, only the *total* visited-node count across the
//! whole closure. This module does **not** also call the classifier's
//! `check_acyclic`: a genuinely self-referential record is already refused
//! above, when `CandidateSurface::derive` runs first and its own bounded
//! walk hits `MAX_RECORD_DEPTH` (a coarser, differently coded refusal than
//! the classifier's own dedicated [`classifier::Refusal::RecursiveClosure`],
//! but a refusal all the same) — calling `check_acyclic` here would be
//! unreachable dead code, never a second layer of defense, because nothing
//! satisfying its precondition (a value `CandidateSurface::derive` already
//! accepted) can also be cyclic. Every subject this module previously
//! admitted successfully still admits identically; the only newly reachable
//! refusal is a record exceeding the per-record field-count bound
//! ([`classifier::Refusal::BoundExceeded`], `SPX-PG613`), previously
//! silently admitted into an unbounded descriptor.

use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedFunction, ResolvedProgram};
use crate::public_generic_abi::classifier;
use crate::public_generic_abi::{digest, frame};
use crate::public_generic_settlement::{self as settlement, SettlementPlan};
use crate::public_generic_surface::CandidateSurface;
use crate::public_generic_type::{InstanceFacts, TypeInventory};

use super::{DescriptorV1, InstanceBinding};

const PROGRAM_ROOT_DOMAIN: &[u8] = b"semaprax.public-generic-descriptor.v1.program-root\0";
const SOURCE_PROJECTION_DOMAIN: &[u8] =
    b"semaprax.public-generic-descriptor.v1.source-projection\0";
const CLEANUP_INVENTORY_DOMAIN: &[u8] =
    b"semaprax.public-generic-descriptor.v1.cleanup-inventory\0";
const CLEANUP_PLAN_DOMAIN: &[u8] = b"semaprax.public-generic-descriptor.v1.cleanup-plan\0";

/// The selected export does not have exactly one owned aggregate input
/// parameter, per [Public Generic Boundary Profile
/// v1](../../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s export-shape
/// rule 5.
pub const WRONG_PARAMETER_COUNT: &str = "SPX-PG705";
/// The selected export's one parameter is not owned (`own`).
pub const WRONG_OWNERSHIP_MODE: &str = "SPX-PG706";
/// The input or result position is not a fully concrete authored record
/// instance (it is a bare scalar, `Bytes`, or a borrowed view at the top
/// level).
pub const UNSUPPORTED_INSTANCE_SHAPE: &str = "SPX-PG707";
/// The selected export declares one or more effects; v1 requires a
/// synchronous, effect-free function.
pub const EFFECTFUL_EXPORT: &str = "SPX-PG708";
/// The rendered descriptor's wire bytes exceed
/// [`crate::public_generic_abi::boundary_profile::MAX_DESCRIPTOR_WIRE_BYTES`].
pub const DESCRIPTOR_RENDER_BUDGET_EXCEEDED: &str = "SPX-PG709";

fn shape(code: &'static str, subject: &str) -> Diagnostic {
    Diagnostic::io(
        code,
        format!("semaprax.public-generic-descriptor.v1 generation refused: {subject}"),
    )
}

/// Every fact the producer derived beyond the frozen `DescriptorV1` wire
/// bytes themselves: the reachable record closure, the settlement plan, and
/// the three cleanup/settlement digests the wire format does not carry.
///
/// This is the `DescriptorArtifact` the generation API is required to
/// return: canonical bytes, the descriptor digest, the trusted admitted
/// profile subject (the export id and the programme-subject digests bound
/// inside `descriptor`), and the target-neutral settlement facts a later
/// carrier or provider-binding round needs.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedDescriptor {
    descriptor: DescriptorV1,
    wire_bytes: Vec<u8>,
    input_facts: InstanceFacts,
    result_facts: InstanceFacts,
    record_closure: BTreeMap<String, InstanceFacts>,
    settlement: SettlementPlan,
    cleanup_inventory_digest: String,
    cleanup_plan_digest: String,
    settlement_obligations_digest: String,
}

impl GeneratedDescriptor {
    /// The frozen `DescriptorV1` value.
    pub fn descriptor(&self) -> &DescriptorV1 {
        &self.descriptor
    }

    /// The exact canonical wire bytes (`descriptor.encode()`), already
    /// bound-checked and self-verified by an independent replay of these
    /// same bytes against `descriptor`.
    pub fn wire_bytes(&self) -> &[u8] {
        &self.wire_bytes
    }

    /// The domain-separated identity digest of `descriptor`.
    pub fn descriptor_digest(&self) -> String {
        self.descriptor.identity_digest()
    }

    /// The complete grammar facts for the owned input instance.
    pub fn input_facts(&self) -> &InstanceFacts {
        &self.input_facts
    }

    /// The complete grammar facts for the owned result instance.
    pub fn result_facts(&self) -> &InstanceFacts {
        &self.result_facts
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

    /// A domain-separated digest of the cleanup inventory's structural leaf
    /// order and per-leaf lifecycle, read from the settlement plan's already
    /// cross-checked obligations.
    pub fn cleanup_inventory_digest(&self) -> &str {
        &self.cleanup_inventory_digest
    }

    /// A domain-separated digest of the cleanup plan's transfer unit: the
    /// owned parameter transferred whole, never per-leaf.
    pub fn cleanup_plan_digest(&self) -> &str {
        &self.cleanup_plan_digest
    }

    /// The settlement plan's own domain-separated digest, covering the
    /// export, parameter index, instance identity, transfer unit, and every
    /// ordered obligation together.
    pub fn settlement_obligations_digest(&self) -> &str {
        &self.settlement_obligations_digest
    }
}

/// The sorted, deduplicated, length-framed set of every persistent
/// declaration identity in `program`: every checked type, function, function
/// template, and function instance. Never a display name, so a rename never
/// changes it. See the module documentation for exactly what this does and
/// does not bind.
fn declaration_identity_preimage(program: &ResolvedProgram) -> Vec<u8> {
    let mut ids: Vec<&str> = Vec::new();
    ids.extend(
        program
            .types
            .iter()
            .map(|declaration| declaration.id.as_str()),
    );
    ids.extend(
        program
            .function_templates
            .iter()
            .map(|declaration| declaration.id.as_str()),
    );
    ids.extend(
        program
            .functions
            .iter()
            .map(|function| function.id.as_str()),
    );
    ids.extend(
        program
            .function_instances
            .iter()
            .map(|instance| instance.id.as_str()),
    );
    ids.sort_unstable();
    ids.dedup();

    let mut preimage = Vec::new();
    frame(&mut preimage, &(ids.len() as u64).to_le_bytes());
    for id in ids {
        frame(&mut preimage, id.as_bytes());
    }
    preimage
}

/// Look up the checked function for `export_id`. `CandidateSurface::derive`
/// already required exactly one declaration with this identity that is not a
/// generic template, using this same equality; by the time a caller reaches
/// here that lookup has already succeeded once, so a second miss would be an
/// internal inconsistency in `program` between the two lookups, not a
/// reachable user-facing refusal.
fn find_function<'a>(program: &'a ResolvedProgram, export_id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == export_id)
        .expect("CandidateSurface::derive already found exactly this declaration")
}

/// Derive one [`GeneratedDescriptor`] from a real checked `program`, naming
/// `export_id` (a persistent declaration identity, never a display name).
///
/// `source_revision` is the caller's already-computed canonical source
/// revision for `program` (for example
/// [`crate::graph::revision`]'s output over the same parsed `Program` that
/// was resolved into `program`) — a checked, deterministic fact about the
/// retained source projection, never raw untrusted bytes and never guessed.
///
/// Every fact bound into the returned descriptor is re-derived from
/// `program` and `source_revision`; nothing is read back from a previously
/// emitted artifact. Generation refuses, rather than silently encoding, any
/// export outside [Public Generic Boundary Profile
/// v1](../../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)'s v1 shape.
pub fn generate_public_generic_descriptor(
    program: &ResolvedProgram,
    source_revision: &str,
    export_id: &str,
) -> Result<GeneratedDescriptor, Diagnostic> {
    let surface = CandidateSurface::derive(program, &[export_id.to_owned()])?;
    let entry = surface
        .entries()
        .get(export_id)
        .expect("CandidateSurface::derive admitted exactly the requested export");

    if entry.parameters.len() != 1 {
        return Err(shape(
            WRONG_PARAMETER_COUNT,
            &format!(
                "`{export_id}` has {} parameters; v1 requires exactly one owned aggregate input",
                entry.parameters.len()
            ),
        ));
    }
    let input = &entry.parameters[0];
    if input.ownership != "own" {
        return Err(shape(
            WRONG_OWNERSHIP_MODE,
            &format!(
                "`{export_id}`'s parameter has ownership `{}`; v1 requires `own`",
                input.ownership
            ),
        ));
    }
    if input.value.kind != "data" || input.value.instance_digest.is_none() {
        return Err(shape(
            UNSUPPORTED_INSTANCE_SHAPE,
            &format!(
                "`{export_id}`'s input is not a concrete authored record instance (kind `{}`)",
                input.value.kind
            ),
        ));
    }
    if entry.result.kind != "data" || entry.result.instance_digest.is_none() {
        return Err(shape(
            UNSUPPORTED_INSTANCE_SHAPE,
            &format!(
                "`{export_id}`'s result is not a concrete authored record instance (kind `{}`)",
                entry.result.kind
            ),
        ));
    }
    if !entry.effects.is_empty() {
        return Err(shape(
            EFFECTFUL_EXPORT,
            &format!(
                "`{export_id}` declares {} effect(s); v1 requires a synchronous, effect-free function",
                entry.effects.len()
            ),
        ));
    }

    let function = find_function(program, export_id);

    // Reuse, not reinvention: [`MAX_FIELDS_PER_RECORD`] is a frozen bound
    // this module's own local shape predicate above never enforced, and
    // `public_generic_surface::CandidateSurface::derive` above bounds only
    // the *total* visited-node count across the whole closure (already run,
    // successfully, to reach this point), never one record's own field
    // count in isolation — so a record over this bound but still under the
    // shared total-node budget reaches here undetected without this call.
    // Reused directly from the classifier (issue #150) rather than
    // re-derived; see the module doc's "Sharing the classifier's bound
    // checks" section above for why this module does not also call the
    // classifier's `check_acyclic`: a genuinely self-referential record is
    // already refused above, by `CandidateSurface::derive`'s own bounded
    // walk hitting `MAX_RECORD_DEPTH` before ever reaching this line, so an
    // acyclicity check here would be unreachable dead code, not a second
    // layer of defense.
    classifier::check_field_counts(program, &function.params[0].ty)
        .map_err(|refusal| refusal.diagnostic())?;
    classifier::check_field_counts(program, &function.return_type)
        .map_err(|refusal| refusal.diagnostic())?;

    let input_facts = surface
        .instances()
        .get(&input.value.term)
        .cloned()
        .ok_or_else(|| {
            shape(
                UNSUPPORTED_INSTANCE_SHAPE,
                "the input instance's substituted facts could not be reconstructed",
            )
        })?;
    let result_facts = surface
        .instances()
        .get(&entry.result.term)
        .cloned()
        .ok_or_else(|| {
            shape(
                UNSUPPORTED_INSTANCE_SHAPE,
                "the result instance's substituted facts could not be reconstructed",
            )
        })?;

    let inventory = TypeInventory::of(program);
    let settlement = settlement::plan(&inventory, function, 0)?;

    let mut inventory_preimage = Vec::new();
    frame(
        &mut inventory_preimage,
        &(settlement.obligations().len() as u64).to_le_bytes(),
    );
    for obligation in settlement.obligations() {
        frame(&mut inventory_preimage, &obligation.index.to_le_bytes());
        frame(&mut inventory_preimage, obligation.path.as_bytes());
        frame(&mut inventory_preimage, obligation.lifecycle.as_bytes());
    }
    let cleanup_inventory_digest = digest(CLEANUP_INVENTORY_DOMAIN, &inventory_preimage);
    let cleanup_plan_digest = digest(CLEANUP_PLAN_DOMAIN, settlement.transfer_unit().as_bytes());
    let settlement_obligations_digest = settlement.digest().to_owned();

    let program_root_digest = digest(PROGRAM_ROOT_DOMAIN, &declaration_identity_preimage(program));
    let source_projection_digest = digest(SOURCE_PROJECTION_DOMAIN, source_revision.as_bytes());

    let descriptor = DescriptorV1::new(
        export_id,
        entry.name.clone(),
        program_root_digest,
        source_projection_digest,
        surface.digest().to_owned(),
        InstanceBinding::from_facts(&input_facts),
        InstanceBinding::from_facts(&result_facts),
    );

    let wire_bytes = descriptor.encode();
    if wire_bytes.len() > crate::public_generic_abi::boundary_profile::MAX_DESCRIPTOR_WIRE_BYTES {
        return Err(shape(
            DESCRIPTOR_RENDER_BUDGET_EXCEEDED,
            &format!(
                "rendered descriptor is {} bytes, exceeding the {}-byte bound",
                wire_bytes.len(),
                crate::public_generic_abi::boundary_profile::MAX_DESCRIPTOR_WIRE_BYTES
            ),
        ));
    }
    // Step 10 of the generation contract: self-verify by independently
    // replaying the just-rendered bytes against the value that produced
    // them. This is a local generation assertion only, never the
    // independent verifier issue #152 owns.
    super::replay(&wire_bytes, &descriptor)?;

    Ok(GeneratedDescriptor {
        descriptor,
        wire_bytes,
        input_facts,
        result_facts,
        record_closure: surface.instances().clone(),
        settlement,
        cleanup_inventory_digest,
        cleanup_plan_digest,
        settlement_obligations_digest,
    })
}

/// `true` when `mode` is the v1 profile's only admitted parameter ownership.
/// Exposed so a caller assembling its own selection can pre-filter without
/// duplicating the constant.
pub const fn is_admitted_input_ownership(mode: OwnershipMode) -> bool {
    matches!(mode, OwnershipMode::Own)
}

#[cfg(test)]
mod tests;
