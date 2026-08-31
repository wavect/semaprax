//! Host-only archive handoff. Serialized candidates never enter the registry
//! until independent archive replay and both live snapshot checks succeed.
use super::{candidates, failure, Arc, Diagnostic, VNextSession};
use crate::project::{ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraftArchive};

impl VNextSession {
    /// Recover a historical unfinished draft before accepting the first frame.
    /// Only the draft is retained; its prior valid candidate, approvals and
    /// source authority are not installed. The live manifest must match.
    pub fn restore_draft_archive(
        &mut self,
        bytes: &[u8],
        expected_archive: &str,
        expected_draft: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        if self.started || self.terminal || !self.policy.candidate_prepare {
            return Err(failure(
                "SPX-G303",
                "draft archive retention requires startup and the host candidate-preparation grant",
            ));
        }
        let registry = &self.registry;
        let (handle, mutation) = self.snapshot.with_authenticated_request(|snapshot| {
            let draft = ProjectCandidateDraftArchive::restore_for_manifest(
                bytes,
                expected_archive,
                expected_draft,
                snapshot.manifest(),
            )?;
            let summary: serde_json::Value =
                serde_json::from_str(draft.summary(draft.draft_digest())?).map_err(|_| {
                    failure(
                        "SPX-G303",
                        "archived draft summary serialization is invalid",
                    )
                })?;
            let source_candidate = summary["last_valid_candidate_digest"]
                .as_str()
                .ok_or_else(|| failure("SPX-G303", "archived draft candidate binding is absent"))?;
            let (handle, mutation) = registry.retain_recovered_draft(draft, source_candidate)?;
            registry.admit(&mutation)?;
            let handle = serde_json::to_string(&handle)
                .map_err(|_| failure("SPX-G303", "archived draft handle serialization failed"))?;
            Ok((handle, mutation))
        })?;
        // Complete source replay, response preparation, registry admission and
        // both live-source authentications precede this only state change.
        self.registry.commit(mutation);
        Ok(handle)
    }

    /// Export a retained draft with its canonical original source and history.
    /// Persistence remains a host decision; no unresolved source is emitted.
    pub fn export_draft_archive(
        &mut self,
        expected_image: &str,
        expected_draft: &str,
    ) -> Result<ProjectCandidateDraftArchive, Vec<Diagnostic>> {
        if self.terminal || !self.policy.candidate_prepare {
            return Err(failure(
                "SPX-G303",
                "draft archive export requires a live candidate-preparation session",
            ));
        }
        if self.image.image_digest() != expected_image {
            return Err(failure("SPX-G282", "v5 expected image revision is stale"));
        }
        let registry = &self.registry;
        self.snapshot.with_authenticated_request(|_| {
            ProjectCandidateDraftArchive::prepare(
                registry.draft_value(expected_draft)?,
                expected_draft,
            )
        })
    }

    /// Restore a self-contained historical candidate before the first frame.
    /// Returns the ordinary candidate-handle JSON, not a publication receipt.
    /// Archive replay owns source/manifest rebuilding and exact history replay;
    /// the live session owns authentication and bounded registry admission.
    pub fn restore_candidate_archive(
        &mut self,
        bytes: &[u8],
        expected_archive: &str,
        expected_candidate: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.install_archived_candidate(expected_candidate, || {
            ProjectCandidateArchive::restore(bytes, expected_archive, expected_candidate)
        })
    }

    /// Accept an opaque compiler-created candidate, including a candidate
    /// returned by independently replaying an archive store. This does not
    /// deserialize unchecked state or certify archive provenance. The host
    /// must load its archive through the ordinary archive/store verifier first.
    pub fn retain_archived_candidate(
        &mut self,
        candidate: ProjectCandidate,
        expected_candidate: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.install_archived_candidate(expected_candidate, || Ok(candidate))
    }

    fn install_archived_candidate(
        &mut self,
        expected_candidate: &str,
        prepare: impl FnOnce() -> Result<ProjectCandidate, Vec<Diagnostic>>,
    ) -> Result<String, Vec<Diagnostic>> {
        if self.started || self.terminal || !self.policy.candidate_prepare {
            return Err(failure(
                "SPX-G303",
                "candidate archive retention requires startup and the host candidate-preparation grant",
            ));
        }
        let registry = &self.registry;
        let (handle, mutation) = self.snapshot.with_authenticated_request(|snapshot| {
            let candidate = prepare()?;
            if candidate.candidate_digest() != expected_candidate {
                return Err(failure(
                    "SPX-G224",
                    "archived candidate expectation is stale",
                ));
            }
            if candidate.base_revision().manifest().to_canonical_toml()
                != snapshot.manifest().to_canonical_toml()
                || candidate.revision().manifest().to_canonical_toml()
                    != snapshot.manifest().to_canonical_toml()
            {
                return Err(failure(
                    "SPX-G303",
                    "archived candidate must retain the session's canonical Project manifest",
                ));
            }
            let (handle, mutation) = candidates::retain_candidate(Arc::new(candidate))?;
            registry.admit(&mutation)?;
            let handle = serde_json::to_string(&handle).map_err(|_| {
                failure("SPX-G303", "archived candidate handle serialization failed")
            })?;
            Ok((handle, mutation))
        })?;
        // No candidate, draft, approval, or Git state is changed before the
        // post-replay physical authentication above has completed successfully.
        self.registry.commit(mutation);
        Ok(handle)
    }

    /// Export one retained candidate as an independently replayable archive.
    /// Available to an authorized host after frames as well as at startup;
    /// this performs no archive persistence or source publication.
    pub fn export_candidate_archive(
        &mut self,
        expected_image: &str,
        expected_candidate: &str,
    ) -> Result<ProjectCandidateArchive, Vec<Diagnostic>> {
        if self.terminal || !self.policy.candidate_prepare {
            return Err(failure(
                "SPX-G303",
                "candidate archive export requires a live candidate-preparation session",
            ));
        }
        if self.image.image_digest() != expected_image {
            return Err(failure("SPX-G282", "v5 expected image revision is stale"));
        }
        let registry = &self.registry;
        self.snapshot.with_authenticated_request(|_| {
            let candidate = registry.candidate(expected_candidate)?;
            ProjectCandidateArchive::prepare(candidate, expected_candidate)
        })
    }
}
