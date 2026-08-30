//! Immutable declaration dependency facts, shared with the host read batch.
use super::*;
use crate::project::{
    IMAGE_DECLARATION_DEPENDENCIES_SCHEMA, MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES,
};

const METHOD: Method = Method {
    name: "image/dependencies",
    operation: Operation::VNext(Action::Dependencies),
    parameters: &[
        REVISION,
        TARGET,
        Parameter {
            name: "offset",
            kind: ParameterKind::Integer(0, MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES),
            required: false,
        },
        Parameter {
            name: "chunk_bytes",
            kind: ParameterKind::Integer(1024, 65536),
            required: false,
        },
    ],
    query: true,
    payload_schema: "semaprax.image-declaration-dependencies-chunk.v1",
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G221",
            "dependency query image revision is stale",
        ));
    }
    let target = text(params, "target");
    let report = image.declaration_dependencies(image.image_digest(), target)?;
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if report.len() > MAX_IMAGE_DECLARATION_DEPENDENCIES_BYTES {
        return Err(failure(
            "SPX-G220",
            "dependency report exceeds its transport byte bound",
        ));
    }
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > report.len()
        || !report.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G219",
            "dependency chunk is outside its bounded UTF8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-declaration-dependencies-chunk.v1",
        "report_schema":IMAGE_DECLARATION_DEPENDENCIES_SCHEMA,"image_revision":image.image_digest(),"target":target,
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],
        "next_offset":(end<report.len()).then_some(end),"source_authority":false,
    }))
}
