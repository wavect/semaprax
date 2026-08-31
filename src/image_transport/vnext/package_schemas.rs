//! Closed package report shapes, independent of Project image identity.
use super::*;

fn coordinate() -> Value {
    object(vec![("package", text()), ("version", text())])
}
fn imports() -> Value {
    json!({"type":"array","maxItems":256,"items":object(vec![
        ("dependent",coordinate()),("dependency",coordinate()),("target",text()),
        ("alias",text()),("ordinal",uint()),
    ])})
}
pub(super) fn documents() -> BTreeMap<String, Value> {
    let summary = "semaprax.package-semantic-summary.v1";
    let consumers = "semaprax.package-semantic-consumers.v1";
    BTreeMap::from([
        (
            format!("urn:{summary}"),
            document(
                summary,
                vec![
                    ("graph_revision", digest()),
                    ("source_capsule_digest", digest()),
                    ("source_set_digest", digest()),
                    ("link_digest", digest()),
                    ("root_package", coordinate()),
                    (
                        "packages",
                        json!({"type":"array","minItems":2,"maxItems":4,"items":object(vec![
                            ("coordinate",coordinate()),("subject_digest",digest()),("report_digest",digest()),
                            ("interface_digest",digest()),("interface_source_revision",digest()),
                            ("source_revision",digest()),("source_digest",digest()),
                            ("source_bytes",json!({"type":"integer","minimum":0,"maximum":1048576})),
                            ("exports",json!({"type":"array","maxItems":4096,"items":text()})),
                        ])}),
                    ),
                    ("imports", imports()),
                    (
                        "counts",
                        object(vec![
                            (
                                "packages",
                                json!({"type":"integer","minimum":2,"maximum":4}),
                            ),
                            (
                                "interface_functions",
                                json!({"type":"integer","minimum":0,"maximum":4096}),
                            ),
                            (
                                "imports",
                                json!({"type":"integer","minimum":0,"maximum":256}),
                            ),
                            (
                                "cross_package_calls",
                                json!({"type":"integer","minimum":0,"maximum":65536}),
                            ),
                        ]),
                    ),
                    ("project_association", json!({"const":"none"})),
                    (
                        "evidence_owner",
                        json!({"const":"verified_package_source_capsule_and_workspace_calls"}),
                    ),
                    ("source_authority", json!({"const":false})),
                    ("execution", json!({"const":false})),
                    ("publication_authority", json!({"const":false})),
                    ("nonclaims", array(text())),
                ],
            ),
        ),
        (
            format!("urn:{consumers}"),
            document(
                consumers,
                vec![
                    ("graph_revision", digest()),
                    ("source_capsule_digest", digest()),
                    ("provider", coordinate()),
                    ("target", text()),
                    ("provider_source_revision", digest()),
                    ("provider_source_digest", digest()),
                    ("imports", imports()),
                    (
                        "calls",
                        json!({"type":"array","maxItems":65536,"items":object(vec![
                            ("caller_package",coordinate()),("target_package",coordinate()),
                            ("caller",text()),("target",text()),("caller_source_revision",digest()),
                            ("target_source_revision",digest()),("site",text()),("expression",text()),
                            ("ast_path",text()),("alias",text()),("ordinal",uint()),
                        ])}),
                    ),
                    ("project_association", json!({"const":"none"})),
                    ("source_authority", json!({"const":false})),
                    ("execution", json!({"const":false})),
                    ("publication_authority", json!({"const":false})),
                    ("nonclaims", array(text())),
                ],
            ),
        ),
    ])
}
