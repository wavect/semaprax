//! Executable evidence that [`generate_public_generic_descriptor`] derives a
//! `DescriptorV1` from a real compiled program rather than a hand-built
//! fixture: every source string below is compiled through the real parser
//! and resolver (`crate::parse` + `crate::hir::resolve`), exactly as the
//! [Public Generic Candidate Surface](crate::public_generic_surface) and
//! [Settlement](crate::public_generic_settlement) gates already do.

use std::path::Path;

use super::*;
use crate::hir::{self, ResolvedProgram};
use crate::parse;

/// One nested owned record closure: `Pair<Leaf, i64>` in and out, so a single
/// program exercises a `Bytes` leaf, a Copy-scalar field, and one level of
/// record nesting at once.
const BASE: &str = r#"
module test.public_generic_descriptor;

@id("descriptor.leaf")
record Leaf {
    @id("descriptor.leaf.head")
    head: Bytes,
}

@id("descriptor.pair")
record Pair<T, U> {
    @id("descriptor.pair.left")
    left: T,
    @id("descriptor.pair.right")
    right: U,
}

@id("descriptor.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("descriptor.scalar")
fn scalar(value: i64) -> i64 { value }

@id("descriptor.owned_bytes")
fn owned_bytes(value: own Bytes) -> Bytes { value }

@id("descriptor.borrowed")
fn borrowed(value: borrow Pair<Leaf, i64>) -> i64 {
    match borrow value {
        Pair { left: Leaf { head: _head }, right: count } => count,
    }
}

@id("descriptor.two_params")
fn two_params(value: own Pair<Leaf, i64>, extra: i64) -> Pair<Leaf, i64> { value }

@id("descriptor.no_owned_result")
fn no_owned_result(value: own Pair<Leaf, i64>) -> i64 {
    match own value {
        Pair { left: Leaf { head: _head }, right: count } => count,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Every display name changed; every persistent identity, declaration set,
/// ownership mode, and structural shape kept exactly the same as [`BASE`].
const RENAMED: &str = r#"
module test.public_generic_descriptor;

@id("descriptor.leaf")
record Node {
    @id("descriptor.leaf.head")
    contents: Bytes,
}

@id("descriptor.pair")
record Couple<A, B> {
    @id("descriptor.pair.left")
    first: A,
    @id("descriptor.pair.right")
    second: B,
}

@id("descriptor.take")
fn accept(subject: own Couple<Node, i64>) -> Couple<Node, i64> { subject }

@id("descriptor.scalar")
fn number(subject: i64) -> i64 { subject }

@id("descriptor.owned_bytes")
fn relay_bytes(subject: own Bytes) -> Bytes { subject }

@id("descriptor.borrowed")
fn peek(subject: borrow Couple<Node, i64>) -> i64 {
    match borrow subject {
        Couple { first: Node { contents: _contents }, second: tally } => tally,
    }
}

@id("descriptor.two_params")
fn duo(subject: own Couple<Node, i64>, more: i64) -> Couple<Node, i64> { subject }

@id("descriptor.no_owned_result")
fn unwrap_count(subject: own Couple<Node, i64>) -> i64 {
    match own subject {
        Couple { first: Node { contents: _contents }, second: tally } => tally,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// `Couple<Leaf, i64>` in, `Leaf` out: two different templates across input
/// and result.
const DIFFERENT_TEMPLATES: &str = r#"
module test.public_generic_descriptor;

@id("descriptor.leaf")
record Leaf {
    @id("descriptor.leaf.head")
    head: Bytes,
}

@id("descriptor.pair")
record Pair<T, U> {
    @id("descriptor.pair.left")
    left: T,
    @id("descriptor.pair.right")
    right: U,
}

@id("descriptor.split")
fn split(value: own Pair<Leaf, i64>) -> Leaf {
    match own value {
        Pair { left: Leaf { head: payload }, right: _count } => Leaf { head: payload },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Same template (`Pair`), different concrete arguments for input and
/// result: `Pair<Leaf, i64>` in, `Pair<Leaf, bool>` out.
const SAME_TEMPLATE_DIFFERENT_ARGUMENTS: &str = r#"
module test.public_generic_descriptor;

@id("descriptor.leaf")
record Leaf {
    @id("descriptor.leaf.head")
    head: Bytes,
}

@id("descriptor.pair")
record Pair<T, U> {
    @id("descriptor.pair.left")
    left: T,
    @id("descriptor.pair.right")
    right: U,
}

@id("descriptor.recount")
fn recount(value: own Pair<Leaf, i64>) -> Pair<Leaf, bool> {
    match own value {
        Pair { left: Leaf { head: payload }, right: count } =>
            Pair<Leaf, bool> { left: Leaf { head: payload }, right: count > 0 },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A generic function template: never a candidate export, per the frozen
/// boundary profile's export-shape rule 1.
const GENERIC_TEMPLATE: &str = r#"
module test.public_generic_descriptor;

@id("descriptor.leaf")
record Leaf {
    @id("descriptor.leaf.head")
    head: Bytes,
}

@id("descriptor.pair")
record Pair<T, U> {
    @id("descriptor.pair.left")
    left: T,
    @id("descriptor.pair.right")
    right: U,
}

@id("descriptor.identity")
fn identity<T>(value: own Pair<Leaf, T>) -> Pair<Leaf, T> { value }

@id("descriptor.use_identity")
fn use_identity(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { identity<i64>(value) }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("descriptor.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

const REVISION: &str = "test-source-revision";

#[test]
fn generates_a_descriptor_for_a_real_compiled_nested_record_export() {
    let program = resolved(BASE);
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.take").unwrap();

    assert_eq!(generated.descriptor().export_id(), "descriptor.take");
    assert_eq!(generated.descriptor().export_name(), "take");
    assert_eq!(
        generated.input_facts().term,
        "@15:descriptor.pair<@15:descriptor.leaf<>,i64>"
    );
    assert_eq!(generated.input_facts().term, generated.result_facts().term);
    assert_eq!(
        generated.input_facts().instance_digest,
        generated.result_facts().instance_digest
    );
    // The reachable closure carries both the pair instance and the nested
    // leaf instance, each keyed by its own canonical term.
    assert!(generated
        .record_closure()
        .contains_key("@15:descriptor.pair<@15:descriptor.leaf<>,i64>"));
    assert!(generated
        .record_closure()
        .contains_key("@15:descriptor.leaf<>"));
    assert_eq!(generated.input_facts().owned_leaves.len(), 1);

    // Settlement: one owned leaf, one whole-parameter transfer unit.
    assert_eq!(generated.settlement().obligations().len(), 1);
    assert_eq!(generated.settlement().export(), "descriptor.take");
}

/// Golden byte fixture: pinned wire bytes and digests for `descriptor.take`
/// derived from [`BASE`]. If this ever fails after an intentional change to
/// the descriptor derivation, regenerate the fixture with a fresh run and
/// review the diff — do not just paste in whatever the new run prints.
#[test]
fn golden_wire_bytes_and_digests_are_pinned() {
    let program = resolved(BASE);
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.take").unwrap();

    assert_eq!(
        generated.wire_bytes().len(),
        generated.descriptor().encode().len()
    );
    assert_eq!(generated.wire_bytes(), generated.descriptor().encode());

    // Known-answer digests: byte-exact identities that must not drift
    // silently. Compare against a second, independently produced run rather
    // than a literal so a genuine wire-format change is still caught without
    // this test itself becoming the second hand-maintained copy of the
    // preimage the specification already pins.
    let again = generate_public_generic_descriptor(&program, REVISION, "descriptor.take").unwrap();
    assert_eq!(generated.wire_bytes(), again.wire_bytes());
    assert_eq!(generated.descriptor_digest(), again.descriptor_digest());
    assert_eq!(
        generated.cleanup_inventory_digest(),
        again.cleanup_inventory_digest()
    );
    assert_eq!(generated.cleanup_plan_digest(), again.cleanup_plan_digest());
    assert_eq!(
        generated.settlement_obligations_digest(),
        again.settlement_obligations_digest()
    );

    // Fixed fixture: the exact byte length and a content hash of the wire
    // bytes, pinned once and never silently updated.
    assert_eq!(generated.wire_bytes().len(), 681);
    let content_digest = crate::public_generic_abi::digest(
        b"semaprax.public-generic-descriptor.v1.tests.golden\0",
        generated.wire_bytes(),
    );
    assert_eq!(
        content_digest,
        "sha256:dffe0449911fb7649c28e4f4987bff45436c90d5e7dd928f2fec49deae92c315"
    );
}

#[test]
fn repeated_generation_is_byte_identical() {
    let program = resolved(BASE);
    let first = generate_public_generic_descriptor(&program, REVISION, "descriptor.take").unwrap();
    let second = generate_public_generic_descriptor(&program, REVISION, "descriptor.take").unwrap();
    assert_eq!(first.wire_bytes(), second.wire_bytes());
    assert_eq!(first.descriptor_digest(), second.descriptor_digest());
}

/// A fresh parse-and-resolve of the identical source text stands in for
/// "trusted reload/recovery": nothing is read back from a previously emitted
/// artifact, so a second independent compilation must produce the same
/// bytes.
#[test]
fn generation_after_reparsing_the_same_source_is_byte_identical() {
    let first =
        generate_public_generic_descriptor(&resolved(BASE), REVISION, "descriptor.take").unwrap();
    let second =
        generate_public_generic_descriptor(&resolved(BASE), REVISION, "descriptor.take").unwrap();
    assert_eq!(first.wire_bytes(), second.wire_bytes());
}

/// A Windows-style (CRLF) checkout of the identical source must not change
/// the derived bytes: the checked facts the producer reads (types, ownership
/// modes, effects) do not depend on line-ending bytes.
#[test]
fn crlf_source_checkout_does_not_change_bytes() {
    let crlf_source = BASE.replace('\n', "\r\n");
    let unix =
        generate_public_generic_descriptor(&resolved(BASE), REVISION, "descriptor.take").unwrap();
    let windows =
        generate_public_generic_descriptor(&resolved(&crlf_source), REVISION, "descriptor.take")
            .unwrap();
    assert_eq!(unix.wire_bytes(), windows.wire_bytes());
    assert_eq!(unix.descriptor_digest(), windows.descriptor_digest());
}

/// Every display name changed, every persistent identity and shape kept:
/// the wire bytes differ (the trailing presentation field), but the identity
/// digest — and therefore the descriptor/facts digest a verifier binds to —
/// does not move.
#[test]
fn display_rename_changes_wire_bytes_but_not_identity_digest() {
    let base =
        generate_public_generic_descriptor(&resolved(BASE), REVISION, "descriptor.take").unwrap();
    let renamed =
        generate_public_generic_descriptor(&resolved(RENAMED), REVISION, "descriptor.take")
            .unwrap();

    assert_ne!(base.wire_bytes(), renamed.wire_bytes());
    assert_eq!(base.descriptor_digest(), renamed.descriptor_digest());
    assert_eq!(base.descriptor().export_name(), "take");
    assert_eq!(renamed.descriptor().export_name(), "accept");
    // Field/record display names moved too; the field-level digests inside
    // the instance facts are still identity-bearing, so this equality proves
    // a field rename alone does not move them either.
    assert_eq!(
        base.input_facts().instance_digest,
        renamed.input_facts().instance_digest
    );
}

#[test]
fn different_templates_for_input_and_result_are_bound_distinctly() {
    let program = resolved(DIFFERENT_TEMPLATES);
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.split").unwrap();
    assert_ne!(
        generated.input_facts().template.declaration,
        generated.result_facts().template.declaration
    );
    assert_eq!(generated.result_facts().term, "@15:descriptor.leaf<>");
}

#[test]
fn same_template_different_arguments_are_bound_distinctly() {
    let program = resolved(SAME_TEMPLATE_DIFFERENT_ARGUMENTS);
    let generated =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.recount").unwrap();
    assert_eq!(
        generated.input_facts().template.declaration,
        generated.result_facts().template.declaration
    );
    assert_ne!(generated.input_facts().term, generated.result_facts().term);
    assert_ne!(
        generated.input_facts().instance_digest,
        generated.result_facts().instance_digest
    );
}

#[test]
fn unknown_export_is_refused() {
    let program = resolved(BASE);
    let error =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.absent").unwrap_err();
    assert_eq!(error.code, crate::public_generic_surface::INVALID_SELECTION);
}

#[test]
fn selecting_a_generic_function_template_is_refused() {
    let program = resolved(GENERIC_TEMPLATE);
    let error =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.identity").unwrap_err();
    assert_eq!(error.code, crate::public_generic_surface::INVALID_SELECTION);
}

#[test]
fn wrong_parameter_count_is_refused() {
    let program = resolved(BASE);
    let error = generate_public_generic_descriptor(&program, REVISION, "descriptor.two_params")
        .unwrap_err();
    assert_eq!(error.code, WRONG_PARAMETER_COUNT);
}

#[test]
fn borrowed_top_level_parameter_is_refused_as_wrong_ownership() {
    let program = resolved(BASE);
    let error =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.borrowed").unwrap_err();
    assert_eq!(error.code, WRONG_OWNERSHIP_MODE);
}

#[test]
fn scalar_value_mode_parameter_is_refused_as_wrong_ownership() {
    let program = resolved(BASE);
    let error =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.scalar").unwrap_err();
    assert_eq!(error.code, WRONG_OWNERSHIP_MODE);
}

#[test]
fn owned_non_record_input_is_refused_as_unsupported_shape() {
    let program = resolved(BASE);
    let error = generate_public_generic_descriptor(&program, REVISION, "descriptor.owned_bytes")
        .unwrap_err();
    assert_eq!(error.code, UNSUPPORTED_INSTANCE_SHAPE);
}

#[test]
fn no_owned_aggregate_result_is_refused_as_unsupported_shape() {
    let program = resolved(BASE);
    let error =
        generate_public_generic_descriptor(&program, REVISION, "descriptor.no_owned_result")
            .unwrap_err();
    assert_eq!(error.code, UNSUPPORTED_INSTANCE_SHAPE);
}

/// A descriptor derived from one program must not replay as trusted against
/// a descriptor derived from a structurally different one, even though both
/// are real producer output rather than hand-built fixtures.
#[test]
fn cross_paired_generated_descriptors_fail_independent_replay() {
    let base =
        generate_public_generic_descriptor(&resolved(BASE), REVISION, "descriptor.take").unwrap();
    let different = generate_public_generic_descriptor(
        &resolved(DIFFERENT_TEMPLATES),
        REVISION,
        "descriptor.split",
    )
    .unwrap();

    let error =
        crate::public_generic_abi::descriptor::replay(different.wire_bytes(), base.descriptor())
            .unwrap_err();
    assert_eq!(
        error.code,
        crate::public_generic_abi::descriptor::DESCRIPTOR_REPLAY_MISMATCH
    );
}

/// A different source revision label for the identical checked program
/// changes the bound `program_root_digest` and `source_projection_digest`,
/// so a descriptor cannot be replayed as trusted across two different
/// retained-source contexts even when the checked types are unchanged.
#[test]
fn different_source_revision_changes_the_programme_subject_digests() {
    let program = resolved(BASE);
    let first =
        generate_public_generic_descriptor(&program, "revision-one", "descriptor.take").unwrap();
    let second =
        generate_public_generic_descriptor(&program, "revision-two", "descriptor.take").unwrap();
    assert_ne!(first.descriptor_digest(), second.descriptor_digest());
}

// ---------------------------------------------------------------------
// Field-count bound (issue #150 follow-up): this producer now enforces
// `MAX_FIELDS_PER_RECORD` by directly reusing the classifier's own
// `check_field_counts`, which nothing else this producer already calls
// (its own local shape predicate, or `CandidateSurface::derive`'s
// total-visited-node budget) enforces per-record. Mirrors
// `crate::public_generic_abi::classifier::tests::many_fields_source`.
// ---------------------------------------------------------------------

/// `field_count - 1` admitted `i64` Copy-scalar fields plus one direct owned
/// `Bytes` leaf, so the fixture always has exactly one owned leaf regardless
/// of `field_count`. `field_count` must be at least 1.
fn many_fields_source(field_count: usize) -> String {
    assert!(field_count >= 1);
    let mut fields = String::new();
    for index in 0..field_count - 1 {
        fields.push_str(&format!(
            "    @id(\"descriptor.big.f{index}\")\n    f{index}: i64,\n"
        ));
    }
    fields.push_str("    @id(\"descriptor.big.leaf\")\n    leaf: Bytes,\n");
    format!(
        "\nmodule test.public_generic_descriptor_fields;\n\n\
         @id(\"descriptor.big\")\nrecord Big {{\n{fields}}}\n\n\
         @id(\"descriptor.big_take\")\nfn big_take(value: own Big) -> Big {{ value }}\n\n\
         @id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
    )
}

/// Exact bound: `MAX_FIELDS_PER_RECORD` fields are admitted, exactly as the
/// classifier's own equivalent test asserts for `classify` directly.
#[test]
fn a_record_at_the_field_count_bound_is_admitted() {
    let source =
        many_fields_source(crate::public_generic_abi::boundary_profile::MAX_FIELDS_PER_RECORD);
    let generated =
        generate_public_generic_descriptor(&resolved(&source), REVISION, "descriptor.big_take")
            .unwrap();
    assert_eq!(
        generated.input_facts().fields.len(),
        crate::public_generic_abi::boundary_profile::MAX_FIELDS_PER_RECORD
    );
    assert_eq!(generated.input_facts().owned_leaves.len(), 1);
}

/// First-over-bound: one field more than `MAX_FIELDS_PER_RECORD` is refused
/// with the classifier's own `BOUND_EXCEEDED` (`SPX-PG613`), not a new
/// `SPX-PG7xx` code, since this bound is enforced by shared, not
/// re-derived, logic. Before this round's change this record would have
/// been silently admitted: `CandidateSurface::derive`'s own total-node
/// budget (4096) does not reject a single record of 257 fields.
#[test]
fn a_record_one_field_over_the_bound_is_refused() {
    let source =
        many_fields_source(crate::public_generic_abi::boundary_profile::MAX_FIELDS_PER_RECORD + 1);
    let error =
        generate_public_generic_descriptor(&resolved(&source), REVISION, "descriptor.big_take")
            .unwrap_err();
    assert_eq!(
        error.code,
        crate::public_generic_abi::classifier::BOUND_EXCEEDED
    );
}
