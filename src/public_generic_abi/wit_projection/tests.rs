//! Executable evidence for [`project_admitted_subject`]. Every positive case
//! is a real program compiled through `crate::parse` + `crate::hir::resolve`
//! and classified through the actual
//! [`crate::public_generic_abi::classifier::classify`] admission, exactly the
//! technique `classifier::tests` already uses for its own fixtures — this
//! module defines no parallel notion of what a "checked program" is.
//!
//! Declaration ids below are deliberately chosen so their canonical grammar
//! terms sort in the *opposite* order from their dependency relationship
//! (`wit_projection.aaa_pair` < `wit_projection.zzz_leaf` lexically, even
//! though `Pair` contains `Leaf`). [`project_admitted_subject`] visits the
//! closure's `BTreeMap` in term order, so if record emission only followed
//! that top-level iteration, `Pair` would be emitted, and would reference a
//! `Leaf` record not yet declared, before `Leaf` itself. The ordering
//! assertion below is what actually distinguishes "records are emitted in
//! dependency order" from "records happen to come out alphabetically."

use std::path::Path;

use super::*;
use crate::hir::{self, ResolvedProgram};
use crate::parse;
use crate::public_generic_abi::classifier::classify;
use crate::public_generic_type::{FieldFact, InstanceFacts, TemplateIdentity};

const BASE: &str = r#"
module test.public_generic_wit_projection;

permit { clock.read }

@id("wit_projection.aaa_pair")
record Pair<T, U> {
    @id("wit_projection.pair.left")
    left: T,
    @id("wit_projection.pair.right")
    right: U,
}

@id("wit_projection.zzz_leaf")
record Leaf {
    @id("wit_projection.leaf.head")
    head: Bytes,
}

@id("wit_projection.scalars")
record Scalars {
    @id("wit_projection.scalars.a")
    a: i64,
    @id("wit_projection.scalars.b")
    b: i32,
    @id("wit_projection.scalars.c")
    c: u8,
    @id("wit_projection.scalars.d")
    d: usize,
    @id("wit_projection.scalars.e")
    e: char,
    @id("wit_projection.scalars.f")
    f: f32,
    @id("wit_projection.scalars.g")
    g: f64,
    @id("wit_projection.scalars.h")
    h: bool,
    @id("wit_projection.scalars.leaf")
    leaf: Bytes,
}

@id("wit_projection.take")
fn take(value: own Pair<Leaf, i64>) -> Pair<Leaf, i64> { value }

@id("wit_projection.scalars_take")
fn scalars_take(value: own Scalars) -> Scalars { value }

@id("wit_projection.main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("wit_projection.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

/// A from-scratch hex encoder, independent of `super::legal_name`. Used to
/// compute an expected WIT name without exercising the same code path being
/// tested, so a bug shared by both would not go unnoticed.
fn reference_wit_name(identity: &str) -> String {
    let mut out = String::from("spx-");
    for byte in identity.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn fake_facts(term: &str, fields: Vec<FieldFact>) -> InstanceFacts {
    InstanceFacts {
        term: term.to_owned(),
        term_digest: "sha256:0".to_string(),
        template: TemplateIdentity {
            declaration: "fixture".to_owned(),
            name: "fixture".to_owned(),
            arity: 0,
            parameters: Vec::new(),
            digest: "sha256:0".to_owned(),
        },
        arguments: Vec::new(),
        fields,
        owned_leaves: Vec::new(),
        instance_digest: "sha256:0".to_owned(),
    }
}

#[test]
fn projects_records_in_dependency_order_with_a_shared_bytes_resource() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let projection = project_admitted_subject(&admitted).unwrap();

    assert_eq!(projection.schema, WIT_TYPE_PROJECTION_SCHEMA);
    assert_eq!(projection.package, WIT_PACKAGE);
    assert!(projection.uses_owned_bytes_resource);
    assert_eq!(projection.records.len(), 2);

    let leaf_term = admitted
        .record_closure()
        .keys()
        .find(|term| term.ends_with("zzz_leaf<>"))
        .expect("Leaf is in the closure")
        .clone();
    let pair_term = admitted.input().term.clone();
    assert!(pair_term.contains("aaa_pair"));
    // The closure's own key order puts `aaa_pair` before `zzz_leaf`.
    assert!(admitted.record_closure().keys().next().unwrap() == &pair_term);

    let leaf_name = reference_wit_name(&leaf_term);
    let pair_name = reference_wit_name(&pair_term);
    assert_eq!(
        projection.records[0].name, leaf_name,
        "Leaf (the dependency) must be emitted first"
    );
    assert_eq!(
        projection.records[1].name, pair_name,
        "Pair (the dependent) must be emitted second"
    );

    assert_eq!(projection.input_type, pair_name);
    assert_eq!(projection.result_type, pair_name);

    let leaf_record = &projection.records[0];
    assert_eq!(leaf_record.fields.len(), 1);
    assert_eq!(
        leaf_record.fields[0].name,
        reference_wit_name("wit_projection.leaf.head")
    );
    assert_eq!(
        leaf_record.fields[0].type_text,
        format!("own<{OWNED_BYTES_RESOURCE}>")
    );

    let pair_record = &projection.records[1];
    assert_eq!(pair_record.fields.len(), 2);
    assert_eq!(
        pair_record.fields[0].name,
        reference_wit_name("wit_projection.pair.left")
    );
    assert_eq!(pair_record.fields[0].type_text, leaf_name);
    assert_eq!(
        pair_record.fields[1].name,
        reference_wit_name("wit_projection.pair.right")
    );
    assert_eq!(pair_record.fields[1].type_text, "s64");

    assert!(projection.wit.starts_with(&format!(
        "package {WIT_PACKAGE};\n\ninterface {WIT_INTERFACE} {{\n"
    )));
    assert!(projection
        .wit
        .contains(&format!("resource {OWNED_BYTES_RESOURCE};\n")));
    assert!(projection.wit.ends_with(&format!(
        "world {WIT_WORLD} {{\n  export {WIT_INTERFACE};\n}}\n"
    )));
    assert!(projection.wit.len() <= MAX_WIT_PROJECTION_BYTES);
}

#[test]
fn projection_is_byte_deterministic_across_repeated_calls() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let first = project_admitted_subject(&admitted).unwrap();
    let second = project_admitted_subject(&admitted).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.wit, second.wit);
}

#[test]
fn usize_is_admitted_by_the_grammar_but_refused_by_this_wit_projection_specifically() {
    let program = resolved(BASE);
    // The grammar/classifier admit `usize` (PG-1 vocabulary includes it);
    // the failure this test names belongs to the WIT projection layer, not
    // to an earlier check, so the classification itself must succeed first.
    let admitted = classify(&program, "wit_projection.scalars_take")
        .expect("usize is an admitted grammar scalar; classification must succeed");

    let error = project_admitted_subject(&admitted).unwrap_err();
    assert_eq!(error.code, UNSUPPORTED_SCALAR);
    assert!(error.message.contains("usize"), "{}", error.message);
}

#[test]
fn round_trip_parses_back_to_the_pre_render_structure_not_a_re_render() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let projection = project_admitted_subject(&admitted).unwrap();

    let parsed = parse_wit_projection(&projection.wit).unwrap();

    // Compare the parsed structure to the pre-render `WitTypeProjectionV1`
    // facts that produced the text, not to a second call to `render`.
    assert_eq!(parsed.package, projection.package);
    assert_eq!(parsed.interface, projection.interface);
    assert_eq!(parsed.world, projection.world);
    assert_eq!(
        parsed.uses_owned_bytes_resource,
        projection.uses_owned_bytes_resource
    );
    assert_eq!(parsed.records.len(), projection.records.len());
    for (parsed_record, original_record) in parsed.records.iter().zip(projection.records.iter()) {
        assert_eq!(parsed_record.name, original_record.name);
        let expected_fields: Vec<(String, String)> = original_record
            .fields
            .iter()
            .map(|field| (field.name.clone(), field.type_text.clone()))
            .collect();
        assert_eq!(parsed_record.fields, expected_fields);
    }
}

#[test]
fn truncated_wit_text_is_rejected() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let projection = project_admitted_subject(&admitted).unwrap();
    let truncated = &projection.wit[..projection.wit.len() - 5];
    let error = parse_wit_projection(truncated).unwrap_err();
    assert_eq!(error.code, MALFORMED_WIT);
}

#[test]
fn mutated_field_separator_is_rejected() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let projection = project_admitted_subject(&admitted).unwrap();
    let mutated = projection.wit.replacen(": ", ":=", 1);
    assert_ne!(mutated, projection.wit);
    let error = parse_wit_projection(&mutated).unwrap_err();
    assert_eq!(error.code, MALFORMED_WIT);
}

#[test]
fn trailing_byte_after_the_world_block_is_rejected() {
    let program = resolved(BASE);
    let admitted = classify(&program, "wit_projection.take").unwrap();
    let projection = project_admitted_subject(&admitted).unwrap();
    let mut mutated = projection.wit.clone();
    mutated.push('\n');
    let error = parse_wit_projection(&mutated).unwrap_err();
    assert_eq!(error.code, MALFORMED_WIT);
}

#[test]
fn wit_names_stay_distinct_and_wit_legal_for_adversarial_identities() {
    // Identities deliberately reuse grammar punctuation (`@`, `<`, `>`, `:`,
    // `,`) and near-duplicate substrings, the same style of hostile fixture
    // `public_generic_type`'s own injectivity case uses. Hex framing must
    // keep every one of these distinct.
    let identities = [
        "a.b",
        "a-b",
        "a_b",
        "@13:x<y>,z",
        "@13:x<y>,z ",
        "",
        "\0",
        "@14:x<y>,z",
    ];
    let names: Vec<String> = identities
        .iter()
        .map(|identity| legal_name(identity))
        .collect();
    let unique: std::collections::BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), identities.len(), "{names:?}");
    for name in &names {
        assert!(name.starts_with("spx-"));
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase() || byte == b'-'),
            "{name} is not a legal WIT kebab identifier"
        );
    }
}

#[test]
fn missing_closure_member_is_refused_with_the_exact_missing_term() {
    let pair = fake_facts(
        "@12:missing.pair<>",
        vec![FieldFact {
            index: 0,
            id: "missing.pair.left".to_owned(),
            name: "left".to_owned(),
            term: "@12:missing.leaf<>".to_owned(),
            digest: "sha256:0".to_owned(),
        }],
    );
    let mut projector = Projector {
        closure: [(pair.term.as_str(), &pair)].into_iter().collect(),
        emitted: std::collections::BTreeSet::new(),
        records: Vec::new(),
        uses_owned_bytes_resource: false,
    };
    let term = pair.term.clone();
    let error = projector.emit_record(&term, 1).unwrap_err();
    assert_eq!(error.code, MISSING_CLOSURE_MEMBER);
    assert!(
        error.message.contains("@12:missing.leaf<>"),
        "{}",
        error.message
    );
}

#[test]
fn a_closure_entry_whose_own_term_disagrees_with_its_map_key_is_refused() {
    let leaf = fake_facts("@9:real.leaf<>", Vec::new());
    let mut projector = Projector {
        closure: [("@10:wrong.leaf<>", &leaf)].into_iter().collect(),
        emitted: std::collections::BTreeSet::new(),
        records: Vec::new(),
        uses_owned_bytes_resource: false,
    };
    let error = projector.emit_record("@10:wrong.leaf<>", 1).unwrap_err();
    assert_eq!(error.code, CLOSURE_INCONSISTENT);
}
