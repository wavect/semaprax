//! Successful String replacement consumes only the replay-authenticated old leaf.
use super::*;
impl NativeBytesPlan {
    pub(super) fn release_replaced_string(
        &self,
        transition: &CleanupTransition,
        destination: &ByteSlot,
    ) -> String {
        if !matches!(transition, CleanupTransition::Renew { .. })
            || destination.kind != OwnedLeafKind::String
        {
            return String::new();
        }
        self.emit_finalizers(std::slice::from_ref(destination), false)
    }
    pub(in crate::codegen) fn transfer_to(
        &self,
        storage: &StorageId,
        at: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        let mut matches =
            self.transitions.get(at).into_iter().flatten().filter_map(
                |transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination,
                        ..
                    }
                    | CleanupTransition::Renew {
                        source,
                        destination,
                        ..
                    } => (destination.storage == *storage && destination.projections.is_empty())
                        .then_some((source, destination)),
                    _ => None,
                },
            );
        let Some((source, destination)) = matches.next() else {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has no canonical transfer"
            )));
        };
        if matches.next().is_some() {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has ambiguous transfers"
            )));
        }
        let source = self
            .slots
            .get(source)
            .ok_or_else(|| error("Bytes transfer source is not indexed"))?;
        let destination = self
            .slots
            .get(destination)
            .ok_or_else(|| error("Bytes transfer destination is not indexed"))?;
        if source.kind != destination.kind {
            return Err(error("owned plan transfer changes carrier kind"));
        }
        let release = self.transitions.get(at).into_iter().flatten()
            .filter(|t| matches!(t, CleanupTransition::Renew { destination: d, .. } if d.storage == *storage && d.projections.is_empty()))
            .map(|t| self.release_replaced_string(t, destination)).collect::<String>();
        Ok(release + &format!(
            "if (!{} || {}) spx_runtime_invariant_failure(\"owned plan transfer liveness {} to {}\");\n{} = {};\n{} = false;\n{} = true;\n",
            source.flag,
            destination.flag,
            source.value,
            destination.value,
            destination.value,
            source.kind.move_call(&source.value),
            source.flag,
            destination.flag
        ))
    }
}
