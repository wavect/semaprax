//! Authenticate the borrowed field read's canonical ownership commit.
use super::{error, CleanupTransition, Diagnostic, ExpressionId, NativeBytesPlan};

impl NativeBytesPlan {
    pub(in crate::codegen) fn authenticate_vec_field_commit(
        &self,
        call: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let mut commits =
            self.transitions
                .get(call)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::CallCommit {
                        call: owner,
                        arguments,
                    } => Some((owner, arguments)),
                    _ => None,
                });
        let (owner, arguments) = commits
            .next()
            .ok_or_else(|| error("Vec field read lacks canonical CallCommit"))?;
        if owner != call || !arguments.is_empty() || commits.next().is_some() {
            return Err(error(
                "Vec field read requires exactly one zero-owned CallCommit",
            ));
        }
        // The authenticated commit transfers no ownership and emits no C.
        // Authenticate it after argument evaluation, before any field access.
        Ok(())
    }
}
