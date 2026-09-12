//! Independent verifier for [Public Generic Descriptor
//! v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#verification-and-trusted-replay)
//! (issue #152): the trust boundary between untrusted descriptor bytes and a
//! caller's own checked programme.
//!
//! The central rule this module enforces: **a submitted descriptor can name
//! what must be checked, but it can never supply the trusted facts used to
//! validate itself.** Every trusted input —the checked `ResolvedProgram`, the
//! source revision, the expected export identity, and the expected
//! programme-root digest— is supplied by the caller out of band. Nothing is
//! ever read back from `candidate_bytes` and treated as authoritative; the
//! candidate's own claims are compared against independently derived facts,
//! never substituted for them.
//!
//! This module is not [`super::producer`] run backwards. Calling
//! [`producer::generate_public_generic_descriptor`] a second time and diffing
//! its bytes against the candidate would only prove the producer is
//! deterministic — already covered by its own tests — and would silently
//! reproduce any bug in the producer's own final assembly step, since both
//! the "trusted" value and the check would come from identical code.
//! Instead, [`verify_public_generic_descriptor`] independently reconstructs
//! the wire value from the same *lower-level, already-tested* primitives the
//! producer is built from ([`CandidateSurface::derive`], the settlement
//! plan, and this module's own re-derivation of the two programme-subject
//! digests, matching the algorithm the specification document defines rather
//! than calling the producer's private helpers) and assembles its own
//! [`DescriptorV1`] with [`DescriptorV1::new`] directly. The real generator
//! is still invoked — once, on the same trusted facts — both because the
//! specification requires it and because its shape-admission predicate is
//! legitimately producer-owned logic that must not be duplicated; its output
//! is cross-checked against the independent reconstruction rather than
//! trusted as the sole basis of acceptance. See the module tests for a
//! concrete "self-consistently reminted" attack this buys: a descriptor
//! whose input and result bindings are swapped, using two genuinely,
//! correctly digested facts, in a way that a plausible producer assembly bug
//! could also produce.
//!
//! No fact this module derives is ever adopted as HIR, graph, cleanup, or
//! settlement authority: every value returned by [`generate_public_generic_descriptor`]
//! is either read back unchanged from the trusted `program` the caller
//! already checked, or is a fresh digest over that program's own persistent
//! declaration identities. This module performs no build, filesystem write,
//! registry access, or code execution, and grants no support or publication
//! claim.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::public_generic_abi::boundary_profile::MAX_DESCRIPTOR_WIRE_BYTES;
use crate::public_generic_abi::descriptor::producer::{self, GeneratedDescriptor};
use crate::public_generic_abi::descriptor::{self, DescriptorV1, InstanceBinding};
use crate::public_generic_abi::{digest, frame};
use crate::public_generic_settlement::SettlementPlan;
use crate::public_generic_surface::CandidateSurface;
use crate::public_generic_type::InstanceFacts;

/// Domain separation for the independently recomputed programme-root digest.
/// Must byte-for-byte match [`super::producer`]'s own private domain
/// constant for a correctly generated descriptor to verify; the two are
/// deliberately declared independently (never imported from one another) so
/// this module never inherits a mistaken domain the same way it never
/// inherits a mistaken algorithm. [Public Generic Descriptor
/// v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#verification-and-trusted-replay)
/// freezes the exact byte string.
const PROGRAM_ROOT_DOMAIN: &[u8] = b"semaprax.public-generic-descriptor.v1.program-root\0";
/// Domain separation for the independently recomputed source-projection
/// digest. See [`PROGRAM_ROOT_DOMAIN`].
const SOURCE_PROJECTION_DOMAIN: &[u8] =
    b"semaprax.public-generic-descriptor.v1.source-projection\0";

/// The candidate's own claimed export identity does not match the caller's
/// independently supplied expected export. Checked before any expensive
/// reconstruction work.
pub const EXPECTED_EXPORT_MISMATCH: &str = "SPX-PG710";
/// The caller's own expected programme-root digest does not match the
/// independently recomputed root of the trusted `program` it supplied: the
/// caller passed a programme that disagrees with its own stated expectation.
pub const EXPECTED_ROOT_NOT_ADMISSIBLE: &str = "SPX-PG711";
/// The candidate's embedded programme-root digest does not match the
/// trusted programme's independently recomputed root: a cross-paired
/// descriptor, rejected before the expensive trusted reconstruction below.
pub const CROSS_PAIRED_PROGRAM_ROOT: &str = "SPX-PG712";
/// The real generator's output disagrees with this module's independent
/// reconstruction of the same trusted facts. This is a producer-side defect
/// signal, never an attacker signal (the attacker does not control either
/// side of this comparison): defense in depth, not expected to be reachable
/// against a correct producer. See the differential test that pins today's
/// agreement.
pub const GENERATOR_DISAGREEMENT: &str = "SPX-PG713";

fn refusal(code: &'static str, subject: &str) -> Diagnostic {
    Diagnostic::io(
        code,
        format!(
            "{} verification refused: {subject}",
            descriptor::DESCRIPTOR_SCHEMA
        ),
    )
}

/// Controls beyond the mandatory checks. Every field narrows or records
/// intent; none can widen a frozen bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationOptions {
    /// A caller-chosen ceiling on `candidate_bytes.len()`, checked before any
    /// parsing. Never widens past [`MAX_DESCRIPTOR_WIRE_BYTES`]: a value
    /// above that frozen bound is silently clamped down to it, never treated
    /// as a request to accept a larger descriptor than the specification
    /// admits.
    pub max_descriptor_bytes: usize,
    /// `false` (default): the caller is asserting `program`/`source_revision`
    /// is its current head checked programme. `true`: the caller explicitly
    /// authorizes checking against a deliberately selected historical
    /// revision. This function itself has no retained-store access and
    /// cannot distinguish "current" from "historical" on its own — it
    /// simply records this flag on the returned
    /// [`VerifiedPublicGenericDescriptor`] for downstream audit and changes
    /// no check here. [`retained_store::verify_public_generic_descriptor_against_store`]
    /// is the layer that actually enforces currentness against a real
    /// store: see [Recovery and
    /// currentness](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#recovery-and-currentness).
    pub historical_mode: bool,
}

impl VerificationOptions {
    /// The default policy: the frozen byte bound, current-head only.
    pub fn current_head() -> Self {
        Self {
            max_descriptor_bytes: MAX_DESCRIPTOR_WIRE_BYTES,
            historical_mode: false,
        }
    }

    /// The frozen byte bound, with historical revisions explicitly
    /// authorized by the caller.
    pub fn historical() -> Self {
        Self {
            max_descriptor_bytes: MAX_DESCRIPTOR_WIRE_BYTES,
            historical_mode: true,
        }
    }
}

impl Default for VerificationOptions {
    fn default() -> Self {
        Self::current_head()
    }
}

/// Bounded, minimal selector fields read from otherwise-untrusted descriptor
/// bytes: the two-phase "look up a trusted subject by descriptor" workflow
/// [Public Generic Descriptor
/// v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#verification-and-trusted-replay)
/// allows. This type is deliberately much narrower than
/// [`VerifiedPublicGenericDescriptor`] — it exposes no instance facts, no
/// settlement plan, and no programme-subject digest — and there is
/// deliberately no `From<ParsedPublicGenericDescriptor> for
/// VerifiedPublicGenericDescriptor>` anywhere in this module: naming an
/// export is never authority to adopt it. A caller may use
/// [`claimed_export_id`](Self::claimed_export_id) only to select *which*
/// trusted subject to fetch before calling
/// [`verify_public_generic_descriptor`]; it must never adopt the selection
/// itself as authority that the export exists, is admitted, or matches any
/// particular programme.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedPublicGenericDescriptor {
    schema: String,
    claimed_export_id: String,
}

impl ParsedPublicGenericDescriptor {
    /// The descriptor schema literal the bytes declared.
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// The export identity the untrusted bytes *claim* to name. Not
    /// authority; see the type documentation.
    pub fn claimed_export_id(&self) -> &str {
        &self.claimed_export_id
    }
}

/// Phase-bounded selector parse: only enough of `bytes` is interpreted to
/// support a lookup-by-descriptor workflow. This still runs the full strict
/// [`descriptor::decode`] (the wire format has no cheaper prefix-only parse),
/// but narrows what is retained afterward, so a caller cannot accidentally
/// treat the richer parsed fields as if they had been verified.
pub fn parse_public_generic_descriptor_selectors(
    bytes: &[u8],
    options: &VerificationOptions,
) -> Result<ParsedPublicGenericDescriptor, Diagnostic> {
    let max_bytes = options.max_descriptor_bytes.min(MAX_DESCRIPTOR_WIRE_BYTES);
    if bytes.len() > max_bytes {
        return Err(refusal(
            descriptor::DESCRIPTOR_CAPACITY,
            "candidate exceeds the configured selector-parse byte bound",
        ));
    }
    let decoded = descriptor::decode(bytes)?;
    Ok(ParsedPublicGenericDescriptor {
        schema: descriptor::DESCRIPTOR_SCHEMA.to_owned(),
        claimed_export_id: decoded.export_id().to_owned(),
    })
}

/// One descriptor that has passed every verification phase against an
/// independently selected trusted subject. Constructible only by
/// [`verify_public_generic_descriptor`] in this module — every field is
/// private, and there is no public constructor, `Default`, or `From`
/// implementation from any parsed or deserialized value. A caller can only
/// ever obtain one by successfully running the full trusted-reconstruction
/// algorithm below.
#[derive(Clone, Debug, PartialEq)]
pub struct VerifiedPublicGenericDescriptor {
    accepted_bytes: Vec<u8>,
    descriptor: DescriptorV1,
    trusted_program_root_digest: String,
    trusted_source_projection_digest: String,
    trusted_public_surface_digest: String,
    input_facts: InstanceFacts,
    result_facts: InstanceFacts,
    settlement: SettlementPlan,
    historical_mode: bool,
}

impl VerifiedPublicGenericDescriptor {
    /// The exact accepted canonical wire bytes: `candidate_bytes`, unchanged.
    pub fn accepted_bytes(&self) -> &[u8] {
        &self.accepted_bytes
    }

    /// The accepted descriptor value.
    pub fn descriptor(&self) -> &DescriptorV1 {
        &self.descriptor
    }

    /// The accepted descriptor's identity digest.
    pub fn descriptor_digest(&self) -> String {
        self.descriptor.identity_digest()
    }

    /// The trusted subject's export identity (equal to the caller's
    /// independently supplied `expected_export_id`).
    pub fn export_id(&self) -> &str {
        self.descriptor.export_id()
    }

    /// The independently recomputed programme-root digest of the trusted
    /// `program` this value was verified against.
    pub fn program_root_digest(&self) -> &str {
        &self.trusted_program_root_digest
    }

    /// The independently recomputed source-projection digest of the trusted
    /// `source_revision` this value was verified against.
    pub fn source_projection_digest(&self) -> &str {
        &self.trusted_source_projection_digest
    }

    /// The independently recomputed public-surface digest ([`CandidateSurface::digest`])
    /// of the trusted subject.
    pub fn public_surface_digest(&self) -> &str {
        &self.trusted_public_surface_digest
    }

    /// The trusted, independently reconstructed owned-input instance facts.
    pub fn input_facts(&self) -> &InstanceFacts {
        &self.input_facts
    }

    /// The trusted, independently reconstructed owned-result instance facts.
    pub fn result_facts(&self) -> &InstanceFacts {
        &self.result_facts
    }

    /// The trusted settlement plan for the owned input parameter.
    pub fn settlement(&self) -> &SettlementPlan {
        &self.settlement
    }

    /// `true` when this value was accepted under
    /// [`VerificationOptions::historical_mode`]. Recorded for downstream
    /// audit; see that field's documentation for what it does and does not
    /// mean at this layer.
    pub fn historical_mode(&self) -> bool {
        self.historical_mode
    }

    #[allow(clippy::too_many_arguments)]
    fn seal(
        accepted_bytes: Vec<u8>,
        descriptor: DescriptorV1,
        trusted_program_root_digest: String,
        trusted_source_projection_digest: String,
        trusted_public_surface_digest: String,
        input_facts: InstanceFacts,
        result_facts: InstanceFacts,
        settlement: SettlementPlan,
        historical_mode: bool,
    ) -> Self {
        Self {
            accepted_bytes,
            descriptor,
            trusted_program_root_digest,
            trusted_source_projection_digest,
            trusted_public_surface_digest,
            input_facts,
            result_facts,
            settlement,
            historical_mode,
        }
    }
}

/// The sorted, deduplicated, length-framed set of every persistent
/// declaration identity in `program`, independently recomputed per [Public
/// Generic Descriptor
/// v1](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#derivation-from-checked-facts-the-producer)'s
/// documented algorithm. Deliberately not shared code with
/// [`super::producer`]'s own private `declaration_identity_preimage`: a bug
/// in the producer's collection (a forgotten category, a missing dedup, a
/// display name substituted for an identity) changes only one side of the
/// comparison this module performs, not both.
fn recompute_program_root_digest(program: &ResolvedProgram) -> String {
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
    digest(PROGRAM_ROOT_DOMAIN, &preimage)
}

/// Independently recompute the source-projection digest of a caller-supplied
/// canonical source revision. See [`recompute_program_root_digest`].
fn recompute_source_projection_digest(source_revision: &str) -> String {
    digest(SOURCE_PROJECTION_DOMAIN, source_revision.as_bytes())
}

/// Independently reconstruct the trusted `DescriptorV1` value for
/// `expected_export_id` in `program`/`source_revision`, using only
/// lower-level, already-tested primitives
/// ([`CandidateSurface::derive`](crate::public_generic_surface::CandidateSurface::derive))
/// plus this module's own digest recomputation — never
/// [`producer::generate_public_generic_descriptor`]'s own final assembly.
/// Callers of this private helper have already run the real generator once
/// (for its shape-admission refusal behavior, which stays producer-owned) so
/// the shape invariants it establishes (exactly one parameter, `own`
/// ownership, a concrete record instance in both positions) are known to
/// hold for the same deterministic inputs here.
fn reconstruct_trusted_descriptor(
    program: &ResolvedProgram,
    source_revision: &str,
    expected_export_id: &str,
) -> Result<(DescriptorV1, InstanceFacts, InstanceFacts, String), Diagnostic> {
    let surface = CandidateSurface::derive(program, &[expected_export_id.to_owned()])?;
    let entry = surface
        .entries()
        .get(expected_export_id)
        .expect("the real generator already selected exactly this export with these inputs");
    let input = entry
        .parameters
        .first()
        .expect("the real generator already required exactly one parameter");
    let input_facts = surface
        .instances()
        .get(&input.value.term)
        .cloned()
        .expect("the real generator already required the input instance facts to exist");
    let result_facts = surface
        .instances()
        .get(&entry.result.term)
        .cloned()
        .expect("the real generator already required the result instance facts to exist");

    let program_root_digest = recompute_program_root_digest(program);
    let source_projection_digest = recompute_source_projection_digest(source_revision);

    let reconstructed = DescriptorV1::new(
        expected_export_id,
        entry.name.clone(),
        program_root_digest,
        source_projection_digest,
        surface.digest().to_owned(),
        InstanceBinding::from_facts(&input_facts),
        InstanceBinding::from_facts(&result_facts),
    );
    Ok((
        reconstructed,
        input_facts,
        result_facts,
        surface.digest().to_owned(),
    ))
}

/// Verify `candidate_bytes` against a trusted subject the caller selects and
/// supplies independently of the candidate: `program`, `source_revision`,
/// `expected_export_id`, and `expected_program_root_digest`. None of these
/// four are read from `candidate_bytes`.
///
/// Phases, in the fixed order this function runs them (see the module
/// documentation and [Deterministic refusal
/// precedence](../../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#deterministic-refusal-precedence)
/// for why the order is pinned):
///
/// 1. Bound `candidate_bytes.len()` before any parsing.
/// 2. Strict structural parse ([`descriptor::decode`]): rejects malformed
///    framing, an unknown schema literal, an oversized field, and trailing
///    bytes.
/// 3. Caller-independent selection: the candidate's claimed export identity
///    must equal `expected_export_id`; the caller's own
///    `expected_program_root_digest` must equal the independently
///    recomputed root of `program`; the candidate's claimed programme root
///    must equal that same recomputed root — all before the expensive
///    reconstruction below.
/// 4. Trusted reconstruction: the real generator runs once, on
///    `program`/`source_revision`/`expected_export_id` only (never on
///    anything from `candidate_bytes`), for its shape-admission refusal
///    behavior; this module separately, independently reconstructs the same
///    wire value from lower-level primitives and requires the two to agree
///    (a disagreement is a producer-side defect signal, not an attacker
///    signal).
/// 5. Exact byte equality: [`descriptor::replay`] requires the candidate's
///    identity preimage to equal the independently reconstructed value's,
///    byte-for-byte, not merely digest-equal.
///
/// On success, returns a [`VerifiedPublicGenericDescriptor`] that cannot be
/// constructed any other way.
pub fn verify_public_generic_descriptor(
    program: &ResolvedProgram,
    source_revision: &str,
    expected_export_id: &str,
    expected_program_root_digest: &str,
    candidate_bytes: &[u8],
    options: &VerificationOptions,
) -> Result<VerifiedPublicGenericDescriptor, Diagnostic> {
    // Phase A: bound before any parsing.
    let max_bytes = options.max_descriptor_bytes.min(MAX_DESCRIPTOR_WIRE_BYTES);
    if candidate_bytes.len() > max_bytes {
        return Err(refusal(
            descriptor::DESCRIPTOR_CAPACITY,
            "candidate exceeds the configured verification byte bound",
        ));
    }

    // Phase B: strict structural parse, reusing the frozen codec exactly.
    let candidate = descriptor::decode(candidate_bytes)?;

    // Phase C: caller-independent trusted subject selection, cheapest
    // checks first.
    if candidate.export_id() != expected_export_id {
        return Err(refusal(
            EXPECTED_EXPORT_MISMATCH,
            "the candidate's export identity does not match the caller's independently \
             supplied expected export",
        ));
    }
    let recomputed_program_root_digest = recompute_program_root_digest(program);
    if recomputed_program_root_digest != expected_program_root_digest {
        return Err(refusal(
            EXPECTED_ROOT_NOT_ADMISSIBLE,
            "the caller's expected programme-root digest does not match the independently \
             recomputed root of the trusted programme it supplied",
        ));
    }

    // Phase D: trusted reconstruction. The real generator runs first, on
    // trusted facts only, both to satisfy the specification's requirement
    // that it be invoked and to reuse its shape-admission refusal behavior
    // rather than duplicating it.
    let generated: GeneratedDescriptor =
        producer::generate_public_generic_descriptor(program, source_revision, expected_export_id)?;

    let (independently_reconstructed, input_facts, result_facts, public_surface_digest) =
        reconstruct_trusted_descriptor(program, source_revision, expected_export_id)?;

    if independently_reconstructed.export_id() != candidate.export_id()
        || generated.descriptor().encode() != independently_reconstructed.encode()
    {
        return Err(refusal(
            GENERATOR_DISAGREEMENT,
            "the real generator's output disagrees with this module's independent \
             reconstruction of the same trusted facts",
        ));
    }

    // The candidate's own embedded programme root is checked only now,
    // because reading it requires this module's descendant-module access to
    // `DescriptorV1`'s private fields (there is deliberately no public
    // accessor, so as not to widen the frozen codec's surface); every
    // cheaper, publicly accessible check above already ran first.
    if candidate.program_root_digest != recomputed_program_root_digest {
        return Err(refusal(
            CROSS_PAIRED_PROGRAM_ROOT,
            "the candidate's programme-root digest does not match the trusted programme",
        ));
    }

    // Phase E: exact canonical bytes, not only digests, must match.
    descriptor::replay(candidate_bytes, &independently_reconstructed)?;

    Ok(VerifiedPublicGenericDescriptor::seal(
        candidate_bytes.to_vec(),
        candidate,
        recomputed_program_root_digest,
        recompute_source_projection_digest(source_revision),
        public_surface_digest,
        input_facts,
        result_facts,
        generated.settlement().clone(),
        options.historical_mode,
    ))
}

pub mod retained_store;

#[cfg(test)]
mod tests;
