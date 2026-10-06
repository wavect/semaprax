//! Unit tests for `change`-profile emission and independent replay against
//! source (issue #209).
//!
//! The distinction these tests draw is the whole point of the module: a
//! capsule can be perfectly *self*-consistent -- every digest recomputed,
//! every association sound, every structural check green -- and still
//! describe source that never existed. Each drift fixture below therefore
//! keeps the capsule structurally valid (so `verify_capsule` still accepts
//! it) and asserts that replay against source rejects it anyway, with the
//! exact `SPX-Z909` code.

use std::collections::BTreeMap;

use super::*;
use crate::audit_capsule::{
    check_object_bytes, nonclaims, parse_capsule, SignaturePolicyContext, TransparencyContext,
};

const FIXTURE_LABEL: &str = "audit_capsule_change_fixture.spx";

/// A minimal admitted module: one function with a persistent `@id` plus the
/// `fn main() -> i64` entry point every module needs
/// (`docs/AGENT-QUICK-REFERENCE.md`). That is enough to parse, verify,
/// format canonically, and project to a semantic graph -- every primitive
/// replay re-derives.
const FIXTURE_SOURCE: &str = r#"module audit.capsule.fixture;

@id("audit.capsule.fixture.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}

@id("app.main")
fn main() -> i64
{
    add(19, 23)
}
"#;

/// The same module with one operator changed: a real semantic change, so
/// every identity a capsule binds differs.
const DRIFTED_SOURCE: &str = r#"module audit.capsule.fixture;

@id("audit.capsule.fixture.add")
fn add(left: i64, right: i64) -> i64
{
    left - right
}

@id("app.main")
fn main() -> i64
{
    add(19, 23)
}
"#;

/// Parses cleanly but has no `fn main() -> i64`, so it fails verification.
const UNVERIFIABLE_SOURCE: &str = r#"module audit.capsule.fixture;

@id("audit.capsule.fixture.add")
fn add(left: i64, right: i64) -> i64
{
    left + right
}
"#;

const TRANSACTION_BYTES: &[u8] = b"semantic transaction canonical bytes for the change fixture";
const ASSURANCE_BYTES: &[u8] = b"assurance manifest canonical bytes for the change fixture";

fn empty_signature_ctx() -> SignaturePolicyContext {
    SignaturePolicyContext {
        verification_time_unix_seconds: 1_000,
        revoked_identities: Default::default(),
        required_roles: Vec::new(),
        identity_public_keys: Default::default(),
    }
}

fn empty_transparency_ctx() -> TransparencyContext {
    TransparencyContext {
        known_logs: Default::default(),
        minimum_accepted_checkpoint_size: 0,
    }
}

/// The two evidence objects only the change itself can supply -- its
/// semantic transaction and its assurance manifest. The emitter derives the
/// other two required object types from source.
fn supplied() -> (Vec<ObjectRef>, BTreeMap<String, Vec<u8>>) {
    let objects = vec![
        ObjectRef {
            id: "supplied-a-semantic-transaction".to_owned(),
            object_type: "semantic-transaction".to_owned(),
            schema: "semaprax.project-candidate-semantic-delta.v1".to_owned(),
            digest: sha256_digest(TRANSACTION_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
        ObjectRef {
            id: "supplied-b-assurance-manifest".to_owned(),
            object_type: "assurance-manifest".to_owned(),
            schema: "semaprax.assurance-manifest.v1".to_owned(),
            digest: sha256_digest(ASSURANCE_BYTES),
            redacted: false,
            redaction_reason: None,
            binds: BTreeMap::new(),
        },
    ];
    let mut bytes = BTreeMap::new();
    bytes.insert(
        "supplied-a-semantic-transaction".to_owned(),
        TRANSACTION_BYTES.to_vec(),
    );
    bytes.insert(
        "supplied-b-assurance-manifest".to_owned(),
        ASSURANCE_BYTES.to_vec(),
    );
    (objects, bytes)
}

fn emit_fixture(source: &str) -> ChangeCapsule {
    let (objects, bytes) = supplied();
    emit_change_capsule(source, FIXTURE_LABEL, &objects, &bytes, &[], &[])
        .expect("the fixture source parses, verifies, and projects to a graph")
}

// ---------------------------------------------------------------------
// Emission: re-derived identities, deterministic bytes, pinned golden.
// ---------------------------------------------------------------------

#[test]
fn an_emitted_capsule_binds_identities_independently_recomputed_from_the_source() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let parsed = parse_capsule(&capsule.manifest_bytes).expect("emitted bytes parse");

    // Recomputed here from scratch, sharing nothing with the emitter's own
    // bookkeeping beyond the same public primitives any third party has.
    let program = crate::parse(FIXTURE_SOURCE, FIXTURE_LABEL).expect("fixture parses");
    let graph_json = crate::graph::to_json(&program).expect("fixture projects to a graph");

    assert_eq!(
        parsed.subject.get("source_digest"),
        Some(&sha256_digest(FIXTURE_SOURCE.as_bytes()))
    );
    assert_eq!(
        parsed.subject.get("root_digest"),
        Some(&sha256_digest(graph_json.as_bytes()))
    );
    assert_eq!(
        parsed.subject.get("revision"),
        Some(&crate::graph::revision(&program))
    );
    assert_eq!(
        parsed.subject.get("compiler_version").map(String::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn emitting_the_same_change_twice_produces_byte_identical_capsules() {
    let first = emit_fixture(FIXTURE_SOURCE);
    let second = emit_fixture(FIXTURE_SOURCE);
    assert_eq!(first.manifest_bytes, second.manifest_bytes);
    assert_eq!(first.object_bytes, second.object_bytes);
}

#[test]
fn the_emitted_manifest_matches_its_pinned_canonical_golden() {
    // The golden pins the canonical *layout*: `serde_json`'s alphabetical
    // key order, the derived object ids and types, their bind sets, the
    // derived association edge, and the full nonclaims list. The identity
    // values and the graph schema are substituted out because they
    // legitimately change with the toolchain -- a literal for them would
    // pin the compiler version into this test rather than the format --
    // and each one is asserted against an independent recomputation in
    // `an_emitted_capsule_binds_identities_independently_recomputed_from_the_source`.
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let rendered = String::from_utf8(capsule.manifest_bytes.clone()).expect("manifest is UTF-8");
    let parsed = parse_capsule(&capsule.manifest_bytes).expect("emitted bytes parse");

    let mut normalized = rendered;
    for key in [
        "source_digest",
        "root_digest",
        "revision",
        "compiler_version",
    ] {
        let value = parsed.subject.get(key).expect("subject key is present");
        normalized = normalized.replace(value.as_str(), &format!("<{key}>"));
    }
    for object in &parsed.objects {
        // The program root's digest *is* the subject's `root_digest` and is
        // already substituted above; this pass names the rest.
        normalized = normalized.replace(object.digest.as_str(), &format!("<digest {}>", object.id));
    }
    let graph_schema = parsed
        .objects
        .iter()
        .find(|candidate| candidate.id == PROGRAM_ROOT_OBJECT_ID)
        .expect("the derived program root is present")
        .schema
        .clone();
    normalized = normalized.replace(graph_schema.as_str(), "<graph schema>");

    // This fixture's source is already in canonical form, so its canonical
    // projection hashes to the same value as the raw source bytes and both
    // normalize to `<source_digest>` below. That coincidence is a property
    // of the fixture, not of the format: a source file that is not already
    // canonical gives the projection object a digest of its own.
    assert_eq!(
        parsed.objects[0].digest, parsed.subject["source_digest"],
        "the fixture is expected to be canonical already"
    );

    const GOLDEN: &str = concat!(
        r#"{"associations":[{"from_id":"derived-b-program-root","relation":"derived_from","#,
        r#""to_id":"derived-a-source-projection"}],"#,
        r#""nonclaims":["evidence-is-not-authorization","local-evidence-only","#,
        r#""signatures-not-cryptographically-verified","#,
        r#""transparency-inclusion-not-independently-confirmed"],"#,
        r#""objects":[{"binds":{"source_digest":"<source_digest>"},"#,
        r#""digest":"<source_digest>","id":"derived-a-source-projection","#,
        r#""object_type":"source-projection","redacted":false,"redaction_reason":null,"#,
        r#""schema":"semaprax.canonical-source.v1"},"#,
        r#"{"binds":{"revision":"<revision>","root_digest":"<root_digest>"},"#,
        r#""digest":"<root_digest>","id":"derived-b-program-root","object_type":"program-root","#,
        r#""redacted":false,"redaction_reason":null,"schema":"<graph schema>"},"#,
        r#"{"binds":{},"digest":"<digest supplied-a-semantic-transaction>","#,
        r#""id":"supplied-a-semantic-transaction","object_type":"semantic-transaction","#,
        r#""redacted":false,"redaction_reason":null,"#,
        r#""schema":"semaprax.project-candidate-semantic-delta.v1"},"#,
        r#"{"binds":{},"digest":"<digest supplied-b-assurance-manifest>","#,
        r#""id":"supplied-b-assurance-manifest","object_type":"assurance-manifest","#,
        r#""redacted":false,"redaction_reason":null,"#,
        r#""schema":"semaprax.assurance-manifest.v1"}],"#,
        r#""profile":"change","schema":"semaprax.audit-capsule.v1","signatures":[],"#,
        r#""subject":{"compiler_version":"<compiler_version>","revision":"<revision>","#,
        r#""root_digest":"<root_digest>","source_digest":"<source_digest>"},"#,
        r#""transparency":null}"#,
        "\n",
    );
    assert_eq!(normalized, GOLDEN);
}

#[test]
fn an_emitted_capsule_declares_every_always_required_nonclaim() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let parsed = parse_capsule(&capsule.manifest_bytes).expect("emitted bytes parse");
    for required in nonclaims::ALWAYS_REQUIRED_NONCLAIMS {
        assert!(
            parsed.nonclaims.iter().any(|entry| entry == required),
            "emitted capsule omits `{required}`"
        );
    }
    // The emitter did re-derive its identities, so it must not claim
    // otherwise -- that nonclaim is reserved for hand-assembled capsules.
    assert!(!parsed
        .nonclaims
        .iter()
        .any(|entry| entry == "identities-not-replayed-against-source"));
}

#[test]
fn a_supplied_object_may_not_shadow_a_derived_one() {
    let (mut objects, mut bytes) = supplied();
    objects.push(ObjectRef {
        id: PROGRAM_ROOT_OBJECT_ID.to_owned(),
        object_type: "program-root".to_owned(),
        schema: "semaprax.graph.v40".to_owned(),
        digest: sha256_digest(b"a program root the producer would rather you believed"),
        redacted: false,
        redaction_reason: None,
        binds: BTreeMap::new(),
    });
    bytes.insert(PROGRAM_ROOT_OBJECT_ID.to_owned(), b"substitute".to_vec());
    let error =
        emit_change_capsule(FIXTURE_SOURCE, FIXTURE_LABEL, &objects, &bytes, &[], &[]).unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(error.message.contains("reserved"), "{}", error.message);
}

// ---------------------------------------------------------------------
// Replay: fails closed on drift.
// ---------------------------------------------------------------------

#[test]
fn an_emitted_capsule_replays_against_the_exact_source_it_was_emitted_from() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let report = verify_change_capsule_against_source(
        &capsule.manifest_bytes,
        &capsule.object_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .expect("a freshly emitted capsule replays against its own source");
    assert_eq!(report.profile, Profile::Change);
    assert_eq!(report.verified_object_ids.len(), 4);
    assert!(report.unavailable_claims.is_empty());
    // The nonclaims travel out of replay as data, not prose: a caller
    // rendering this report cannot show a green result without them.
    assert!(report
        .nonclaims
        .iter()
        .any(|entry| entry == "signatures-not-cryptographically-verified"));
}

#[test]
fn a_capsule_whose_source_drifted_is_refused_even_though_it_is_structurally_perfect() {
    let capsule = emit_fixture(FIXTURE_SOURCE);

    // Structurally the capsule is still flawless -- this is precisely the
    // gap replay exists to close.
    let parsed = parse_capsule(&capsule.manifest_bytes).expect("emitted bytes parse");
    check_object_bytes(&parsed, &capsule.object_bytes)
        .expect("every object still hashes to its recorded digest");

    let error = verify_change_capsule_against_source(
        &capsule.manifest_bytes,
        &capsule.object_bytes,
        DRIFTED_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(error.message.contains("source_digest"), "{}", error.message);
}

#[test]
fn a_capsule_emitted_by_a_different_compiler_is_refused_before_any_identity_is_compared() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let manifest = String::from_utf8(capsule.manifest_bytes).expect("manifest is UTF-8");
    let tampered = manifest.replace(
        &format!("\"compiler_version\":\"{}\"", env!("CARGO_PKG_VERSION")),
        "\"compiler_version\":\"0.0.0-not-this-toolchain\"",
    );
    assert_ne!(tampered, manifest, "the substitution must have applied");

    let error = verify_change_capsule_against_source(
        tampered.as_bytes(),
        &capsule.object_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(
        error.message.contains("0.0.0-not-this-toolchain"),
        "{}",
        error.message
    );
}

#[test]
fn a_source_projection_object_describing_a_different_program_is_refused_by_replay() {
    // The sharpest case: the producer swaps the retained canonical source
    // projection for a *different* program's text and updates the object's
    // digest to match, so every structural check still passes. Only
    // re-rendering the projection from the source under test catches it.
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let substitute = crate::format::canonical(
        &crate::parse(DRIFTED_SOURCE, FIXTURE_LABEL).expect("the drifted fixture parses"),
    );
    let manifest = String::from_utf8(capsule.manifest_bytes).expect("manifest is UTF-8");
    let honest_digest = sha256_digest(
        capsule
            .object_bytes
            .get(SOURCE_PROJECTION_OBJECT_ID)
            .expect("the derived projection is retained"),
    );
    // Rewrite only this object's own `digest` field. A blanket replace would
    // also rewrite the subject's `source_digest`, which happens to hold the
    // same value for this already-canonical fixture, and the capsule would
    // then be caught one check earlier -- by subject drift rather than by
    // the projection-bytes comparison this test exists to exercise.
    let tampered_manifest = manifest.replace(
        &format!("\"digest\":\"{honest_digest}\",\"id\":\"{SOURCE_PROJECTION_OBJECT_ID}\""),
        &format!(
            "\"digest\":\"{}\",\"id\":\"{SOURCE_PROJECTION_OBJECT_ID}\"",
            sha256_digest(substitute.as_bytes())
        ),
    );
    assert_ne!(
        tampered_manifest, manifest,
        "the substitution must have applied"
    );
    let mut tampered_bytes = capsule.object_bytes.clone();
    tampered_bytes.insert(
        SOURCE_PROJECTION_OBJECT_ID.to_owned(),
        substitute.into_bytes(),
    );

    // Structural verification is satisfied: the bytes hash to the recorded
    // digest, because the producer updated both together.
    let parsed = parse_capsule(tampered_manifest.as_bytes()).expect("tampered bytes still parse");
    check_object_bytes(&parsed, &tampered_bytes)
        .expect("the substituted object is internally consistent");

    let error = verify_change_capsule_against_source(
        tampered_manifest.as_bytes(),
        &tampered_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(
        error.message.contains("canonical source projection"),
        "{}",
        error.message
    );
}

#[test]
fn a_capsule_cannot_be_replayed_against_source_that_no_longer_parses() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let error = verify_change_capsule_against_source(
        &capsule.manifest_bytes,
        &capsule.object_bytes,
        "module audit.capsule.fixture; fn add(",
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(
        error.message.contains("does not parse"),
        "{}",
        error.message
    );
}

#[test]
fn emission_refuses_source_that_does_not_parse_rather_than_capsuling_it_anyway() {
    let (objects, bytes) = supplied();
    let error = emit_change_capsule(
        "module audit.capsule.fixture; fn add(",
        FIXTURE_LABEL,
        &objects,
        &bytes,
        &[],
        &[],
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(
        error.message.contains("does not parse"),
        "{}",
        error.message
    );
}

#[test]
fn emission_refuses_source_that_parses_but_no_longer_verifies() {
    // A capsule about source that does not compile would be evidence of
    // nothing, and is a distinct refusal from source that does not parse.
    let (objects, bytes) = supplied();
    let error = emit_change_capsule(
        UNVERIFIABLE_SOURCE,
        FIXTURE_LABEL,
        &objects,
        &bytes,
        &[],
        &[],
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(
        error.message.contains("does not pass verification"),
        "{}",
        error.message
    );
}

#[test]
fn a_non_change_capsule_is_refused_by_source_replay_rather_than_silently_skipped() {
    // Replay must never quietly pass a capsule it cannot actually check.
    // The `release` fixture below is structurally valid, so a permissive
    // implementation would return `Ok` having verified nothing about source.
    let artifact_bytes = b"release archive bytes for the fixture";
    let manifest = format!(
        r#"{{
  "schema": "semaprax.audit-capsule.v1",
  "profile": "release",
  "subject": {{
    "release_tag": "v1.2.3",
    "commit": "0123456789abcdef0123456789abcdef01234567",
    "artifact_digest": "{digest}"
  }},
  "objects": [
    {{"id": "obj-a-artifact", "object_type": "artifact", "schema": "semaprax.release-manifest.v1", "digest": "{digest}", "redacted": false, "redaction_reason": null, "binds": {{}}}},
    {{"id": "obj-b-package-manifest", "object_type": "package-manifest", "schema": "semaprax.package-manifest.v1", "digest": "{digest}", "redacted": false, "redaction_reason": null, "binds": {{}}}},
    {{"id": "obj-c-release-provenance", "object_type": "release-provenance", "schema": "semaprax.release-provenance.v1", "digest": "{digest}", "redacted": false, "redaction_reason": null, "binds": {{}}}},
    {{"id": "obj-d-release-signature-claim", "object_type": "release-signature-claim", "schema": "semaprax.release-signature-claim.v1", "digest": "{digest}", "redacted": false, "redaction_reason": null, "binds": {{}}}}
  ],
  "associations": [],
  "signatures": [],
  "transparency": null,
  "nonclaims": ["evidence-is-not-authorization", "local-evidence-only", "signatures-not-cryptographically-verified", "transparency-inclusion-not-independently-confirmed"]
}}"#,
        digest = sha256_digest(artifact_bytes),
    );
    let mut object_bytes = BTreeMap::new();
    for id in [
        "obj-a-artifact",
        "obj-b-package-manifest",
        "obj-c-release-provenance",
        "obj-d-release-signature-claim",
    ] {
        object_bytes.insert(id.to_owned(), artifact_bytes.to_vec());
    }

    let error = verify_change_capsule_against_source(
        manifest.as_bytes(),
        &object_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-Z909");
    assert!(error.message.contains("release"), "{}", error.message);
}

// ---------------------------------------------------------------------
// A capsule carries no authority.
// ---------------------------------------------------------------------

#[test]
fn replay_is_pure_and_repeatable_and_returns_only_a_report() {
    // Replaying twice yields the same report from the same inputs: there is
    // no spend-once semantics, no state consumed, and nothing performed. A
    // report is the only thing that comes back -- no handle, token, or
    // permission that could be presented anywhere as authorization to
    // publish, tag, deploy, or sign the change this capsule describes.
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let first = verify_change_capsule_against_source(
        &capsule.manifest_bytes,
        &capsule.object_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .expect("first replay succeeds");
    let second = verify_change_capsule_against_source(
        &capsule.manifest_bytes,
        &capsule.object_bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .expect("replaying again succeeds identically");
    assert_eq!(first.verified_object_ids, second.verified_object_ids);
    assert_eq!(first.nonclaims, second.nonclaims);
    assert_eq!(first.unavailable_claims, second.unavailable_claims);
}

#[test]
fn neither_emission_nor_replay_reaches_the_filesystem_process_table_or_network() {
    // A source-text guard rather than an inspection claim: if a future edit
    // adds a path read, a subprocess, or a socket to this module, this test
    // fails. `source_path_label` is a diagnostic string and must stay one --
    // replay taking a path would make it a route to reading files the
    // caller was never authorized to read.
    let module_source = include_str!("../change_replay.rs");
    for forbidden in [
        "std::fs",
        "std::process",
        "std::net",
        "Command::",
        "File::open",
    ] {
        assert!(
            !module_source.contains(forbidden),
            "`{forbidden}` appeared in change_replay.rs; emission and replay must stay pure"
        );
    }
}

// ---------------------------------------------------------------------
// Issue #583 (DV-23): replay binds the source-derived objects by their
// required *type*, never by the emitter's conventional ids. Each fixture is
// re-rendered through the public `render_capsule`, so it stays structurally
// valid and only the replay comparison can catch it.
// ---------------------------------------------------------------------

/// Re-renders `capsule` after renaming object ids (`renames`, applied to
/// objects, association edges, and the byte map) and then substituting the
/// retained bytes of the named objects (`substitutions`, by post-rename id),
/// recomputing each substituted object's digest so integrity still holds.
fn relabelled(
    capsule: &ChangeCapsule,
    renames: &[(&str, &str)],
    substitutions: &[(&str, &[u8])],
) -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let parsed = parse_capsule(&capsule.manifest_bytes).expect("emitted bytes parse");
    let rename = |id: &str| -> String {
        renames
            .iter()
            .find(|(from, _)| *from == id)
            .map_or_else(|| id.to_owned(), |(_, to)| (*to).to_owned())
    };
    let mut bytes: BTreeMap<String, Vec<u8>> = capsule
        .object_bytes
        .iter()
        .map(|(id, retained)| (rename(id), retained.clone()))
        .collect();
    let mut objects = parsed.objects.clone();
    for object in &mut objects {
        object.id = rename(&object.id);
        if let Some((_, substitute)) = substitutions.iter().find(|(id, _)| *id == object.id) {
            object.digest = sha256_digest(substitute);
            bytes.insert(object.id.clone(), substitute.to_vec());
        }
    }
    let associations: Vec<AssociationEdge> = parsed
        .associations
        .iter()
        .map(|edge| AssociationEdge {
            from_id: rename(&edge.from_id),
            relation: edge.relation.clone(),
            to_id: rename(&edge.to_id),
        })
        .collect();
    let manifest = render_capsule(
        Profile::Change,
        &parsed.subject,
        &objects,
        &associations,
        &parsed.signatures,
        parsed.transparency.as_ref(),
        &parsed.nonclaims,
    )
    .expect("relabelled capsule renders");
    (manifest, bytes)
}

fn replay(manifest: &[u8], bytes: &BTreeMap<String, Vec<u8>>) -> Result<(), Diagnostic> {
    verify_change_capsule_against_source(
        manifest,
        bytes,
        FIXTURE_SOURCE,
        FIXTURE_LABEL,
        &empty_signature_ctx(),
        &empty_transparency_ctx(),
    )
    .map(|_| ())
}

fn drifted_projection() -> String {
    crate::format::canonical(
        &crate::parse(DRIFTED_SOURCE, FIXTURE_LABEL).expect("the drifted fixture parses"),
    )
}

fn drifted_graph() -> String {
    derive_change_identities(DRIFTED_SOURCE, FIXTURE_LABEL)
        .expect("the drifted fixture derives identities")
        .graph_json
}

#[test]
fn honestly_renamed_source_derived_objects_still_replay() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let (manifest, bytes) = relabelled(
        &capsule,
        &[
            (SOURCE_PROJECTION_OBJECT_ID, "alternate-source-projection"),
            (PROGRAM_ROOT_OBJECT_ID, "alternate-program-root"),
        ],
        &[],
    );
    replay(&manifest, &bytes).expect("an honest capsule replays regardless of object labels");
}

#[test]
fn a_substituted_source_projection_fails_replay_under_any_object_id() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let substitute = drifted_projection();
    for label in [SOURCE_PROJECTION_OBJECT_ID, "alternate-source-projection"] {
        let (manifest, bytes) = relabelled(
            &capsule,
            &[(SOURCE_PROJECTION_OBJECT_ID, label)],
            &[(label, substitute.as_bytes())],
        );
        let error = replay(&manifest, &bytes).expect_err("substituted source must fail replay");
        assert_eq!(error.code, "SPX-Z909", "{label}: {}", error.message);
        assert!(
            error.message.contains("canonical source projection") && error.message.contains(label),
            "{label}: {}",
            error.message
        );
    }
}

#[test]
fn a_substituted_program_root_fails_replay_under_any_object_id() {
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let substitute = drifted_graph();
    for label in [PROGRAM_ROOT_OBJECT_ID, "alternate-program-root"] {
        let (manifest, bytes) = relabelled(
            &capsule,
            &[(PROGRAM_ROOT_OBJECT_ID, label)],
            &[(label, substitute.as_bytes())],
        );
        let error = replay(&manifest, &bytes).expect_err("substituted graph must fail replay");
        assert_eq!(error.code, "SPX-Z909", "{label}: {}", error.message);
        assert!(error.message.contains(label), "{label}: {}", error.message);
    }
}

#[test]
fn a_reserved_id_occupied_by_another_object_type_is_not_mistaken_for_the_source_projection() {
    // The honest source projection moves to an arbitrary id and an ordinary
    // semantic-transaction object takes the conventional reserved id. Replay
    // must still find and check the real projection by type (here: honest,
    // so it passes) and must not compare the transaction's bytes against
    // the canonical source.
    let capsule = emit_fixture(FIXTURE_SOURCE);
    let swapped = [
        (SOURCE_PROJECTION_OBJECT_ID, "alternate-source-projection"),
        (
            "supplied-a-semantic-transaction",
            SOURCE_PROJECTION_OBJECT_ID,
        ),
    ];
    let (manifest, bytes) = relabelled(&capsule, &swapped, &[]);
    replay(&manifest, &bytes).expect("type, not id, selects the replayed object");

    // And the same layout with a substituted projection still fails.
    let substitute = drifted_projection();
    let (manifest, bytes) = relabelled(
        &capsule,
        &swapped,
        &[("alternate-source-projection", substitute.as_bytes())],
    );
    let error = replay(&manifest, &bytes).expect_err("substituted source must fail replay");
    assert_eq!(error.code, "SPX-Z909");
}
