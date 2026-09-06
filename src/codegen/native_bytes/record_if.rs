//! Apply only the reached record branch's canonical join, then its continuation.
use super::*;

impl NativeBytesPlan {
    pub(in crate::codegen) fn apply_record_if_branch(
        &self,
        parent: &ExpressionId,
        branch: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        let destination = CleanupPlace {
            storage: StorageId::Temporary(parent.clone()),
            projections: Vec::new(),
        };
        let source_storage = StorageId::Temporary(branch.clone());
        let transitions = self
            .transitions
            .get(parent)
            .ok_or_else(|| error("owning record If has no canonical transitions"))?;
        let selected = transitions.iter().filter(|transition| matches!(transition,
            CleanupTransition::Transfer { source, destination: target, .. }
                if *target == destination && source.storage == source_storage && source.projections.is_empty())).count();
        if selected != 1 {
            return Err(error(
                "owning record If branch has no unique canonical join",
            ));
        }
        let mut output = String::new();
        for transition in transitions {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination: target,
                    ..
                } => {
                    if *target == destination && source.storage != source_storage {
                        continue;
                    }
                    for (source, target) in self.transfer_pairs(source, target)? {
                        output.push_str(&emit_transfer(source, target, "record If plan transfer"));
                    }
                }
                CleanupTransition::Initialize { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::StageCopyResult { .. } => {
                    return Err(error("owning record If has a non-record continuation"));
                }
                CleanupTransition::CallCommit { .. } | CleanupTransition::SelectFailure { .. } => {}
            }
        }
        Ok(output)
    }
}
