//! Candidate-bound public generic surface delta: gate PG-4 of the public
//! generic ownership milestone.
//!
//! Three fixtures, because the gate has to hold in three different shapes.
//!
//! `GenericFixture` is the real GEN-05 project: its internal closure owns a
//! `Pair<Bytes, bool>` while its only export keeps the already-admitted
//! `fn(borrow Slice<u8>) -> i64` signature. That project is the separation
//! invariant in delta form — the route must produce a complete, valid report
//! in which no template, instance term, or record identity appears at all.
//!
//! `ScalarFixture` exports `fn(i64, i64) -> i64`, which the grammar can spell
//! end to end, so it is where a described surface, a real surface digest, and
//! the rename non-inference can be asserted against something rather than
//! against two absences.
//!
//! `RecordFixture` exports `fn(i64) -> Packet`, the only shape admitted today
//! that puts a record instance into a public signature. It is where ordered
//! arguments, substituted fields, owned leaves, and a breaking field change
//! are actually exercised.

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, SemanticChange,
    MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "spx-public-generic-delta-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

struct Fixture(PathBuf);

impl Fixture {
    /// The shared GEN-05 project: a concrete generic record owned inside the
    /// body, an export whose signature never mentions it.
    fn generic() -> Self {
        let root = root("generic");
        crate::concrete_generic_record_product::write_project(&root);
        Self(root)
    }

    /// A Project v1 scalar export the grammar can spell in every position.
    fn scalar() -> Self {
        let root = root("scalar");
        std::fs::create_dir_all(root.join("src")).unwrap();
        Self::write(
            &root,
            r#"schema = "semaprax.project.v1"
name = "public-generic-delta-scalar"
entry = "pg.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["pg.add"]
tests = ["pg.tests"]
"#,
            &[
                (
                    "src/app.spx",
                    "module pg.app; use function @id(\"pg.add\") from pg.core as add; @id(\"pg.main\") fn main()->i64 {add(20,22)}",
                ),
                (
                    "src/core.spx",
                    "module pg.core; @id(\"pg.add\") fn add(left:i64,right:i64)->i64 {left+right}",
                ),
                (
                    "src/tests.spx",
                    "module pg.tests; use function @id(\"pg.add\") from pg.core as add; @id(\"pg.tests.main\") fn main()->i64 {if add(20,22)==42 {0} else {1}}",
                ),
            ],
        );
        Self(root)
    }

    /// A Project v9 flat owned-record export: `fn(i64) -> Packet`. Scalar
    /// parameters and a record result are both admitted, so this is the one
    /// shape available today that carries a record instance into a described
    /// public generic surface.
    fn record() -> Self {
        let root = root("record");
        std::fs::create_dir_all(root.join("src")).unwrap();
        Self::write(
            &root,
            r#"schema = "semaprax.project.v9"
name = "public-generic-delta-record"
version = "1.0.0"
profile = "flat-owned-record-api.v1"
entry = "pgr.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["pgr.make"]
tests = ["pgr.tests"]
"#,
            &[
                (
                    "src/core.spx",
                    r#"module pgr.core;
@id("pgr.packet") record Packet { @id("pgr.packet.payload") payload:Bytes, @id("pgr.packet.value") value:i64, }
@id("pgr.make") fn make(value:i64)->Packet {let seed = [7u8, 9u8]; Packet {payload:bytes_copy(array_as_slice(seed)),value:value}}
"#,
                ),
                (
                    "src/app.spx",
                    "module pgr.app; @id(\"pgr.main\") fn main()->i64 {0}",
                ),
                (
                    "src/tests.spx",
                    "module pgr.tests; @id(\"pgr.tests.main\") fn main()->i64 {0}",
                ),
            ],
        );
        Self(root)
    }

    fn write(root: &std::path::Path, manifest: &str, sources: &[(&str, &str)]) {
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        for (path, source) in sources {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
    }

    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn apply(base: &ProjectCandidate, intent: Value) -> ProjectCandidate {
    base.apply(
        base.candidate_digest(),
        &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
    )
    .unwrap()
}

fn delta(candidate: &ProjectCandidate) -> (String, Value) {
    let bytes = candidate
        .public_generic_delta(candidate.candidate_digest())
        .unwrap();
    let value = serde_json::from_str(&bytes).unwrap();
    (bytes, value)
}

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let errors = result.err().expect("hostile public generic delta accepted");
    assert!(
        errors.iter().any(|error| error.code == expected),
        "{errors:?}"
    );
}

/// A byte-different, JSON-equal re-serialization: the same members, emitted in
/// reverse key order instead of the canonical byte order.
fn reordered(bytes: &str) -> String {
    let value: Value = serde_json::from_str(bytes).unwrap();
    let object = value.as_object().unwrap();
    let mut out = String::from("{");
    for (index, (key, item)) in object.iter().rev().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(key).unwrap());
        out.push(':');
        out.push_str(&serde_json::to_string(item).unwrap());
    }
    out.push_str("}\n");
    assert_ne!(out, bytes);
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap(),
        serde_json::from_str::<Value>(bytes).unwrap()
    );
    out
}

/// An unchanged candidate describes one surface on each side, and the two are
/// the same surface: equal digests, an `unchanged` verdict, no finding. The
/// same computation repeated is byte-identical, and independent replay accepts
/// exactly those bytes.
#[test]
fn an_unchanged_candidate_describes_two_equal_surfaces_and_replays_byte_exactly() {
    let fixture = Fixture::scalar();
    let candidate = fixture.candidate();
    let (bytes, value) = delta(&candidate);
    assert_eq!(
        value["schema"],
        "semaprax.project-candidate-public-generic-delta.v1"
    );
    assert_eq!(value["candidate_digest"], candidate.candidate_digest());
    assert_eq!(
        value["base_project_revision"],
        candidate.base_revision().project_revision()
    );
    assert_eq!(
        value["project_revision"],
        candidate.revision().project_revision()
    );
    for (field, expected) in [
        ("support", "not_assessed"),
        ("publication", "not_assessed"),
        ("runtime", "not_observed"),
        ("semantic_version_decision", "not_inferred"),
    ] {
        assert_eq!(value[field], expected, "{field}");
    }
    assert!(value["compatibility_authority"]
        .as_str()
        .unwrap()
        .contains("compatibility_support_and_publication_remain_with"));
    assert!(value["admission"]
        .as_str()
        .unwrap()
        .starts_with("candidate_description_only_no_public_generic_signature_is_admitted"));
    assert_eq!(
        value["selection_basis"],
        "exact_manifest_web_exports_and_command_by_stable_identity_never_display_name"
    );
    assert!(value["facts_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let comparison = &value["facts"]["comparison"];
    assert_eq!(comparison["verdict"], "unchanged");
    assert!(comparison["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        comparison["basis"],
        "public_generic_compatibility_v1_over_both_described_surfaces"
    );
    let base_digest = comparison["base_surface_digest"].as_str().unwrap();
    assert_eq!(comparison["candidate_surface_digest"], base_digest);
    assert!(base_digest.starts_with("sha256:"));
    assert_eq!(
        comparison["public_generic_compatibility_v1"]["schema"],
        "semaprax.public-generic-compatibility.v1"
    );
    assert_eq!(
        comparison["public_generic_compatibility_v1"]["semantic_version_decision"],
        "not_inferred"
    );

    // The described surface is the export's exact grammar-spelled signature.
    let side = &value["facts"]["base"];
    assert_eq!(side["described"], json!(["pg.add"]));
    assert!(side["excluded"].as_array().unwrap().is_empty());
    assert_eq!(side["described_instances"], 0);
    let entry = &side["surface"]["entries"]["pg.add"];
    assert_eq!(entry["result"]["term"], "i64");
    assert_eq!(entry["result"]["kind"], "data");
    let parameters = entry["parameters"].as_array().unwrap();
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0]["type"]["term"], "i64");
    assert_eq!(parameters[0]["ownership"], "value");

    // Determinism, then byte-exact independent replay from the retained base.
    assert_eq!(
        candidate
            .public_generic_delta(candidate.candidate_digest())
            .unwrap(),
        bytes
    );
    let verification = candidate
        .verify_public_generic_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
    let verification: Value = serde_json::from_str(&verification).unwrap();
    assert_eq!(
        verification["schema"],
        "semaprax.project-candidate-public-generic-delta-verification.v1"
    );
    assert_eq!(verification["result"], "exact_recomputation");
    assert_eq!(verification["submitted_bytes_authority"], false);
    assert!(verification["delta_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

/// The separation invariant, in delta form.
///
/// The real GEN-05 project owns a `Pair<Bytes, bool>` inside its body. Its one
/// export is `fn(borrow Slice<u8>) -> i64`, so the grammar refuses the first
/// parameter with its exact closed reason and the route describes no surface
/// at all. That all-excluded report must still be complete and valid — and it
/// must not contain the template, the instance term, or any record identity.
#[test]
fn the_real_generic_project_is_all_excluded_and_leaks_no_generic_identity() {
    let fixture = Fixture::generic();
    let candidate = fixture.candidate();
    let (bytes, value) = delta(&candidate);

    for side in ["base", "candidate"] {
        let side = &value["facts"][side];
        assert_eq!(side["selected"], 1);
        assert!(side["described"].as_array().unwrap().is_empty());
        assert_eq!(side["surface"], Value::Null);
        assert_eq!(side["surface_digest"], Value::Null);
        assert_eq!(side["described_instances"], 0);
        assert_eq!(
            side["excluded"],
            json!([{
                "export": "generic.product.evaluate",
                "position": "parameter#0",
                "reason": "borrowed_byte_view",
            }])
        );
    }
    let comparison = &value["facts"]["comparison"];
    assert_eq!(comparison["verdict"], "unchanged");
    assert_eq!(
        comparison["basis"],
        "no_described_export_on_either_revision"
    );
    assert!(comparison["findings"].as_array().unwrap().is_empty());
    assert_eq!(comparison["public_generic_compatibility_v1"], Value::Null);
    assert_eq!(comparison["base_surface_digest"], Value::Null);
    assert_eq!(value["inventory"]["candidate_described"], 0);
    assert_eq!(value["inventory"]["candidate_excluded"], 1);

    // No template identity, no instance term, no record identity, and none of
    // the surface's own instance-describing keys appear anywhere in the bytes.
    // `@` is the grammar's instance sigil and also frames every owned-leaf
    // path, so its absence alone rules out a rendered instance.
    for forbidden in [
        "generic.product.pair",
        "generic.product.pair.left",
        "generic.product.pair.right",
        "Pair",
        "@",
        "\"template\"",
        "\"arity\"",
        "owned_leaves",
        "instance_digest",
        "term_digest",
        "\"fields\"",
        "\"instances\"",
        "\"entries\"",
    ] {
        assert!(
            !bytes.contains(forbidden),
            "all-excluded public generic delta leaked {forbidden}"
        );
    }
    // The report is nonetheless complete and independently replayable.
    candidate
        .verify_public_generic_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
}

/// A display rename is not compatibility. Applied through `SemanticChange` on
/// the described-surface fixture, the rendered surfaces disagree about the
/// name while the surface digests and the verdict do not move; applied to the
/// real generic project, the exclusion and the verdict stay exactly as they
/// were.
#[test]
fn a_display_rename_does_not_move_the_candidate_verdict() {
    let scalar = Fixture::scalar();
    let base = scalar.candidate();
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"pg.add","name":"add_scalars"}),
    );
    let (_, value) = delta(&renamed);
    let comparison = &value["facts"]["comparison"];
    assert_eq!(comparison["verdict"], "unchanged");
    assert!(comparison["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        comparison["base_surface_digest"],
        comparison["candidate_surface_digest"]
    );
    // The rename is real: the presentation differs on the two sides even
    // though every identity-bearing fact, and therefore the digest, does not.
    assert_eq!(
        value["facts"]["base"]["surface"]["entries"]["pg.add"]["name"],
        "add"
    );
    assert_eq!(
        value["facts"]["candidate"]["surface"]["entries"]["pg.add"]["name"],
        "add_scalars"
    );
    assert_eq!(
        value["facts"]["candidate"]["described"],
        json!(["pg.add"]),
        "selection is by stable identity, so a rename does not change it"
    );

    let generic = Fixture::generic();
    let base = generic.candidate();
    let renamed = apply(
        &base,
        json!({"kind":"rename_declaration","target":"generic.product.evaluate","name":"evaluate_product"}),
    );
    let (bytes, value) = delta(&renamed);
    assert_eq!(value["facts"]["comparison"]["verdict"], "unchanged");
    assert_eq!(
        value["facts"]["candidate"]["excluded"][0]["reason"],
        "borrowed_byte_view"
    );
    assert!(!bytes.contains("generic.product.pair"));
    renamed
        .verify_public_generic_delta(renamed.candidate_digest(), bytes.as_bytes())
        .unwrap();
}

/// A record instance in a described surface retains its template identity, its
/// ordered arguments, its substituted fields in declaration order, and its
/// transitive owned leaves. Adding a field to that record is breaking, and the
/// finding is reported on the instance rather than on the signature position
/// that mentions it.
#[test]
fn a_reachable_record_field_addition_is_breaking_and_retains_the_substituted_fields() {
    let fixture = Fixture::record();
    let base = fixture.candidate();
    let (_, before) = delta(&base);
    let instances = before["facts"]["base"]["surface"]["instances"]
        .as_object()
        .unwrap();
    assert_eq!(instances.len(), 1);
    let (term, instance) = instances.iter().next().unwrap();
    assert_eq!(term, "@10:pgr.packet<>");
    assert_eq!(instance["template"]["declaration"], "pgr.packet");
    assert_eq!(instance["template"]["arity"], 0);
    assert!(instance["arguments"].as_array().unwrap().is_empty());
    let fields = instance["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0]["id"], "pgr.packet.payload");
    assert_eq!(fields[0]["term"], "bytes");
    assert_eq!(fields[1]["id"], "pgr.packet.value");
    assert_eq!(fields[1]["term"], "i64");
    assert_eq!(instance["owned_leaves"], json!(["@18:pgr.packet.payload"]));

    let candidate = apply(
        &base,
        json!({"kind":"add_record_field","target":"pgr.packet","field":{"id":"pgr.packet.tag","name":"tag","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    let (bytes, value) = delta(&candidate);
    let comparison = &value["facts"]["comparison"];
    assert_eq!(comparison["verdict"], "breaking");
    let findings = comparison["findings"].as_array().unwrap();
    assert_eq!(
        findings
            .iter()
            .map(|finding| (
                finding["subject"].as_str().unwrap(),
                finding["reason"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        vec![("@10:pgr.packet<>", "instance_fields_changed")],
        "a reachable field change is reported once, on the record that changed"
    );
    assert_eq!(
        comparison["public_generic_compatibility_v1"]["verdict"],
        "breaking"
    );
    assert_eq!(
        comparison["public_generic_compatibility_v1"]["semantic_version_decision"],
        "not_inferred"
    );
    // The signature term is unchanged: the record is the same instance, with a
    // different substituted field tree.
    assert_ne!(
        comparison["base_surface_digest"],
        comparison["candidate_surface_digest"]
    );
    let candidate_fields = value["facts"]["candidate"]["surface"]["instances"]["@10:pgr.packet<>"]
        ["fields"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(candidate_fields.len(), 3);
    assert_eq!(candidate_fields[2]["id"], "pgr.packet.tag");
    assert_eq!(candidate_fields[2]["term"], "bool");
    // The verdict is a classification, never a support or publication decision.
    assert_eq!(value["support"], "not_assessed");
    assert_eq!(value["publication"], "not_assessed");
    candidate
        .verify_public_generic_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();
}

/// Mutation and recovery. A candidate restored from its recovery capsule
/// recomputes byte-identical delta bytes, and a delta bound to a digest that
/// is not this candidate's fails closed on the existing candidate selectors
/// before any surface is described.
#[test]
fn recovery_is_byte_identical_and_a_stale_candidate_selector_fails_closed() {
    let fixture = Fixture::record();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"add_record_field","target":"pgr.packet","field":{"id":"pgr.packet.tag","name":"tag","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    let (bytes, _) = delta(&candidate);
    let restored = ProjectCandidate::restore(
        Arc::clone(candidate.base_revision()),
        candidate.base_revision().project_revision(),
        candidate.recovery_capsule().unwrap().as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_eq!(
        restored
            .public_generic_delta(restored.candidate_digest())
            .unwrap(),
        bytes
    );
    restored
        .verify_public_generic_delta(restored.candidate_digest(), bytes.as_bytes())
        .unwrap();

    // A different candidate's digest, a well-formed digest of nothing, and a
    // malformed selector all fail before the route describes anything.
    code(
        candidate.public_generic_delta(base.candidate_digest()),
        "SPX-G224",
    );
    code(
        candidate.public_generic_delta(
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        ),
        "SPX-G224",
    );
    code(candidate.public_generic_delta("not-a-digest"), "SPX-G222");
    code(
        candidate.verify_public_generic_delta(base.candidate_digest(), bytes.as_bytes()),
        "SPX-G224",
    );
}

/// Independent replay compares bytes. It accepts the exact report and rejects
/// a single-byte mutation, a truncation, a JSON-equal re-serialization with
/// reordered keys, and the report of a different candidate.
#[test]
fn independent_replay_rejects_mutation_truncation_reordering_and_another_candidate() {
    let fixture = Fixture::record();
    let base = fixture.candidate();
    let candidate = apply(
        &base,
        json!({"kind":"add_record_field","target":"pgr.packet","field":{"id":"pgr.packet.tag","name":"tag","type":"bool","default":{"kind":"bool","value":false}}}),
    );
    let (bytes, _) = delta(&candidate);
    candidate
        .verify_public_generic_delta(candidate.candidate_digest(), bytes.as_bytes())
        .unwrap();

    let mut mutated = bytes.clone().into_bytes();
    let index = mutated
        .iter()
        .position(|byte| *byte == b'b')
        .expect("the report contains a lowercase b");
    mutated[index] = b'c';
    assert_ne!(mutated, bytes.as_bytes());
    code(
        candidate.verify_public_generic_delta(candidate.candidate_digest(), &mutated),
        "SPX-PG303",
    );

    code(
        candidate.verify_public_generic_delta(
            candidate.candidate_digest(),
            &bytes.as_bytes()[..bytes.len() - 1],
        ),
        "SPX-PG303",
    );
    code(
        candidate.verify_public_generic_delta(candidate.candidate_digest(), b""),
        "SPX-PG303",
    );
    code(
        candidate.verify_public_generic_delta(
            candidate.candidate_digest(),
            reordered(&bytes).as_bytes(),
        ),
        "SPX-PG303",
    );

    // A tampered status field is a byte mismatch, not a re-derived verdict.
    let mut tampered: Value = serde_json::from_str(&bytes).unwrap();
    tampered["support"] = json!("supported");
    tampered["facts"]["comparison"]["verdict"] = json!("unchanged");
    code(
        candidate.verify_public_generic_delta(
            candidate.candidate_digest(),
            format!("{tampered}\n").as_bytes(),
        ),
        "SPX-PG303",
    );

    // The bytes of a different candidate of the same base are still bytes.
    let (other, _) = delta(&base);
    assert_ne!(other, bytes);
    code(
        candidate.verify_public_generic_delta(candidate.candidate_digest(), other.as_bytes()),
        "SPX-PG303",
    );
    code(
        base.verify_public_generic_delta(base.candidate_digest(), bytes.as_bytes()),
        "SPX-PG303",
    );

    // Oversized submissions are refused by the bound, not parsed.
    code(
        candidate.verify_public_generic_delta(
            candidate.candidate_digest(),
            &vec![b' '; MAX_PROJECT_CANDIDATE_PUBLIC_GENERIC_DELTA_BYTES + 1],
        ),
        "SPX-PG302",
    );
}
