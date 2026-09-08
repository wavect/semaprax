//! Exact continuation after an authored owning variant Match join.
use super::*;
impl Emitter<'_> {
    pub(super) fn apply_variant_match_continuation(
        &mut self,
        expression: &ResolvedExpr,
        value: &Value,
    ) -> Result<(), Diagnostic> {
        let mut source = crate::cleanup_plan::CleanupPlace {
            storage: crate::cleanup_plan::StorageId::Temporary(expression.id.clone()),
            projections: Vec::new(),
        };
        let transitions = self.function.cleanup_plan.blocks.iter().flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, crate::cleanup_plan::CleanupTransition::TransferVariant { at, .. } if *at == expression.id)).cloned().collect::<Vec<_>>();
        for transition in transitions {
            if let crate::cleanup_plan::CleanupTransition::TransferVariant {
                source: candidate,
                destination,
                variant,
                ..
            } = transition
            {
                if candidate != source {
                    continue;
                }
                self.apply_variant_transfer_group(
                    std::slice::from_ref(&candidate),
                    &destination,
                    &variant,
                    value,
                )?;
                source = destination;
            }
        }
        Ok(())
    }
}

impl Emitter<'_> {
    pub(super) fn emit_owned_variant_match_cleanup(
        &mut self,
        fields: &[crate::hir::ResolvedMatchPatternField],
    ) -> Result<(), Diagnostic> {
        let storage = fields
            .iter()
            .filter(|field| field.binding.ty == ResolvedType::Bytes)
            .map(|field| crate::cleanup_plan::StorageId::Value(field.binding.id.clone()))
            .collect::<std::collections::BTreeSet<_>>();
        if storage.is_empty() {
            return Ok(());
        }
        let mut regions = self.cleanup_plan.regions.iter().filter(|region| {
            storage
                .iter()
                .all(|candidate| region.slots.contains(candidate))
        });
        let region = regions
            .next()
            .ok_or_else(|| error("owned variant bindings have no CleanupPlan region"))?;
        if regions.next().is_some() {
            return Err(error(
                "owned variant bindings map to ambiguous cleanup regions",
            ));
        }
        let exit = self
            .cleanup_plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| exit.id == region.normal_scope_end)
            .ok_or_else(|| error("owned variant match region has no normal exit"))?;
        if !matches!(
            exit.continuation,
            crate::cleanup_plan::ExitContinuation::Continue(_)
        ) || exit.leaves_regions.as_slice() != [region.id]
        {
            return Err(error("owned variant match cleanup exit is not canonical"));
        }
        self.emit_cleanup_actions(&exit.finalize_in_order)
    }
}
