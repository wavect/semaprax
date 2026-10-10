//! Authenticate the borrowed field read's canonical ownership commit.
use super::{
    error, CleanupTransition, Diagnostic, ExpressionId, NativeBytesPlan, ResolvedFunction,
};

impl NativeBytesPlan {
    pub(in crate::codegen) fn authenticate_vec_field_commit(
        function: &ResolvedFunction,
        call: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let mut commits = function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter_map(|transition| match transition {
                CleanupTransition::CallCommit {
                    call: owner,
                    arguments,
                } if owner == call => Some((owner, arguments)),
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
        // Borrow-only functions can have no physical owning slots and therefore
        // no NativeBytesPlan instance. Their semantic commits remain authoritative.
        // The authenticated commit transfers no ownership and emits no C.
        // Authenticate it after argument evaluation, before any field access.
        Ok(())
    }
}
