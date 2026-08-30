//! Compiler-prepared data handed to an explicitly selected publication host.

use super::carrier::{decode_carrier_artifacts, ReplayedNpmArtifacts};
use super::ProjectNpmBuild;
use crate::diagnostic::Diagnostic;

/// Immutable exact artifacts for an owned-profile Project publication.
/// Only a live Project build constructs this plan. Artifact data does not grant
/// filesystem authority; the explicitly supplied host owns that operation.
pub struct ProjectNpmPublication {
    project_schema: &'static str,
    artifacts: ReplayedNpmArtifacts,
}

impl ProjectNpmPublication {
    pub(in crate::project) fn prepare(
        build: &ProjectNpmBuild,
        project_schema: &'static str,
    ) -> Result<Self, Diagnostic> {
        build.verify()?;
        let artifacts = decode_carrier_artifacts(build.envelope(), build.max_bytes())?;
        Ok(Self {
            project_schema,
            artifacts,
        })
    }

    pub fn project_schema(&self) -> &str {
        self.project_schema
    }

    /// Exact canonical publication order, with no caller-supplied path names.
    pub fn artifacts(&self) -> impl ExactSizeIterator<Item = (&str, &[u8])> {
        self.artifacts
            .as_slice()
            .iter()
            .map(|artifact| (artifact.path, artifact.bytes.as_slice()))
    }
}
