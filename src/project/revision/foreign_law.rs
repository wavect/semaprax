//! Project-authenticated, read-only LAW-09 diagnostic route.

use crate::diagnostic::Diagnostic;
use crate::native_rust_binding::foreign_law::{
    self, DeclaredForeignSummary, ForeignBoundary, ForeignLawFrontier, ForeignLawRequest,
};
use crate::native_rust_binding::ScalarBindingPlan;

use super::super::ProjectSnapshot;
use super::ProjectRevision;

impl ProjectRevision {
    /// Re-derive the exact lock from this retained Project and find the Rust
    /// import in checked HIR before recording any foreign assumption.
    pub fn foreign_law_frontier(
        &self,
        binding: &ScalarBindingPlan,
        runtime_target: &str,
        adapter_digest: &str,
        declared: &DeclaredForeignSummary,
        law: &ForeignLawRequest,
    ) -> Result<ForeignLawFrontier, Vec<Diagnostic>> {
        let workspace = self.canonical_workspace_revision()?;
        let imports = [
            self.entry_program(),
            self.public_api_program(),
            self.test_program(),
        ]
        .into_iter()
        .flat_map(|program| &program.interfaces)
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.id.as_str() == binding.import_id)
        .collect::<Vec<_>>();
        let Some(first) = imports.first() else {
            return Err(vec![Diagnostic::io(
                "SPX-FL307",
                "selected foreign import is absent from authenticated Project HIR",
            )]);
        };
        if imports.iter().any(|import| *import != *first) {
            return Err(vec![Diagnostic::io(
                "SPX-FL307",
                "selected foreign import has conflicting authenticated Project HIR rows",
            )]);
        }
        foreign_law::derive(
            first,
            binding,
            ForeignBoundary {
                project_lock_digest: workspace.dependency_lock_digest(),
                target: runtime_target,
                adapter_digest,
            },
            declared,
            law,
        )
        .map_err(|error| vec![error])
    }
}

impl ProjectSnapshot {
    /// Agent diagnostic view under the ordinary before/after held-source
    /// authentication. This result grants no Rust invocation authority.
    pub fn foreign_law_frontier_json(
        &mut self,
        binding: &ScalarBindingPlan,
        runtime_target: &str,
        adapter_digest: &str,
        declared: &DeclaredForeignSummary,
        law: &ForeignLawRequest,
    ) -> Result<String, Vec<Diagnostic>> {
        self.with_authenticated_request(|snapshot| {
            let frontier = snapshot.retain_revision().foreign_law_frontier(
                binding,
                runtime_target,
                adapter_digest,
                declared,
                law,
            )?;
            Ok(frontier.public_view())
        })
    }
}
