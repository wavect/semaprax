use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::*;

const SCHEMA_DOMAIN: &[u8] = b"semaprax.agent-interaction-schema.digest.v1\0";
const REVISION_DOMAIN: &[u8] = b"semaprax.agent-interaction-schema.type-revision.v1\0";

/// One nested record (`Inner`), one record referencing it plus every
/// admitted direct scalar kind (`Outer`), one standalone bounded-`Bytes`
/// record (`WithBytes`), and one variant (`Choice`; case payloads in this
/// language admit only direct Copy scalars, never a nested record or
/// `string`, so its case carries a flat `i64` rather than a nested field).
/// Every executable module needs `fn main() -> i64`.
const FIXTURE: &str = r#"
module test.agent_interaction_schema;

@id("inner.type")
record Inner {
    @id("inner.x")
    x: i64,
    @id("inner.y")
    y: string,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.small")
    small: i32,
    @id("outer.tag")
    tag: u8,
    @id("outer.count")
    count: usize,
    @id("outer.inner")
    inner: Inner,
}

@id("bytes.type")
record WithBytes {
    @id("bytes.tag")
    tag: u8,
    @id("bytes.blob")
    blob: Bytes,
}

@id("choice.type")
variant Choice {
    @id("choice.a")
    A {
        @id("choice.a.n")
        n: i64,
    },
    @id("choice.b")
    B,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const RENAMED_FIXTURE: &str = r#"
module test.agent_interaction_schema;

@id("inner.type")
record InnerRenamed {
    @id("inner.x")
    xx: i64,
    @id("inner.y")
    yy: string,
}

@id("outer.type")
record OuterRenamed {
    @id("outer.flag")
    flagg: bool,
    @id("outer.small")
    smalll: i32,
    @id("outer.tag")
    tagg: u8,
    @id("outer.count")
    countt: usize,
    @id("outer.inner")
    innerr: InnerRenamed,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const STRUCTURAL_CHANGE_FIXTURE: &str = r#"
module test.agent_interaction_schema;

@id("inner.type")
record Inner {
    @id("inner.x")
    x: i64,
    @id("inner.y")
    y: string,
    @id("inner.z")
    z: bool,
}

@id("outer.type")
record Outer {
    @id("outer.flag")
    flag: bool,
    @id("outer.small")
    small: i32,
    @id("outer.tag")
    tag: u8,
    @id("outer.count")
    count: usize,
    @id("outer.inner")
    inner: Inner,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const GENERIC_ARGUMENT_FIXTURE: &str = r#"
module test.agent_interaction_schema;

@id("boxed.type")
record Boxed<T> {
    @id("boxed.value")
    value: T,
}

@id("outer2.type")
record Outer2 {
    @id("outer2.boxed")
    boxed: Boxed<i64>,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const UNSUPPORTED_TYPE_FIXTURE: &str = r#"
module test.agent_interaction_schema;

@id("unsupported.type")
record HasFloat {
    @id("unsupported.value")
    value: f64,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(source: &str, label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-agent-interaction-schema-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn expect_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// Two independent derivations of the same source produce byte-identical
/// schemas, and the schema/revision digests are independently
/// recomputable, not merely self-consistent.
#[test]
fn golden_derivation_is_deterministic_and_digests_are_independently_verifiable() {
    let path = write_temp(FIXTURE, "golden");
    let first =
        compile_agent_interaction_schema(&path, "outer.type").expect("first derivation succeeds");
    let second =
        compile_agent_interaction_schema(&path, "outer.type").expect("second derivation succeeds");
    let bytes_schema = compile_agent_interaction_schema(&path, "bytes.type")
        .expect("bytes.type derivation succeeds");
    std::fs::remove_file(&path).ok();

    assert_eq!(
        first.schema().canonical_json(),
        second.schema().canonical_json(),
        "re-deriving the same checked source must produce byte-identical schema JSON"
    );
    assert_eq!(first.schema().digest(), second.schema().digest());

    let json = first.schema().canonical_json();
    assert!(json.starts_with(
        "{\"schema\":\"semaprax.agent-interaction-schema.v1\",\"root_type_id\":\"outer.type\""
    ));
    assert!(json.ends_with('\n'));
    assert!(json.contains(
        "\"types\":[{\"stable_id\":\"inner.type\",\"kind\":\"record\",\"fields\":[{\"stable_id\":\"inner.x\",\"type\":{\"kind\":\"scalar\",\"representation\":\"i64\",\"minimum\":\"-9223372036854775808\",\"maximum\":\"9223372036854775807\"}},{\"stable_id\":\"inner.y\",\"type\":{\"kind\":\"scalar\",\"representation\":\"string\",\"max_bytes\":4096}}]}"
    ));
    assert!(json.contains(
        "{\"stable_id\":\"outer.inner\",\"type\":{\"kind\":\"nested\",\"stable_id\":\"inner.type\"}}"
    ));
    assert!(json.contains("\"max_types\":64"));
    assert!(json.contains("\"max_depth\":16"));

    let expected_revision = expect_digest(
        REVISION_DOMAIN,
        render::render_revision_body("outer.type", &render::render_types(&first.graph.types))
            .as_bytes(),
    );
    assert_eq!(first.schema().root_type_revision(), expected_revision);
    let expected_digest = expect_digest(SCHEMA_DOMAIN, json.as_bytes());
    assert_eq!(first.schema().digest(), expected_digest);

    let bytes_json = bytes_schema.schema().canonical_json();
    assert!(bytes_json.contains(
        "{\"stable_id\":\"bytes.blob\",\"type\":{\"kind\":\"scalar\",\"representation\":\"bytes\",\"max_bytes\":4096}}"
    ));
}

/// A display rename (declaration and field display names change, every
/// `@id` stays the same) leaves the root type revision and schema digest
/// unchanged; a genuine structural change (one added field) changes both.
#[test]
fn display_rename_preserves_identity_structural_change_invalidates_it() {
    let original_path = write_temp(FIXTURE, "rename-original");
    let renamed_path = write_temp(RENAMED_FIXTURE, "rename-renamed");
    let changed_path = write_temp(STRUCTURAL_CHANGE_FIXTURE, "rename-changed");

    let original = compile_agent_interaction_schema(&original_path, "outer.type")
        .expect("original derivation succeeds");
    let renamed = compile_agent_interaction_schema(&renamed_path, "outer.type")
        .expect("renamed derivation succeeds");
    let changed = compile_agent_interaction_schema(&changed_path, "outer.type")
        .expect("structurally changed derivation succeeds");

    std::fs::remove_file(&original_path).ok();
    std::fs::remove_file(&renamed_path).ok();
    std::fs::remove_file(&changed_path).ok();

    assert_eq!(
        original.schema().root_type_revision(),
        renamed.schema().root_type_revision(),
        "a pure display rename must not change the derived revision"
    );
    assert_eq!(original.schema().digest(), renamed.schema().digest());

    assert_ne!(
        original.schema().root_type_revision(),
        changed.schema().root_type_revision(),
        "an actual field addition must invalidate the derived revision"
    );
    assert_ne!(original.schema().digest(), changed.schema().digest());
}

fn record_value(fields: &[(&str, &str)]) -> String {
    let body = fields
        .iter()
        .map(|(id, value)| format!("\"{id}\":{value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"fields\":{{{body}}}}}")
}

fn inner_value(x: i64, y: &str) -> String {
    record_value(&[
        ("inner.x", &format!("\"{x}\"")),
        ("inner.y", &crate::diagnostic::quote_json(y)),
    ])
}

fn document(root_type_id: &str, schema_digest: &str, value: &str) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":{},\"schema_digest\":{},\"value\":{value}}}\n",
        crate::diagnostic::quote_json(root_type_id),
        crate::diagnostic::quote_json(schema_digest),
    )
}

fn outer_document(schema_digest: &str, inner: &str) -> String {
    let value = record_value(&[
        ("outer.flag", "true"),
        ("outer.small", "\"-2147483648\""),
        ("outer.tag", "\"255\""),
        ("outer.count", "\"18446744073709551615\""),
        ("outer.inner", inner),
    ]);
    document("outer.type", schema_digest, &value)
}

fn bytes_document(schema_digest: &str, tag: u8, blob: &[u8]) -> String {
    let blob_json = format!(
        "[{}]",
        blob.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
    );
    let value = record_value(&[
        ("bytes.tag", &format!("\"{tag}\"")),
        ("bytes.blob", &blob_json),
    ]);
    document("bytes.type", schema_digest, &value)
}

/// Every admitted direct scalar kind and one nested record field round-trip
/// through decode without losing type identity; large integer boundary
/// values (`i32::MIN`, `u8::MAX`, `u64::MAX`, `i64::MIN`) come back as the
/// exact decimal value, never a rounded float.
#[test]
fn full_record_with_nested_field_round_trips_exactly() {
    let path = write_temp(FIXTURE, "roundtrip");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let inner = inner_value(i64::MIN, "héllo\u{1F600}");
    let doc = outer_document(compiled.schema().digest(), &inner);

    let decoded = compiled
        .decode(doc.as_bytes())
        .expect("a fully admitted document must decode");
    assert_eq!(decoded.root_type_id(), "outer.type");
    assert_eq!(decoded.schema_digest(), compiled.schema().digest());
    assert_eq!(decoded.canonical_json(), doc);

    let TypedValue::Record { .. } = decoded.value() else {
        panic!("Outer decodes as a record");
    };
    assert_eq!(
        decoded.value().field("outer.flag"),
        Some(&FieldValue::Scalar(ScalarValue::Bool(true)))
    );
    assert_eq!(
        decoded.value().field("outer.small"),
        Some(&FieldValue::Scalar(ScalarValue::Signed(i32::MIN as i64)))
    );
    assert_eq!(
        decoded.value().field("outer.tag"),
        Some(&FieldValue::Scalar(ScalarValue::Unsigned(255)))
    );
    assert_eq!(
        decoded.value().field("outer.count"),
        Some(&FieldValue::Scalar(ScalarValue::Unsigned(u64::MAX)))
    );
    let Some(FieldValue::Nested(nested)) = decoded.value().field("outer.inner") else {
        panic!("outer.inner decodes as a nested value");
    };
    assert_eq!(
        nested.field("inner.x"),
        Some(&FieldValue::Scalar(ScalarValue::Signed(i64::MIN)))
    );
    assert_eq!(
        nested.field("inner.y"),
        Some(&FieldValue::Scalar(ScalarValue::Text(
            "héllo\u{1F600}".to_owned()
        )))
    );
}

/// A bounded `Bytes` field round-trips exactly as the declared array of
/// byte integers.
#[test]
fn bytes_field_round_trips_exactly() {
    let path = write_temp(FIXTURE, "bytes-roundtrip");
    let compiled =
        compile_agent_interaction_schema(&path, "bytes.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let doc = bytes_document(compiled.schema().digest(), 9, &[0, 1, 255, 128]);
    let decoded = compiled
        .decode(doc.as_bytes())
        .expect("a fully admitted Bytes document must decode");
    assert_eq!(decoded.canonical_json(), doc);
    assert_eq!(
        decoded.value().field("bytes.tag"),
        Some(&FieldValue::Scalar(ScalarValue::Unsigned(9)))
    );
    assert_eq!(
        decoded.value().field("bytes.blob"),
        Some(&FieldValue::Scalar(ScalarValue::Bytes(vec![
            0, 1, 255, 128
        ])))
    );
}

/// A variant case round-trips, and selecting an unknown/wrong tag is
/// refused rather than defaulting to a case.
#[test]
fn variant_case_round_trips_and_rejects_wrong_tag() {
    let path = write_temp(FIXTURE, "variant");
    let compiled =
        compile_agent_interaction_schema(&path, "choice.type").expect("derivation succeeds");

    let good = document(
        "choice.type",
        compiled.schema().digest(),
        "{\"case\":\"choice.a\",\"fields\":{\"choice.a.n\":\"7\"}}",
    );
    let decoded = compiled
        .decode(good.as_bytes())
        .expect("the declared case must decode");
    assert_eq!(decoded.value().case(), Some("choice.a"));
    assert_eq!(
        decoded.value().field("choice.a.n"),
        Some(&FieldValue::Scalar(ScalarValue::Signed(7)))
    );

    let empty_case = document(
        "choice.type",
        compiled.schema().digest(),
        "{\"case\":\"choice.b\",\"fields\":{}}",
    );
    let decoded_empty = compiled
        .decode(empty_case.as_bytes())
        .expect("the empty-field case must decode");
    assert_eq!(decoded_empty.value().case(), Some("choice.b"));

    let bad_tag = document(
        "choice.type",
        compiled.schema().digest(),
        "{\"case\":\"choice.unknown\",\"fields\":{}}",
    );
    let error = compiled
        .decode(bad_tag.as_bytes())
        .expect_err("an unknown variant tag must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("value.case"));

    std::fs::remove_file(&path).ok();
}

/// Malformed UTF-8 response bytes are refused with a stable diagnostic
/// rather than panicking or being silently lossily decoded.
#[test]
fn malformed_utf8_is_refused() {
    let path = write_temp(FIXTURE, "utf8");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let invalid_utf8: &[u8] = b"{\"schema\":\"semaprax.agent-interaction-value.v1\xff\"}\n";
    let error = compiled
        .decode(invalid_utf8)
        .expect_err("invalid UTF-8 must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("utf8"));
}

/// A document whose bytes exceed the declared budget is refused before any
/// unbounded parsing work, never truncated.
#[test]
fn oversized_document_is_refused() {
    let path = write_temp(FIXTURE, "oversized");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let oversized = vec![b'a'; MAX_DOCUMENT_BYTES + 1];
    let error = compiled
        .decode(&oversized)
        .expect_err("an oversized document must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("document_bytes"));
}

/// A duplicate JSON key is never silently collapsed by a generic map: the
/// canonical single-occurrence replay can never byte-match a source that
/// repeats a key, so decode refuses it.
#[test]
fn duplicate_json_key_is_refused() {
    let path = write_temp(FIXTURE, "duplicate");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let inner = inner_value(1, "x");
    let good = outer_document(compiled.schema().digest(), &inner);
    // Duplicate the `outer.flag` key inside the otherwise-valid `fields`
    // object with a differing second value, forcing a naive last-write-wins
    // map to disagree with what a first-occurrence reading would produce —
    // either way the exact-replay check below must reject it.
    let with_duplicate = good.replacen(
        "\"outer.flag\":true",
        "\"outer.flag\":true,\"outer.flag\":false",
        1,
    );
    assert_ne!(
        good, with_duplicate,
        "the fixture must actually inject a duplicate key"
    );
    let error = compiled
        .decode(with_duplicate.as_bytes())
        .expect_err("a duplicate JSON key must be refused, not silently collapsed");
    assert_eq!(error[0].code, "SPX-Z205");
}

/// An undeclared field name is refused rather than silently ignored.
#[test]
fn unknown_field_is_refused() {
    let path = write_temp(FIXTURE, "unknown-field");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let inner = inner_value(1, "x");
    let good = outer_document(compiled.schema().digest(), &inner);
    let with_unknown = good.replacen(
        "\"outer.flag\":true",
        "\"outer.flag\":true,\"outer.unknown\":true",
        1,
    );
    let error = compiled
        .decode(with_unknown.as_bytes())
        .expect_err("an unknown field must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("value.fields.unknown"));
}

/// A missing required field is refused rather than defaulted.
#[test]
fn missing_field_is_refused() {
    let path = write_temp(FIXTURE, "missing-field");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let inner = inner_value(1, "x");
    let good = outer_document(compiled.schema().digest(), &inner);
    let missing = good.replacen("\"outer.flag\":true,", "", 1);
    let error = compiled
        .decode(missing.as_bytes())
        .expect_err("a missing field must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("value.fields.missing"));
}

/// A `Text` field over the declared byte bound is refused, not truncated.
#[test]
fn oversized_string_field_is_refused() {
    let path = write_temp(FIXTURE, "oversized-string");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let too_long = "a".repeat(MAX_STRING_FIELD_BYTES + 1);
    let inner = inner_value(1, &too_long);
    let doc = outer_document(compiled.schema().digest(), &inner);
    let error = compiled
        .decode(doc.as_bytes())
        .expect_err("an oversized string field must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("value.string_bytes"));
}

/// A `Bytes` field beyond the declared element bound is refused.
#[test]
fn oversized_bytes_field_is_refused() {
    let path = write_temp(FIXTURE, "oversized-bytes");
    let compiled =
        compile_agent_interaction_schema(&path, "bytes.type").expect("derivation succeeds");
    std::fs::remove_file(&path).ok();

    let too_many = vec![0u8; MAX_BYTES_FIELD_BYTES + 1];
    let doc = bytes_document(compiled.schema().digest(), 1, &too_many);
    let error = compiled
        .decode(doc.as_bytes())
        .expect_err("an oversized Bytes field must be refused");
    assert_eq!(error[0].code, "SPX-Z206");
    assert!(error[0].message.contains("value.bytes_length"));
}

/// A source type reachable only through a generic argument (`Boxed<i64>`)
/// is refused explicitly, not silently approximated as opaque JSON.
///
/// The base compiler already forecloses every nested generic-instantiated
/// record field universally (`SPX-T223`, independent of this module) before
/// `hir::resolve` ever succeeds, so this fixture never reaches this
/// module's own `type.field.generic_argument` guard in `shape::classify` —
/// that guard is retained as defense in depth for the same reason the
/// cycle-detection guard is (see the module documentation's known
/// limitations). What this test proves is the end-to-end contract this
/// module promises: a generic-argument-bearing nested field is refused,
/// not silently approximated, regardless of which layer refuses it first.
#[test]
fn generic_argument_nested_field_is_refused_explicitly() {
    let path = write_temp(GENERIC_ARGUMENT_FIXTURE, "generic");
    let error = compile_agent_interaction_schema(&path, "outer2.type")
        .expect_err("a generic-argument-bearing nested field must be refused");
    std::fs::remove_file(&path).ok();
    assert_eq!(error[0].code, "SPX-T223");
}

/// A source scalar type outside the admitted vocabulary (`f64`) is refused
/// explicitly rather than serialized as opaque untyped JSON.
#[test]
fn unsupported_scalar_type_is_refused_explicitly() {
    let path = write_temp(UNSUPPORTED_TYPE_FIXTURE, "unsupported");
    let error = compile_agent_interaction_schema(&path, "unsupported.type")
        .expect_err("an unsupported field type must be refused");
    std::fs::remove_file(&path).ok();
    assert_eq!(error[0].code, "SPX-Z202");
    assert!(error[0].message.contains("type.field.unsupported"));
}

/// `verify_agent_interaction_schema_bundle` independently rederives the
/// schema and requires an exact byte match; a stale/foreign document is
/// refused.
#[test]
fn verify_bundle_accepts_exact_replay_and_rejects_drift() {
    let path = write_temp(FIXTURE, "verify-bundle");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    let schema_source = compiled.schema().canonical_json().to_owned();

    verify_agent_interaction_schema_bundle(&path, "outer.type", &schema_source)
        .expect("the exact replayed schema must verify");

    let tampered = schema_source.replace("outer.type", "outer.other");
    let error = verify_agent_interaction_schema_bundle(&path, "outer.type", &tampered)
        .expect_err("a tampered schema document must fail verification");
    assert_eq!(error[0].code, "SPX-Z204");

    std::fs::remove_file(&path).ok();
}

/// A source path that does not resolve fails closed through the same
/// shared `patch::canonical_source_path` binding `capability_manifest`,
/// `region_report` and `assurance_manifest` use, rather than this module
/// inventing a second source-binding mechanism.
#[test]
fn missing_source_path_fails_closed_through_the_shared_binding() {
    let missing = Path::new("/nonexistent/semaprax-agent-interaction-schema-test.spx");
    let error = compile_agent_interaction_schema(missing, "outer.type")
        .expect_err("a nonexistent source path must fail closed");
    assert!(!error.is_empty());
}

/// The provider JSON Schema projection is a self-contained draft 2020-12
/// document describing exactly the admitted wire shape: exact integers as
/// decimal-string patterns (never `type: integer`, so no generated client
/// can round one through a native number), `Bytes` as the bounded
/// byte-integer array the decoder accepts, and nested types as local
/// `$defs` references needing no network lookup.
#[test]
fn provider_json_schema_matches_the_canonical_wire_shape() {
    let path = write_temp(FIXTURE, "provider");
    let compiled =
        compile_agent_interaction_schema(&path, "outer.type").expect("derivation succeeds");
    let bytes_compiled = compile_agent_interaction_schema(&path, "bytes.type")
        .expect("bytes.type derivation succeeds");
    let variant_compiled = compile_agent_interaction_schema(&path, "choice.type")
        .expect("variant derivation succeeds");
    std::fs::remove_file(&path).ok();

    let provider_schema = compiled.provider_json_schema();
    assert!(
        provider_schema.contains("\"$schema\":\"https://json-schema.org/draft/2020-12/schema\"")
    );
    assert!(provider_schema.contains("\"$ref\":\"#/$defs/outer.type\""));
    assert!(provider_schema.contains("\"$ref\":\"#/$defs/inner.type\""));
    assert!(provider_schema.contains(
        "\"outer.small\":{\"type\":\"string\",\"pattern\":\"^-?[0-9]+$\",\"x-representation\":\"i32\""
    ));
    assert!(!provider_schema.contains("\"outer.small\":{\"type\":\"integer\""));

    let bytes_schema = bytes_compiled.provider_json_schema();
    assert!(bytes_schema.contains(
        "\"bytes.blob\":{\"type\":\"array\",\"items\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":255}"
    ));

    let variant_schema = variant_compiled.provider_json_schema();
    assert!(variant_schema.contains("\"oneOf\":["));
    assert!(variant_schema.contains("\"case\":{\"const\":\"choice.a\"}"));
    assert!(variant_schema.contains("\"case\":{\"const\":\"choice.b\"}"));
}
