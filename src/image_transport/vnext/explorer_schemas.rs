use super::{digest, document, nullable, object, text, uint};
use serde_json::{json, Value};

pub(super) fn document_schema() -> Value {
    let subject = object(vec![
        ("kind", json!({"enum":["image","candidate"]})),
        ("image_revision", digest()),
        ("project_revision", digest()),
        ("workspace_revision", digest()),
        ("project_graph_digest", digest()),
        ("candidate_revision", nullable(digest())),
        ("side", json!({"enum":["current","base","candidate"]})),
    ]);
    let query = object(vec![
        ("direction", json!({"enum":["forward","reverse","both"]})),
        ("depth", uint()),
        ("max_nodes", uint()),
        ("max_bytes", uint()),
    ]);
    let inventory = object(vec![
        (
            "view",
            json!({"enum":["modules","declarations","relations","frontier"]}),
        ),
        ("handle", digest()),
        ("total_items", uint()),
    ]);
    let common = vec![
        ("subject", subject),
        ("mode", json!({"enum":["overview","context","impact"]})),
        ("target", nullable(text())),
        ("query", query),
        ("artifact_digest", digest()),
        ("truncation", json!({})),
        ("coverage", json!({})),
        ("source_authority", json!({"const":false})),
        ("execution", json!({"const":false})),
        ("publication_authority", json!({"const":false})),
        ("nonclaims", json!({"type":"array","items":text()})),
    ];
    let mut summary = common.clone();
    summary.extend([
        ("kind", json!({"const":"summary"})),
        (
            "inventories",
            json!({"type":"array","minItems":4,"maxItems":4,"items":inventory}),
        ),
    ]);
    let mut page = common;
    page.extend([
        ("kind", json!({"const":"page"})),
        (
            "view",
            json!({"enum":["modules","declarations","relations","frontier"]}),
        ),
        ("handle", digest()),
        ("cursor", nullable(text())),
        ("offset", uint()),
        ("total_items", uint()),
        (
            "page_size",
            json!({"type":"integer","minimum":1,"maximum":128}),
        ),
        (
            "max_bytes",
            json!({"type":"integer","minimum":1024,"maximum":524288}),
        ),
        ("next_cursor", nullable(text())),
        (
            "items",
            json!({"type":"array","maxItems":128,"items":json!({})}),
        ),
    ]);
    let value = json!({"$id":"urn:semaprax.explorer-view.v1","$schema":"https://json-schema.org/draft/2020-12/schema","oneOf":[document("semaprax.explorer-view.v1",summary),document("semaprax.explorer-view.v1",page)]});
    value
}
