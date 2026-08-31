//! V5-only recovery of unresolved typed drafts; no complete candidate retention.
use super::*;
use crate::project::{
    ProjectCandidateDraft, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES,
    PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
};

const METHODS: &[Method] = &[
    Method {
        name: "hole/recovery-export",
        operation: Operation::VNext(Action::DraftRecoveryExport),
        parameters: &[
            REVISION,
            Parameter {
                name: "draft_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES),
                required: false,
            },
            Parameter {
                name: "chunk_bytes",
                kind: ParameterKind::Integer(1024, 65536),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-draft-recovery-chunk.v1",
    },
    Method {
        name: "hole/recovery-restore",
        operation: Operation::VNext(Action::DraftRecoveryRestore),
        parameters: &[
            REVISION,
            Parameter {
                name: "capsule",
                kind: ParameterKind::Object(PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA),
                required: true,
            },
        ],
        query: false,
        payload_schema: "semaprax.image-draft-handle.v1",
    },
];

pub(super) fn methods() -> &'static [Method] {
    METHODS
}

pub(super) fn prepare(
    action: Action,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<(Value, candidates::Mutation), Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G232",
            "draft recovery image revision is stale",
        ));
    }
    match action {
        Action::DraftRecoveryExport => {
            let draft = registry.draft_value(text(params, "draft_revision"))?;
            Ok((export_for_draft(params, draft)?, candidates::Mutation::None))
        }
        Action::DraftRecoveryRestore => {
            // The frame codec bounds this structured input before construction.
            // Compiler restore owns nested source/history and hole replay.
            let mut capsule = params["capsule"].clone();
            capsule.sort_all_objects();
            let bytes = format!("{capsule}\n");
            let draft = ProjectCandidateDraft::restore(
                Arc::clone(image.revision()),
                image.revision().project_revision(),
                bytes.as_bytes(),
            )?;
            let summary: Value = serde_json::from_str(draft.summary(draft.draft_digest())?)
                .map_err(|_| failure("SPX-G230", "restored draft summary is not compiler JSON"))?;
            let source_candidate =
                summary["last_valid_candidate_digest"]
                    .as_str()
                    .ok_or_else(|| {
                        failure(
                            "SPX-G230",
                            "restored draft lacks its last valid candidate binding",
                        )
                    })?;
            // This association does not install or expose a complete candidate.
            // Existing query/fill/complete operate directly on the opaque draft.
            registry.retain_recovered_draft(draft, source_candidate)
        }
        _ => Err(failure("SPX-G230", "unsupported draft recovery action")),
    }
}

pub(super) fn export_for_draft(
    params: &Map<String, Value>,
    draft: &ProjectCandidateDraft,
) -> Result<Value, Vec<Diagnostic>> {
    let capsule = draft.recovery_capsule()?;
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16384);
    if !(1024..=65536).contains(&chunk_bytes)
        || offset > capsule.len()
        || !capsule.is_char_boundary(offset)
    {
        return Err(failure(
            "SPX-G230",
            "draft recovery chunk is outside its bounded UTF8 capsule",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(capsule.len());
    while !capsule.is_char_boundary(end) {
        end -= 1;
    }
    Ok(json!({
        "schema":"semaprax.image-draft-recovery-chunk.v1",
        "draft_revision":draft.draft_digest(),
        "capsule_schema":PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
        "offset":offset,"total_bytes":capsule.len(),"chunk":&capsule[offset..end],
        "next_offset":(end<capsule.len()).then_some(end),
        "source_authority":false,"materializable":false,
    }))
}
