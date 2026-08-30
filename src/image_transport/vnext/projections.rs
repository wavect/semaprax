//! Host-selected pathless builds and source-bound target projection queries.
use super::*;
use crate::project::{ImageArtifactKind, MAX_IMAGE_ARTIFACT_BUILD_BYTES};

const METHODS: &[Method] = &[
    Method {
        name: "image/target-admission",
        operation: Operation::VNext(Action::Targets),
        parameters: &[
            REVISION,
            TARGET,
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, 1024 * 1024),
                required: false,
            },
            Parameter {
                name: "chunk_bytes",
                kind: ParameterKind::Integer(1024, 65536),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-target-admission-chunk.v1",
    },
    Method {
        name: "candidate/build",
        operation: Operation::VNext(Action::Build),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "kind",
                kind: ParameterKind::Choice(&["web", "npm"]),
                required: true,
            },
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, 1024 * 1024),
                required: false,
            },
            Parameter {
                name: "chunk_bytes",
                kind: ParameterKind::Integer(1024, 65536),
                required: false,
            },
        ],
        query: false,
        payload_schema: "semaprax.image-artifact-projection-chunk.v1",
    },
];
pub(super) fn methods(build_enabled: bool) -> Vec<&'static Method> {
    METHODS
        .iter()
        .filter(|method| build_enabled || method.name != "candidate/build")
        .collect()
}
pub(super) fn prepare(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(vec![Diagnostic::io(
            "SPX-G221",
            "target/artifact query image revision is stale",
        )]);
    }
    let (schema, report_schema, report) = match action {
        Action::Targets => (
            "semaprax.image-target-admission-chunk.v1",
            crate::project::IMAGE_TARGET_ADMISSION_SCHEMA,
            image.target_admission(image.image_digest(), text(params, "target"))?,
        ),
        Action::Build => {
            let candidate = registry.candidate(text(params, "candidate_revision"))?;
            let capsule = candidate.recovery_capsule()?;
            let replay = crate::project::ProjectCandidate::restore(
                Arc::clone(candidate.base_revision()),
                candidate.base_revision().project_revision(),
                capsule.as_bytes(),
            )?;
            let selected = ProjectSemanticImage::derive(
                Arc::clone(replay.revision()),
                replay.revision().project_revision(),
            )?;
            let kind = match text(params, "kind") {
                "web" => ImageArtifactKind::Web,
                "npm" => ImageArtifactKind::Npm,
                _ => return Err(vec![Diagnostic::io("SPX-G290", "unknown artifact kind")]),
            };
            (
                "semaprax.image-artifact-projection-chunk.v1",
                crate::project::IMAGE_ARTIFACT_PROJECTION_SCHEMA,
                selected.artifact_projection(
                    selected.image_digest(),
                    kind,
                    MAX_IMAGE_ARTIFACT_BUILD_BYTES,
                )?,
            )
        }
        _ => {
            return Err(vec![Diagnostic::io(
                "SPX-G290",
                "unknown target/artifact operation",
            )])
        }
    };
    let offset = number(params, "offset", 0);
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(vec![Diagnostic::io(
            "SPX-G290",
            "artifact report offset is outside its UTF-8 boundary",
        )]);
    }
    let mut end = offset
        .saturating_add(number(params, "chunk_bytes", 16384))
        .min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < report.len() {
        return Err(vec![Diagnostic::io(
            "SPX-G291",
            "artifact chunk cannot make progress",
        )]);
    }
    Ok(
        json!({"schema":schema,"report_schema":report_schema,"image_revision":image.image_digest(),
        "candidate_revision":params.get("candidate_revision"),"target":params.get("target"),"kind":params.get("kind"),
        "offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),
        "source_authority":false,"artifact_materialization":false,"target_execution":false}),
    )
}
