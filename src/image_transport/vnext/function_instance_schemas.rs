//! Closed navigation envelopes; heterogeneous facet items remain unbundled.
use super::{array, digest, document, nullable, object, text, uint};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn documents() -> BTreeMap<String, Value> {
    let facet = json!({"enum":["signature","contracts","callers","ownership","loans","cleanup","relationships","data-access","unsafe-boundaries"]});
    let span = object(vec![
        ("start", uint()),
        ("end", uint()),
        ("line", uint()),
        ("column", uint()),
    ]);
    let nonclaims = json!({"const":[
        "no_source_or_commit_authority",
        "no_target_execution_or_test_coverage",
        "retained_instances_not_all_possible_instantiations",
        "template_spans_are_source_provenance_not_executed_sites",
        "no_external_or_dynamic_callers",
    ]});
    let instance = object(vec![
        ("instance_id", text()),
        ("type_arguments", array(text())),
        ("parameter_count", uint()),
        ("return_type_id", text()),
        ("effects", array(text())),
        ("requires_count", uint()),
        ("ensures_count", uint()),
        (
            "facets",
            json!({"type":"array","minItems":9,"maxItems":9,"items":object(vec![
                ("facet",facet.clone()),("handle",digest()),
            ])}),
        ),
    ]);
    let common = || {
        vec![
            ("image_revision", digest()),
            ("project_revision", digest()),
            ("template_id", text()),
            ("path", text()),
            ("module", text()),
            ("source_revision", digest()),
            ("source_digest", digest()),
            ("template_span", span.clone()),
            ("handle", digest()),
            (
                "offset",
                json!({"type":"integer","minimum":0,"maximum":65536}),
            ),
            (
                "next_cursor",
                nullable(json!({"type":"string","maxLength":100,"x-max-utf8-bytes":100})),
            ),
            (
                "evidence_class",
                json!({"const":"descriptive_projection_of_retained_generic_instance_hir"}),
            ),
            ("source_authority", json!({"const":false})),
            ("target_execution", json!({"const":false})),
            ("nonclaims", nonclaims.clone()),
        ]
    };
    let mut listing = common();
    listing.extend([
        ("name", text()),
        ("type_parameter_count", uint()),
        (
            "total_instances",
            json!({"type":"integer","minimum":0,"maximum":65536}),
        ),
        (
            "instances",
            json!({"type":"array","maxItems":128,"items":instance}),
        ),
    ]);
    let mut page = common();
    page.extend([
        ("instance_id", text()),
        ("type_arguments", array(text())),
        ("facet", facet),
        ("total_items", json!({"type":"integer","minimum":0,"maximum":65536})),
        ("items", json!({"type":"array","maxItems":128,"items":{"$ref":"urn:semaprax.image-instance-facet-item.v1"}})),
    ]);
    BTreeMap::from([
        (
            "urn:semaprax.image-function-instances.v1".into(),
            document("semaprax.image-function-instances.v1", listing),
        ),
        (
            "urn:semaprax.image-instance-facet.v1".into(),
            document("semaprax.image-instance-facet.v1", page),
        ),
    ])
}
