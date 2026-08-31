//! Exact source review over a replayed immutable candidate, without authority.
use std::sync::Arc;

use serde_json::json;

use super::{capacity, invalid, wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use crate::project::MAX_SOURCES;

pub const PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA: &str =
    "semaprax.project-candidate-source-review.v1";
pub const MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES: usize = 16 * 1024 * 1024;
const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-source-review.v1\0";
const DIFF_DOMAIN: &[u8] = b"semaprax.candidate.source-diff.v1\0";

impl ProjectCandidate {
    /// Return the complete changed-source review after ordinary independent
    /// source-history replay. Exact base and replacement texts come from the
    /// rebuilt revision, never a parsed heterogeneous report or live files.
    ///
    /// The closed report has at most sixteen changed files and sixteen MiB of
    /// canonical JSON including its final LF. The digest binds the canonical
    /// report without its own digest field. It is evidence, not an edit, commit
    /// approval, saved-editor-buffer claim, or authority to resolve file paths.
    pub fn source_review(&self, expected_candidate: &str) -> Result<String, Vec<Diagnostic>> {
        self.source_review_shared(expected_candidate)
            .map(|report| report.to_string())
    }

    /// One bounded immutable report per candidate, shared by chunk readers.
    /// Selector authentication precedes cache access, including cached failures.
    /// Initialization replays into fresh candidates whose empty caches are not
    /// queried; no source-review initialization can recursively wait on itself.
    pub(crate) fn source_review_shared(
        &self,
        expected_candidate: &str,
    ) -> Result<Arc<str>, Vec<Diagnostic>> {
        self.require_candidate(expected_candidate)?;
        self.source_review_cache
            .get_or_init(|| self.build_source_review().map(Arc::<str>::from))
            .clone()
    }

    fn build_source_review(&self) -> Result<String, Vec<Diagnostic>> {
        if self.base.sources().len() > MAX_SOURCES || self.revision.sources().len() > MAX_SOURCES {
            return Err(capacity(
                "candidate source review inventory exceeds sixteen files",
            ));
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        replay.require_candidate(self.candidate_digest())?;
        if replay.base.manifest().to_canonical_toml()
            != replay.revision.manifest().to_canonical_toml()
            || replay.base.sources().len() != replay.revision.sources().len()
            || replay.revision.sources().len() != self.revision.sources().len()
        {
            return Err(invalid("candidate source review requires the unchanged complete manifest and source inventory"));
        }

        let mut files = Vec::new();
        let mut previous = None;
        let mut remaining = MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES;
        for ((before, after), retained) in replay
            .base
            .sources()
            .iter()
            .zip(replay.revision.sources())
            .zip(self.revision.sources())
        {
            let path = before.path();
            if path != after.path()
                || path != retained.path()
                || previous.is_some_and(|previous| previous >= path)
                || path.len() > 240
                || !path.ends_with(".spx")
                || !crate::workspace::evidence_path_is_valid(path)
                || after.source() != retained.source()
                || after.source_digest() != retained.source_digest()
            {
                return Err(invalid("candidate source review inventory or replayed source differs from its retained revision"));
            }
            previous = Some(path);
            if crate::review::source_digest(before.source().as_bytes()) != before.source_digest()
                || crate::review::source_digest(after.source().as_bytes()) != after.source_digest()
            {
                return Err(invalid(
                    "candidate source review source digest is inconsistent",
                ));
            }
            if before.source() == after.source() {
                continue;
            }
            // A lower bound prevents cloning source text that cannot fit the
            // report even before JSON escaping and the diff are considered.
            let text_bytes = before
                .source()
                .len()
                .checked_add(after.source().len())
                .ok_or_else(|| capacity("candidate source review source size overflow"))?;
            if text_bytes > remaining {
                return Err(capacity("candidate source review texts exceed sixteen MiB"));
            }
            let diff = wire::source_diff(path, before.source(), after.source())?;
            if diff.len() > remaining.saturating_sub(text_bytes) {
                return Err(capacity("candidate source review diff exceeds sixteen MiB"));
            }
            let row = json!({
                "path":path,
                "base_source":before.source(),
                "candidate_source":after.source(),
                "base_digest":before.source_digest(),
                "candidate_digest":after.source_digest(),
                "source_diff_digest":wire::digest(DIFF_DOMAIN,diff.as_bytes()),
                "source_diff":diff,
            });
            // The row LF costs the same one byte as its array separator; the
            // final complete render independently checks root and digest bytes.
            let encoded = wire::render(row.clone(), remaining)?;
            remaining -= encoded.len();
            files.push(row);
        }
        let mut report = json!({
            "schema":PROJECT_CANDIDATE_SOURCE_REVIEW_SCHEMA,
            "base_project_revision":replay.base.project_revision(),
            "candidate_project_revision":replay.revision.project_revision(),
            "candidate_revision":replay.candidate_digest(),
            "source_authority":false,
            "files":files,
        });
        let core = wire::render(report.clone(), MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES)?;
        report["report_revision"] = json!(wire::digest(REPORT_DOMAIN, core.as_bytes()));
        wire::render(report, MAX_PROJECT_CANDIDATE_SOURCE_REVIEW_BYTES)
    }
}
