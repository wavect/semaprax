//! Compiled evidence for the nested-owned-record physical profile
//! `semaprax.authenticated-native-moves-nested.v1` (issue #292 / #288
//! follow-on): a record whose one field is itself an owned-Bytes-only record
//! instance -- `Outer<Leaf>` where `Leaf` has two `Bytes` fields -- executed
//! through the real generated C11 calling consumer for the exact same
//! admitted, two-level nested endpoint. `moves-v1` itself
//! ([`render_authenticated_moves_provider`]) stays exactly flat-only: this
//! module's own [`flat_moves_v1_refuses_the_nested_body_nested_moves_admits_it`]
//! is the regression that a silent widening of `moves-v1`'s own contract
//! would break.
use std::{fmt::Write as _, fs, path::Path, process::Command};

use semaprax::public_generic_abi::carrier::frame::{
    parse_bounded, CarrierFrameBinding, CarrierLeaf, LeafKind,
};
use semaprax::public_generic_abi::carrier::trace::Direction;
use semaprax::public_generic_abi::carrier::{
    CARRIER_REPLAY_MISMATCH, HANDLE_GENERATION_MISMATCH, MALFORMED_CARRIER,
};
use semaprax::public_generic_abi::compiler_endpoint::derive_admitted_public_generic_endpoint_v1;
use semaprax::public_generic_abi::native::admission::{
    NativeCarrierOwnership, NativeInputAdmission, NativeInputTicket,
};
use semaprax::public_generic_abi::native::authenticated::{
    render_authenticated_moves_provider, render_authenticated_nested_moves_provider,
    AuthenticatedNativeNestedMovesArtifact,
};
use semaprax::public_generic_consumer::c_calling::generate_authenticated_nested_moves_calling_consumer_v1;

const SOURCE: &str = r#"
module authenticated.nested;

@id("auth.nested.leaf")
record Leaf {
    @id("auth.nested.leaf.a")
    a: Bytes,
    @id("auth.nested.leaf.b")
    b: Bytes,
}

@id("auth.nested.outer")
record Outer<T> {
    @id("auth.nested.outer.payload")
    payload: T,
}

@id("auth.nested.transform")
fn transform(value: own Outer<Leaf>) -> Outer<Leaf>
    requires true
{
    let saved = value;
    Outer<Leaf> { payload: Leaf { a: saved.payload.b, b: saved.payload.a } }
}

@id("auth.nested.main")
fn main() -> i64 { 0 }
"#;
const EXPORT_ID: &str = "auth.nested.transform";

fn checked(source: &str) -> (semaprax::hir::ResolvedProgram, String) {
    let parsed = semaprax::check(source, Path::new("nested-moves.spx")).unwrap();
    let revision = semaprax::format::canonical(&parsed);
    let again = semaprax::check(&revision, Path::new("nested-moves.spx")).unwrap();
    assert_eq!(revision, semaprax::format::canonical(&again));
    (semaprax::hir::resolve(&parsed).unwrap(), revision)
}

/// `moves-v1` sees one record-typed `payload` field at the top level (never
/// a flat `Bytes` field) and refuses with `SPX-B103`, exactly the refusal
/// issue #292 reported against the component parity fixture's own
/// `provider.transform`. `moves-nested.v1` admits the identical checked body.
#[test]
fn flat_moves_v1_refuses_the_nested_body_nested_moves_admits_it() {
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, EXPORT_ID).unwrap();
    let descriptor = endpoint.descriptor();
    let flat_refusal = render_authenticated_moves_provider(&program, &revision, descriptor)
        .err()
        .unwrap();
    assert_eq!(flat_refusal.code, "SPX-B103");
    assert!(flat_refusal.message.contains("movement body"));

    let nested =
        render_authenticated_nested_moves_provider(&program, &revision, descriptor).unwrap();
    // Deterministic: the same checked HIR renders byte-identical source.
    assert_eq!(
        nested.source(),
        render_authenticated_nested_moves_provider(&program, &revision, descriptor)
            .unwrap()
            .source()
    );
    assert_eq!(descriptor.input_facts().owned_leaves.len(), 2);
}

fn field_macro(path: &str) -> String {
    let mut name = String::with_capacity(6 + path.len() * 2);
    name.push_str("field_");
    for byte in path.bytes() {
        write!(name, "{byte:02x}").unwrap();
    }
    name
}

/// The four generated field macro names, in `INPUT0, INPUT1, OUTPUT0,
/// OUTPUT1` order. Bundled into one array (rather than four parameters) so
/// [`compile_and_run`] stays under the ordinary argument-count bound.
type FieldMacros = [String; 4];

fn compile_and_run(
    root: &Path,
    label: &str,
    provider_source: &str,
    consumer_files: &[(String, String)],
    fields: &FieldMacros,
) -> (bool, String) {
    let [input0, input1, output0, output1] = fields;
    let directory = root.join(label);
    fs::create_dir(&directory).unwrap();
    for (name, contents) in consumer_files {
        fs::write(directory.join(name), contents).unwrap();
    }
    fs::write(directory.join("provider.c"), provider_source).unwrap();
    let driver = format!(
        "#define INPUT0 {input0}\n#define INPUT1 {input1}\n#define OUTPUT0 {output0}\n#define OUTPUT1 {output1}\n{}",
        include_str!("checked_nested_moves_driver.c"),
    );
    fs::write(directory.join("driver.c"), driver).unwrap();

    let mut ok = true;
    let mut last_stdout = String::new();
    for opt in ["-O0", "-O2"] {
        let exe = directory.join(format!("probe{opt}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", opt, "-Wall", "-Wextra", "-Werror"])
            .arg(directory.join("provider.c"))
            .arg(directory.join("driver.c"))
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{label} {opt} compile: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&exe).arg("0107").arg("0b0d").output().unwrap();
        ok &= output.status.success();
        last_stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    }
    (ok, last_stdout)
}

fn provider_source(artifact: &AuthenticatedNativeNestedMovesArtifact) -> String {
    format!(
        "{}\n{}\n",
        include_str!("../allocations.c"),
        artifact.source()
    )
}

/// Real, compiled (`-O0`/`-O2`) execution of the checked nested-record swap
/// body through the generated C11 calling consumer, and of the same body's
/// `requires false` sibling reporting the checked contract failure -- both
/// against the SAME two-level nested `Outer<Leaf>` shape, never a flattened
/// stand-in.
#[test]
fn generated_c_executes_the_checked_nested_movement_body() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-r292-nested-moves-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();

    for guard in [true, false] {
        let source = if guard {
            SOURCE.to_owned()
        } else {
            assert_eq!(SOURCE.matches("requires true").count(), 1);
            SOURCE.replacen("requires true", "requires false", 1)
        };
        let (program, revision) = checked(&source);
        let endpoint =
            derive_admitted_public_generic_endpoint_v1(&program, &revision, EXPORT_ID).unwrap();
        let descriptor = endpoint.descriptor();
        let artifact =
            render_authenticated_nested_moves_provider(&program, &revision, descriptor).unwrap();
        let consumer =
            generate_authenticated_nested_moves_calling_consumer_v1(descriptor, &artifact).unwrap();

        let input = &descriptor.input_facts().owned_leaves;
        let result = &descriptor.result_facts().owned_leaves;
        assert_eq!(input.len(), 2);
        assert_eq!(result.len(), 2);
        let fields: FieldMacros = [
            field_macro(&input[0]),
            field_macro(&input[1]),
            field_macro(&result[0]),
            field_macro(&result[1]),
        ];

        let (ok, stdout) = compile_and_run(
            &root,
            &format!("guard-{guard}"),
            &provider_source(&artifact),
            consumer.files(),
            &fields,
        );
        assert!(ok, "guard={guard}: {stdout}");
        if guard {
            // `a: right, b: left` after the checked swap: left="0107", right="0b0d".
            assert_eq!(
                stdout, "OK 0b0d 0107",
                "swap must be observable, not identity"
            );
        } else {
            assert_eq!(stdout, "CONTRACT_VIOLATION");
        }
    }
    fs::remove_dir_all(&root).unwrap();
}

/// The nested leaf-path check covers every leaf, not only a top-level one:
/// substituting either the outer-then-inner path of the SECOND (deeper, same
/// depth) leaf is refused before any provider work, exactly like the
/// existing flat and one-level-nested hostile corpora
/// ([`crate::native_frame_admission`], issue #173) already prove generically.
/// This is the same [`CarrierFrameBinding::validate_frame`] check the
/// authenticated native C entry point (`authenticated_prepare.c`) embeds
/// verbatim; nothing new was added to it for nesting; this test exists to
/// show it still covers this exact two-level shape.
#[test]
fn substituted_nested_leaf_path_is_refused_before_provider_work() {
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, EXPORT_ID).unwrap();
    let descriptor = endpoint.descriptor();
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    assert_eq!(plan.leaf_paths().len(), 2);
    for hostile_index in [0usize, 1] {
        let mut leaves: Vec<_> = plan
            .leaf_paths()
            .iter()
            .map(|path| CarrierLeaf::new(path.clone(), LeafKind::Bytes, vec![1, 2, 3]))
            .collect();
        leaves[hostile_index] = CarrierLeaf::new(
            "@30:forged.nested.leaf.path.not.in.the.descriptor",
            LeafKind::Bytes,
            vec![1, 2, 3],
        );
        let bytes = plan.frame_with_leaves(leaves).encode();
        let frame = parse_bounded(&bytes).unwrap();
        let error = plan.validate_frame(&frame).unwrap_err();
        assert_eq!(
            error.code, "SPX-PG803",
            "hostile leaf index {hostile_index}"
        );
    }
}

/// Cross-endpoint replay: a nested-moves artifact rendered for one checked
/// endpoint is refused by the consumer generator when handed a *different*
/// verified descriptor, even one admitting the same nested shape.
#[test]
fn consumer_generation_refuses_a_foreign_descriptor() {
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, EXPORT_ID).unwrap();
    let artifact =
        render_authenticated_nested_moves_provider(&program, &revision, endpoint.descriptor())
            .unwrap();

    let other_source = SOURCE.replacen("requires true", "requires false", 1);
    let (other_program, other_revision) = checked(&other_source);
    let other_endpoint =
        derive_admitted_public_generic_endpoint_v1(&other_program, &other_revision, EXPORT_ID)
            .unwrap();
    let error = generate_authenticated_nested_moves_calling_consumer_v1(
        other_endpoint.descriptor(),
        &artifact,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-PG803");
}

/// The nested-moves.v1 authenticated handoff's own admission guard
/// (`NativeInputAdmission`, embedded verbatim in the physical C entry point
/// `authenticated_prepare.c` -- see `docs/PUBLIC-GENERIC-CARRIER-V1.md`)
/// refuses a leaf-kind-tag mismatch on either nested leaf, a substituted
/// cleanup-plan digest, and a stale, future, or zero generation, each before
/// any installer/provider effect -- over this module's own two-level nested
/// `Outer<Leaf>` descriptor. Nothing in `NativeInputAdmission` changed for
/// nesting (see `crate::native_frame_admission`'s pre-existing
/// one-level-nested `Box<Leaf>` corpus, issue #173); this is the same
/// generic check, exercised again against a genuinely two-level shape and
/// the specific profile this module adds.
#[test]
fn nested_authenticated_handoff_refuses_the_hostile_corpus_before_provider_work() {
    let (program, revision) = checked(SOURCE);
    let endpoint =
        derive_admitted_public_generic_endpoint_v1(&program, &revision, EXPORT_ID).unwrap();
    let descriptor = endpoint.descriptor();

    // Zero generation is refused at construction time, before any ticket
    // (and so before any admission attempt) can even exist.
    let zero_generation = NativeInputAdmission::from_verified_descriptor(descriptor, 0)
        .err()
        .unwrap();
    assert_eq!(zero_generation.code, MALFORMED_CARRIER);

    let admission = NativeInputAdmission::from_verified_descriptor(descriptor, 41).unwrap();
    assert_eq!(admission.binding().leaf_paths().len(), 2);
    let leaves: Vec<CarrierLeaf> = admission
        .binding()
        .leaf_paths()
        .iter()
        .enumerate()
        .map(|(index, path)| {
            CarrierLeaf::new(path.clone(), LeafKind::Bytes, vec![index as u8, 0xaa])
        })
        .collect();
    let valid_frame = admission
        .binding()
        .frame_with_leaves(leaves.clone())
        .encode();
    let valid_ticket = || {
        NativeInputTicket::new(
            admission.generation(),
            NativeCarrierOwnership::Caller,
            admission.cleanup_plan_digest(),
            valid_frame.clone(),
        )
    };

    // Control: the unmodified ticket admits and exposes the bound leaves.
    assert_eq!(admission.admit(&valid_ticket()).unwrap(), leaves);

    // Stale and future generation.
    for generation in [admission.generation() - 1, admission.generation() + 1] {
        let ticket = NativeInputTicket::new(
            generation,
            NativeCarrierOwnership::Caller,
            admission.cleanup_plan_digest(),
            valid_frame.clone(),
        );
        let error = admission.admit(&ticket).unwrap_err();
        assert_eq!(
            error.code, HANDLE_GENERATION_MISMATCH,
            "generation {generation}"
        );
    }

    // Substituted cleanup-plan digest.
    let substituted_cleanup = NativeInputTicket::new(
        admission.generation(),
        NativeCarrierOwnership::Caller,
        format!("{}-substituted", admission.cleanup_plan_digest()),
        valid_frame.clone(),
    );
    let error = admission.admit(&substituted_cleanup).unwrap_err();
    assert_eq!(error.code, CARRIER_REPLAY_MISMATCH);

    // Leaf-kind-tag mismatch, tried against each of the two nested leaves in
    // turn: the encoded frame's byte immediately after a leaf's path is its
    // canonical kind tag (0 for the grammar's only admitted `LeafKind`,
    // `Bytes`); flipping it to an unknown tag is refused by the shared codec
    // itself (`parse_bounded`, inside `admission.admit`), before
    // `CarrierFrameBinding::validate_frame` is ever reached.
    for leaf_index in 0..2 {
        let path = admission.binding().leaf_paths()[leaf_index].as_bytes();
        let tag_offset = valid_frame
            .windows(path.len())
            .position(|window| window == path)
            .unwrap()
            + path.len();
        assert_eq!(
            valid_frame[tag_offset], 0,
            "leaf {leaf_index}: the valid Bytes leaf uses tag zero"
        );
        let mut unknown_tag_frame = valid_frame.clone();
        unknown_tag_frame[tag_offset] = 1;
        let ticket = NativeInputTicket::new(
            admission.generation(),
            NativeCarrierOwnership::Caller,
            admission.cleanup_plan_digest(),
            unknown_tag_frame,
        );
        let error = admission.admit(&ticket).unwrap_err();
        assert_eq!(
            error.code, MALFORMED_CARRIER,
            "leaf {leaf_index} tag mismatch"
        );
    }
}
