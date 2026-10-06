//! Single-use extraction from an authenticated semantic workspace read.

use super::{
    invariant, unlock_file, unlock_with_diagnostics, WorkspaceSemanticReadAuthority,
    WorkspaceSemanticSource,
};
use crate::diagnostic::Diagnostic;

impl WorkspaceSemanticReadAuthority {
    pub(crate) fn workspace_revision(&self) -> &str {
        self.guard.snapshot.workspace_revision()
    }

    /// Moves the authenticated sources out, paired with the metadata of the
    /// same snapshot, in snapshot order.
    ///
    /// Extraction is single-use. A second call is refused before any record is
    /// built, so metadata is never paired with already-emptied source bytes.
    pub(crate) fn take_sources(&mut self) -> Result<Vec<WorkspaceSemanticSource>, Vec<Diagnostic>> {
        if std::mem::replace(&mut self.sources_taken, true) {
            return Err(invariant(
                "semantic workspace sources were already consumed",
            ));
        }
        Ok(self
            .guard
            .snapshot
            .files
            .iter_mut()
            .map(|file| WorkspaceSemanticSource {
                path: file.path.clone(),
                source_graph_schema: file.source_graph_schema.clone(),
                source_revision: file.source_revision.clone(),
                source_digest: file.source_digest.clone(),
                source: std::mem::take(&mut file.source),
            })
            .collect())
    }

    pub(crate) fn take_graph(
        &mut self,
    ) -> Result<crate::workspace_graph::WorkspaceGraphBuild, Vec<Diagnostic>> {
        self.guard
            .semantic_graph
            .take()
            .ok_or_else(|| invariant("semantic workspace graph was already consumed"))
    }

    pub(crate) fn manifest_bytes(&self) -> usize {
        self.guard.snapshot.manifest_bytes
    }

    pub(crate) fn retained_generations(&self) -> usize {
        self.guard.snapshot.retained_generations
    }

    pub(crate) fn staging_attempts(&self) -> usize {
        self.guard.snapshot.staging_attempts
    }

    pub(crate) fn finish<T>(
        mut self,
        result: Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        let value = match result {
            Ok(value) => value,
            Err(diagnostics) => {
                return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
            }
        };
        if self.guard.semantic_graph.is_some() {
            return Err(unlock_with_diagnostics(
                &self.guard.lock,
                invariant("semantic workspace graph authority was not consumed exactly once"),
            ));
        }
        if let Err(diagnostics) = self.guard.recheck() {
            return Err(unlock_with_diagnostics(&self.guard.lock, diagnostics));
        }
        unlock_file(&self.guard.lock)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
