//! Source-backed current-base draft recovery under the existing candidate grant.
//! Restores only a draft; no candidate, HIR, approval or source authority.
use super::*;
use crate::project::{
    ProjectCandidateDraftArchive, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES,
    PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
};

const METHODS: &[Method] = &[
    Method {
        name: "hole/archive-export",
        operation: Operation::VNext(Action::DraftArchiveExport),
        parameters: &[
            REVISION,
            Parameter {
                name: "draft_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES),
                required: false,
            },
            Parameter {
                name: "chunk_bytes",
                kind: ParameterKind::Integer(1024, 65536),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-draft-archive-chunk.v1",
    },
    Method {
        name: "hole/archive-restore",
        operation: Operation::VNext(Action::DraftArchiveRestore),
        parameters: &[
            REVISION,
            Parameter {
                name: "archive",
                kind: ParameterKind::Object(PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA),
                required: true,
            },
            Parameter {
                name: "archive_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "draft_revision",
                kind: ParameterKind::Digest,
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
        return Err(failure("SPX-G232", "draft archive image revision is stale"));
    }
    match action {
        Action::DraftArchiveExport => {
            let draft = registry.draft_value(text(params, "draft_revision"))?;
            let archive = ProjectCandidateDraftArchive::prepare(draft, draft.draft_digest())?;
            let bytes = archive.to_json();
            let offset = number(params, "offset", 0);
            let chunk_bytes = number(params, "chunk_bytes", 16384);
            if !(1024..=65536).contains(&chunk_bytes)
                || offset > bytes.len()
                || !bytes.is_char_boundary(offset)
            {
                return Err(failure(
                    "SPX-G230",
                    "draft archive chunk is outside its bounded UTF8 archive",
                ));
            }
            let mut end = offset.saturating_add(chunk_bytes).min(bytes.len());
            while !bytes.is_char_boundary(end) {
                end -= 1;
            }
            Ok((
                json!({
                    "schema":"semaprax.image-draft-archive-chunk.v1",
                    "archive_schema":PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
                    "image_revision":image.image_digest(),"archive_revision":archive.archive_digest(),"draft_revision":archive.draft_digest(),
                    "offset":offset,"total_bytes":bytes.len(),"chunk":&bytes[offset..end],"next_offset":(end<bytes.len()).then_some(end),
                    "source_authority":false,"approval_authority":false,"trusted_hir":false,"materializable":false,
                }),
                candidates::Mutation::None,
            ))
        }
        Action::DraftArchiveRestore => {
            // The ordinary 64KiB frame/closed-request codec runs first. Nested
            // strings retain exact bytes; only outer object keys canonicalize.
            let mut archive = params["archive"].clone();
            archive.sort_all_objects();
            let bytes = format!("{archive}\n");
            let draft = ProjectCandidateDraftArchive::restore_for_base(
                bytes.as_bytes(),
                text(params, "archive_revision"),
                text(params, "draft_revision"),
                image.revision(),
            )?;
            let summary: Value = serde_json::from_str(draft.summary(draft.draft_digest())?)
                .map_err(|_| {
                    failure(
                        "SPX-G230",
                        "restored archive draft summary is not compiler JSON",
                    )
                })?;
            let source_candidate =
                summary["last_valid_candidate_digest"]
                    .as_str()
                    .ok_or_else(|| {
                        failure(
                            "SPX-G230",
                            "restored archive draft lacks its valid candidate binding",
                        )
                    })?;
            registry.retain_recovered_draft(draft, source_candidate)
        }
        _ => Err(failure("SPX-G230", "unsupported draft archive action")),
    }
}
