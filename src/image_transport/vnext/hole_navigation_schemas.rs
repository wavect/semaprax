//! Closed compact hole navigation; the full proof contexts remain unbundled.
use super::{array, digest, document, nullable, object, text, uint};
use crate::project::{
    MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES, MAX_PROJECT_HOLE_NAVIGATION_ITEMS,
    PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA, PROJECT_HOLE_PAGE_SCHEMA, PROJECT_HOLE_SUMMARY_SCHEMA,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn index() -> Value {
    json!({"type":"integer","minimum":0,"maximum":MAX_PROJECT_HOLE_NAVIGATION_ITEMS})
}

fn ownership() -> Value {
    json!({"enum":["value","own","borrow","shared"]})
}

pub(super) fn documents() -> BTreeMap<String, Value> {
    let mut documents = BTreeMap::new();
    documents.insert(
        format!("urn:{PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA}"),
        draft_expression_catalog(),
    );
    let facets = json!({"enum":["scope","calls","obligations","constructors"]});
    let mut references = array(object(vec![
        ("facet", facets),
        ("count", index()),
        ("reference", digest()),
    ]));
    references["minItems"] = json!(4);
    references["maxItems"] = json!(4);
    let summary = document(
        PROJECT_HOLE_SUMMARY_SCHEMA,
        vec![
            (
                "context_schema",
                json!({"enum":[
                    "semaprax.project-candidate-hole-context.v1",
                    "semaprax.project-candidate-expression-hole-context.v1",
                    "semaprax.project-candidate-contract-expression-hole-context.v1"
                ]}),
            ),
            ("context_revision", digest()),
            ("draft_revision", digest()),
            ("hole_id", text()),
            ("hole_handle", digest()),
            ("target", text()),
            ("last_valid_revision", digest()),
            ("expected_type_id", text()),
            ("expected_ownership", nullable(ownership())),
            (
                "intent_kind",
                json!({"enum":["replace_function_body","replace_expression","replace_contract_expression"]}),
            ),
            (
                "effect_policy",
                object(vec![
                    ("allowed", array(text())),
                    ("forbidden", text()),
                    ("module_permits", array(text())),
                    ("enclosing_declared_effects", nullable(array(text()))),
                ]),
            ),
            ("facets", references),
            ("full_context_method", json!({"const":"hole/query"})),
            ("materializable", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            (
                "validation",
                json!({"const":"pending_fill_full_source_replay"}),
            ),
            (
                "evidence_class",
                json!({"const":"descriptive_context_not_candidate_validation"}),
            ),
        ],
    );
    documents.insert(format!("urn:{PROJECT_HOLE_SUMMARY_SCHEMA}"), summary);
    let scope = object(vec![
        ("id", text()),
        ("name", text()),
        ("type_id", text()),
        ("ownership", ownership()),
        ("mutable", nullable(json!({"type":"boolean"}))),
    ]);
    let call = object(vec![
        ("id", text()),
        ("binding", text()),
        ("return_type_id", text()),
        (
            "parameters",
            array(object(vec![
                ("name", text()),
                ("type_id", text()),
                ("ownership", ownership()),
            ])),
        ),
        ("effects", array(text())),
        ("within_effect_budget", json!({"type":"boolean"})),
        (
            "basis",
            json!({"const":"existing_local_or_authenticated_import_binding"}),
        ),
        ("admission", json!({"const":"requires_fill_revalidation"})),
    ]);
    let alternatives = [
        ("scope", scope),
        ("calls", call),
        ("obligations", text()),
        ("constructors", text()),
    ]
    .into_iter()
    .map(|(facet, row)| {
        let mut items = array(row);
        items["maxItems"] = json!(64);
        object(vec![
            ("schema", json!({"const":PROJECT_HOLE_PAGE_SCHEMA})),
            ("draft_revision", digest()),
            ("hole_id", text()),
            ("context_revision", digest()),
            ("facet", json!({"const":facet})),
            ("reference", digest()),
            ("total", index()),
            ("offset", index()),
            ("next_offset", nullable(index())),
            ("items", items),
            ("source_authority", json!({"const":false})),
        ])
    })
    .collect::<Vec<_>>();
    documents.insert(
        format!("urn:{PROJECT_HOLE_PAGE_SCHEMA}"),
        json!({
            "$id":format!("urn:{PROJECT_HOLE_PAGE_SCHEMA}"),
            "$schema":"https://json-schema.org/draft/2020-12/schema",
            "oneOf":alternatives,
        }),
    );
    documents
}

fn draft_expression_catalog() -> Value {
    let mut scope = array(object(vec![
        ("value_id", text()),
        ("name", text()),
        ("type", text()),
        ("ownership", ownership()),
        ("mutable", json!({"type":"boolean"})),
    ]));
    scope["maxItems"] = json!(16384);
    let mut expressions = array(object(vec![
        ("expression_id", text()),
        ("phase", json!({"enum":["body","requires","ensures"]})),
        ("kind", text()),
        ("expected_type", text()),
        ("ownership", ownership()),
        (
            "source_span",
            object(vec![
                ("start", uint()),
                ("end", uint()),
                ("line", uint()),
                ("column", uint()),
            ]),
        ),
        ("replaceable", json!({"type":"boolean"})),
        ("reason", text()),
        ("scope", scope),
    ]));
    expressions["maxItems"] = json!(4096);
    document(
        PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
        vec![
            ("draft_revision", digest()),
            ("last_valid_revision", digest()),
            ("last_valid_candidate_digest", digest()),
            ("target", text()),
            ("region", json!({"enum":["body","contract"]})),
            (
                "source",
                object(vec![
                    ("path", text()),
                    ("module", text()),
                    ("source_revision", digest()),
                    ("source_digest", digest()),
                ]),
            ),
            ("declared_effect_budget", array(text())),
            ("expressions", expressions),
            (
                "limits",
                object(vec![
                    ("max_expressions", json!({"const":4096})),
                    ("max_depth", json!({"const":256})),
                    ("max_scope_facts", json!({"const":16384})),
                    (
                        "max_bytes",
                        json!({"const":MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES}),
                    ),
                ]),
            ),
            ("materializable", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            (
                "validation",
                json!({"const":"pending_fill_full_source_replay"}),
            ),
            (
                "evidence_class",
                json!({"const":"last_valid_expression_inventory_not_draft_validation"}),
            ),
            (
                "selection_admission",
                json!({"const":"requires_hole_open_validation"}),
            ),
            ("nonclaims", array(text())),
        ],
    )
}
