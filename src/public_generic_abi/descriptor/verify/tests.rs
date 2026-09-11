//! Executable evidence for [`super`]: independence from the producer's own
//! output, phase-specific hostile refusals, and the "self-consistently
//! reminted" attack this module exists to catch. Every trusted programme
//! below is compiled through the real parser and resolver (`crate::parse` +
//! `crate::hir::resolve`), never a hand-built HIR fixture, matching
//! [`super::producer`]'s own test convention.

use std::path::Path;

use super::*;
use crate::hir::{self, ResolvedProgram};
use crate::parse;
use crate::public_generic_abi::descriptor::producer;

const REVISION: &str = "verify-test-revision-1";

/// `Pair<Leaf, i64>` in and out (`take`), `Pair<Leaf, i64>` in but `Leaf` out
/// (`split`) — one export whose input and result are the *same* instance and
/// one whose input and result are *different* instances, so a swapped-binding
/// attack actually changes bytes.
const BASE: &str = r#"
module test.public_generic_descriptor_verify;

@id("verify.leaf")
record Leaf {
    @id("verify.leaf.head")
    head: Bytes,
}

@id("verify.pair")
record Pair<T, U> {
    @id("verify.pair.left")
    left: T,
    @id("verify.pair.right")
    right: U,
}

@id("verify.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("verify.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload }, right: _count } => Leaf { head: payload },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every persistent identity, ownership mode, and structural shape exactly
/// matches [`BASE`], plus one extra harmless declaration — so its
/// `program_root_digest` differs from `BASE`'s while `verify.take` remains
/// byte-identical in shape.
const BASE_WITH_EXTRA_DECLARATION: &str = r#"
module test.public_generic_descriptor_verify;

@id("verify.leaf")
record Leaf {
    @id("verify.leaf.head")
    head: Bytes,
}

@id("verify.pair")
record Pair<T, U> {
    @id("verify.pair.left")
    left: T,
    @id("verify.pair.right")
    right: U,
}

@id("verify.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("verify.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload }, right: _count } => Leaf { head: payload },
    }
}

@id("verify.extra_scalar")
fn extra_scalar(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every persistent *top-level declaration* identity, function signature
/// shape, and ownership mode is exactly [`BASE`]'s: `program.types` and
/// `program.functions` name the identical set of declarations, since a
/// field is not itself a top-level declaration
/// ([`crate::hir::ResolvedProgram::types`] holds one entry per record, not
/// per field). The only change is `Leaf` gaining one additional field — a
/// genuine "changed source contract" that `program_root_digest`'s
/// declaration-identity inventory cannot see by construction (it walks only
/// top-level declaration ids, never field lists or field types; see the
/// producer module documentation's own "deliberately minimal real binding"
/// limitation). `Leaf` keeps its original `Bytes` leaf, so it is still a
/// resource type and the `own Pair<Leaf, i64>` parameter is still admitted
/// unchanged. This fixture exists to prove that limitation does not leave
/// the verifier able to accept a stale descriptor: the added field still
/// reaches `public_surface_digest` through `Leaf`'s own instance digest in
/// the reachable record closure, so it still fails closed.
const BASE_WITH_DRIFTED_NESTED_FIELD_TYPE: &str = r#"
module test.public_generic_descriptor_verify;

@id("verify.leaf")
record Leaf {
    @id("verify.leaf.head")
    head: Bytes,
    @id("verify.leaf.extra")
    extra: i64,
}

@id("verify.pair")
record Pair<T, U> {
    @id("verify.pair.left")
    left: T,
    @id("verify.pair.right")
    right: U,
}

@id("verify.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("verify.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload, extra: tag }, right: _count } =>
            Leaf { head: payload, extra: tag },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("verify.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn root_of(program: &ResolvedProgram) -> String {
    recompute_program_root_digest(program)
}

// ---------------------------------------------------------------------
// Positive
// ---------------------------------------------------------------------

#[test]
fn a_genuinely_generated_descriptor_verifies_against_its_own_trusted_programme() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);

    let verified = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();

    assert_eq!(verified.export_id(), "verify.take");
    assert_eq!(verified.program_root_digest(), root);
    assert_eq!(verified.accepted_bytes(), generated.wire_bytes());
    assert_eq!(verified.descriptor().encode(), generated.wire_bytes());
    assert_eq!(verified.input_facts(), generated.input_facts());
    assert_eq!(verified.result_facts(), generated.result_facts());
    assert!(!verified.historical_mode());
}

#[test]
fn different_shaped_export_also_verifies() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.split").unwrap();
    let root = root_of(&program);

    let verified = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.split",
        &root,
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();

    assert_eq!(verified.export_id(), "verify.split");
    assert_ne!(verified.input_facts().term, verified.result_facts().term);
}

#[test]
fn repeated_verification_is_stable() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);
    let options = VerificationOptions::default();

    let first = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        generated.wire_bytes(),
        &options,
    )
    .unwrap();
    let second = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        generated.wire_bytes(),
        &options,
    )
    .unwrap();

    assert_eq!(first, second);
}

#[test]
fn a_display_rename_still_verifies_but_the_accepted_bytes_carry_the_new_name() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let renamed = generated
        .descriptor()
        .clone()
        .with_export_name("renamed_take")
        .encode();
    let root = root_of(&program);

    let verified = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        &renamed,
        &VerificationOptions::default(),
    )
    .unwrap();

    assert_eq!(verified.descriptor().export_name(), "renamed_take");
    assert_eq!(verified.accepted_bytes(), renamed.as_slice());
}

#[test]
fn historical_mode_is_recorded_but_does_not_change_which_bytes_are_accepted() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);

    let verified = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        generated.wire_bytes(),
        &VerificationOptions::historical(),
    )
    .unwrap();

    assert!(verified.historical_mode());
}

// ---------------------------------------------------------------------
// Differential: today's producer and this module's independent
// reconstruction agree. This is the evidence backing the runtime
// `GENERATOR_DISAGREEMENT` cross-check: it pins that the check currently
// never fires against a correct producer, across more than one export shape.
// ---------------------------------------------------------------------

#[test]
fn independent_reconstruction_matches_the_real_generator_for_every_fixture_export() {
    let program = resolved(BASE);
    for export_id in ["verify.take", "verify.split"] {
        let generated =
            producer::generate_public_generic_descriptor(&program, REVISION, export_id).unwrap();
        let (reconstructed, ..) =
            reconstruct_trusted_descriptor(&program, REVISION, export_id).unwrap();
        assert_eq!(
            generated.descriptor().encode(),
            reconstructed.encode(),
            "producer and independent reconstruction disagree for {export_id}"
        );
    }
}

// ---------------------------------------------------------------------
// Phase A — bounds before any parsing
// ---------------------------------------------------------------------

#[test]
fn one_byte_over_the_frozen_bound_is_rejected_before_parsing() {
    let program = resolved(BASE);
    let root = root_of(&program);
    let oversized = vec![0u8; MAX_DESCRIPTOR_WIRE_BYTES + 1];

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        &oversized,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::DESCRIPTOR_CAPACITY);
}

#[test]
fn a_caller_narrowed_bound_is_honored_and_never_widened() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);
    let options = VerificationOptions {
        max_descriptor_bytes: generated.wire_bytes().len() - 1,
        ..VerificationOptions::default()
    };

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        generated.wire_bytes(),
        &options,
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::DESCRIPTOR_CAPACITY);

    // Attempting to widen past the frozen bound has no effect: the excess
    // over MAX_DESCRIPTOR_WIRE_BYTES is silently clamped away, not honored.
    let widened = VerificationOptions {
        max_descriptor_bytes: MAX_DESCRIPTOR_WIRE_BYTES * 4,
        ..VerificationOptions::default()
    };
    let oversized = vec![0u8; MAX_DESCRIPTOR_WIRE_BYTES + 1];
    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        &oversized,
        &widened,
    )
    .unwrap_err();
    assert_eq!(error.code, descriptor::DESCRIPTOR_CAPACITY);
}

// ---------------------------------------------------------------------
// Phase B — strict structural parse propagation (the codec's own hostile
// evidence already covers `decode` exhaustively; this proves this module
// propagates it rather than swallowing or reinterpreting it).
// ---------------------------------------------------------------------

#[test]
fn truncated_bytes_are_rejected_by_the_strict_parse() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);
    let truncated = &generated.wire_bytes()[..generated.wire_bytes().len() - 1];

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        truncated,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::MALFORMED_DESCRIPTOR);
}

#[test]
fn trailing_bytes_are_rejected_by_the_strict_parse() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);
    let mut padded = generated.wire_bytes().to_vec();
    padded.push(0);

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &root,
        &padded,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::MALFORMED_DESCRIPTOR);
}

// ---------------------------------------------------------------------
// Phase C — caller-independent trusted subject selection
// ---------------------------------------------------------------------

#[test]
fn the_candidates_own_claimed_export_must_match_the_callers_expectation() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let root = root_of(&program);

    // The candidate genuinely names `verify.take`, but the caller
    // independently expects `verify.split` — never read the export from the
    // candidate itself.
    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.split",
        &root,
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, EXPECTED_EXPORT_MISMATCH);
}

#[test]
fn the_callers_own_expected_root_must_match_the_programme_it_supplied() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, EXPECTED_ROOT_NOT_ADMISSIBLE);
}

#[test]
fn a_descriptor_generated_for_another_programme_cannot_cross_pair() {
    let program_a = resolved(BASE);
    let program_b = resolved(BASE_WITH_EXTRA_DECLARATION);
    let generated_for_a =
        producer::generate_public_generic_descriptor(&program_a, REVISION, "verify.take").unwrap();
    let root_b = root_of(&program_b);
    assert_ne!(
        root_of(&program_a),
        root_b,
        "the fixtures must have distinct roots"
    );

    let error = verify_public_generic_descriptor(
        &program_b,
        REVISION,
        "verify.take",
        &root_b,
        generated_for_a.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, CROSS_PAIRED_PROGRAM_ROOT);
}

// ---------------------------------------------------------------------
// Source drift with an unchanged declaration-identity set. This is the
// scenario `program_root_digest` cannot itself distinguish (documented as a
// known limitation of its "minimal real binding" in the producer module's
// own documentation): the exact same set of persistent declaration
// identities, but a nested record's field *type* has genuinely changed. A
// descriptor generated before the change must still fail closed when
// replayed against the changed program, because the change is still visible
// through `public_surface_digest` (which folds in every reachable record
// instance's own instance digest, not only the top-level input/result
// bindings' digests) even though `program_root_digest` alone would not have
// caught it.
// ---------------------------------------------------------------------

#[test]
fn a_nested_field_type_change_with_an_unchanged_declaration_set_still_fails_closed() {
    let before = resolved(BASE);
    let after = resolved(BASE_WITH_DRIFTED_NESTED_FIELD_TYPE);

    let root_before = root_of(&before);
    let root_after = root_of(&after);
    assert_eq!(
        root_before, root_after,
        "the drifted fixture must keep the identical declaration-identity set, \
         so program_root_digest alone cannot distinguish it"
    );

    let stale =
        producer::generate_public_generic_descriptor(&before, REVISION, "verify.take").unwrap();
    let current =
        producer::generate_public_generic_descriptor(&after, REVISION, "verify.take").unwrap();
    assert_ne!(
        stale.descriptor().public_surface_digest,
        current.descriptor().public_surface_digest,
        "the field-type change must actually move the public surface digest, \
         or this test would not be exercising real drift"
    );

    // Verifying the stale, pre-drift bytes against the drifted programme
    // (same export, same independently recomputed root) must fail closed at
    // the final exact-byte replay — never silently accept a descriptor whose
    // underlying record shape has since changed.
    let error = verify_public_generic_descriptor(
        &after,
        REVISION,
        "verify.take",
        &root_after,
        stale.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::DESCRIPTOR_REPLAY_MISMATCH);

    // The current, freshly generated bytes for the drifted programme still
    // verify normally: this is drift detection, not a spurious rejection of
    // an up-to-date descriptor.
    verify_public_generic_descriptor(
        &after,
        REVISION,
        "verify.take",
        &root_after,
        current.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();
}

// ---------------------------------------------------------------------
// Phase D — trusted reconstruction, shape refusals reused from the producer
// ---------------------------------------------------------------------

#[test]
fn an_export_outside_the_v1_shape_is_refused_reusing_the_producers_own_diagnostic() {
    let program = resolved(BASE);
    let root = root_of(&program);
    // `app.main` takes no parameters; the shape predicate is producer-owned
    // logic and must not be duplicated here, only reused.
    let placeholder = InstanceBinding {
        term: "i64".to_owned(),
        instance_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
    };
    let candidate = DescriptorV1::new(
        "app.main",
        "main",
        root.clone(),
        recompute_source_projection_digest(REVISION),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        placeholder.clone(),
        placeholder,
    )
    .encode();

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "app.main",
        &root,
        &candidate,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, producer::WRONG_PARAMETER_COUNT);
}

#[test]
fn an_unknown_export_is_refused_reusing_the_surfaces_own_diagnostic() {
    let program = resolved(BASE);
    let root = root_of(&program);
    let placeholder = InstanceBinding {
        term: "i64".to_owned(),
        instance_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
    };
    let candidate = DescriptorV1::new(
        "verify.nonexistent",
        "nonexistent",
        root.clone(),
        recompute_source_projection_digest(REVISION),
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        placeholder.clone(),
        placeholder,
    )
    .encode();

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.nonexistent",
        &root,
        &candidate,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, crate::public_generic_surface::INVALID_SELECTION);
}

// ---------------------------------------------------------------------
// The required deliverable: a self-consistently reminted semantic attack.
//
// Both instance bindings below are genuinely, correctly digested real
// facts from the same trusted programme — nothing is fabricated or
// checksum-patched. The only thing wrong is which position each one
// occupies: exactly the shape of a plausible producer assembly-order bug
// (swapping the `input`/`result` arguments to `DescriptorV1::new`), not
// random byte corruption. Because this module reconstructs the trusted
// value itself (input then result, per the frozen field order) instead of
// trusting the candidate's own arrangement, the swap is caught.
// ---------------------------------------------------------------------

#[test]
fn swapped_input_and_result_bindings_with_correctly_computed_digests_are_rejected() {
    let program = resolved(BASE);
    // `split` has a genuinely different input and result instance
    // (`Pair<Leaf, i64>` vs `Leaf`), so swapping them changes the bytes.
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.split").unwrap();
    assert_ne!(generated.input_facts().term, generated.result_facts().term);

    let root = root_of(&program);
    let swapped = DescriptorV1::new(
        "verify.split",
        generated.descriptor().export_name().to_owned(),
        root.clone(),
        recompute_source_projection_digest(REVISION),
        generated.descriptor().public_surface_digest.clone(),
        // Result's facts placed in the input slot, and vice versa: every
        // byte here is a real, correctly computed digest — just for the
        // wrong position.
        InstanceBinding::from_facts(generated.result_facts()),
        InstanceBinding::from_facts(generated.input_facts()),
    )
    .encode();

    // Sanity: the swap actually changed the wire bytes, so this is not a
    // vacuous test.
    assert_ne!(swapped, generated.wire_bytes());

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.split",
        &root,
        &swapped,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, descriptor::DESCRIPTOR_REPLAY_MISMATCH);
}

#[test]
fn a_program_root_digest_computed_over_the_wrong_declaration_set_is_rejected() {
    // A second plausible producer bug: correctly digesting a declaration
    // inventory that omits one real category (here, `function_instances` is
    // simulated by omitting `verify.split` from the id set entirely) while
    // every other field is the real, correctly digested value for
    // `verify.take`. This is caught by the same final byte-exact comparison,
    // because this module never reads `program_root_digest` off of the
    // candidate as authoritative — it always recomputes its own.
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();
    let real_root = root_of(&program);

    let mut wrong_ids: Vec<&str> = program
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .filter(|id| *id != "verify.split")
        .collect();
    wrong_ids.extend(
        program
            .types
            .iter()
            .map(|declaration| declaration.id.as_str()),
    );
    wrong_ids.sort_unstable();
    wrong_ids.dedup();
    let mut preimage = Vec::new();
    frame(&mut preimage, &(wrong_ids.len() as u64).to_le_bytes());
    for id in wrong_ids {
        frame(&mut preimage, id.as_bytes());
    }
    let wrong_root = digest(PROGRAM_ROOT_DOMAIN, &preimage);
    assert_ne!(
        wrong_root, real_root,
        "the mutation must actually change the digest"
    );

    let mutated = DescriptorV1::new(
        "verify.take",
        generated.descriptor().export_name().to_owned(),
        wrong_root,
        recompute_source_projection_digest(REVISION),
        generated.descriptor().public_surface_digest.clone(),
        InstanceBinding::from_facts(generated.input_facts()),
        InstanceBinding::from_facts(generated.result_facts()),
    )
    .encode();

    let error = verify_public_generic_descriptor(
        &program,
        REVISION,
        "verify.take",
        &real_root,
        &mutated,
        &VerificationOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.code, CROSS_PAIRED_PROGRAM_ROOT);
}

// ---------------------------------------------------------------------
// Two-phase lookup selectors
// ---------------------------------------------------------------------

#[test]
fn parsed_selectors_expose_only_a_claim_never_authority() {
    let program = resolved(BASE);
    let generated =
        producer::generate_public_generic_descriptor(&program, REVISION, "verify.take").unwrap();

    let parsed = parse_public_generic_descriptor_selectors(
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();

    assert_eq!(parsed.schema(), descriptor::DESCRIPTOR_SCHEMA);
    assert_eq!(parsed.claimed_export_id(), "verify.take");

    // The claim alone still requires the full trusted reconstruction to
    // become a `VerifiedPublicGenericDescriptor`; there is no shortcut.
    let root = root_of(&program);
    verify_public_generic_descriptor(
        &program,
        REVISION,
        parsed.claimed_export_id(),
        &root,
        generated.wire_bytes(),
        &VerificationOptions::default(),
    )
    .unwrap();
}

#[test]
fn selector_parsing_also_bounds_before_parsing() {
    let oversized = vec![0u8; MAX_DESCRIPTOR_WIRE_BYTES + 1];
    let error =
        parse_public_generic_descriptor_selectors(&oversized, &VerificationOptions::default())
            .unwrap_err();
    assert_eq!(error.code, descriptor::DESCRIPTOR_CAPACITY);
}
