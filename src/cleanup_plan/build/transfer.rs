//! Canonical owner transfers and successful replacement boundary.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn transfer(
        &mut self,
        block: BlockId,
        at: ExpressionId,
        source: CleanupPlace,
        destination: CleanupPlace,
        state: &mut FlowState,
        normalize_complete_aggregate: bool,
    ) -> Result<(), Diagnostic> {
        self.reserve_string_append(&at, &source, state)?;
        let renewal = state.renewals.get(&at).cloned().filter(|_| {
            matches!(&destination.storage, StorageId::Value(_))
                && crate::cleanup_plan::renewal_binding(self.function, &at).is_some_and(|binding| {
                    destination == CleanupPlace::whole(StorageId::Value(binding.id.clone()))
                })
        });
        let source_flags = self.flags_under(&source);
        let destination_flags = self.flags_under(&destination);
        if source_flags.len() != destination_flags.len() || source_flags.is_empty() {
            return Err(plan_error(format!(
                "transfer at `{at}` has incompatible cleanup shapes"
            )));
        }
        if renewal.is_some()
            && crate::string_ops::replacement::binding(self.function, &at).is_some()
        {
            if source_flags == destination_flags {
                return Err(plan_error("String replacement aliases its new owner"));
            }
            state.remove(&destination_flags.iter().copied().collect());
        }
        if destination_flags.iter().any(|flag| state.is_live(*flag)) {
            return Err(plan_error(format!(
                "transfer at `{at}` initializes a live cleanup place"
            )));
        }
        if let Some(index) = state
            .conditional_variants
            .iter()
            .position(|variant| variant.root == source)
        {
            let variant = state.conditional_variants.remove(index);
            let mut mapped_cases = Vec::with_capacity(variant.cases.len());
            for (case, flags) in variant.cases {
                let mut mapped = Vec::with_capacity(flags.len());
                for source_flag in flags {
                    let source_leaf = &self.leaves[&source_flag].place;
                    let relative = source_leaf
                        .projections
                        .strip_prefix(source.projections.as_slice())
                        .ok_or_else(|| plan_error("invalid conditional transfer source prefix"))?;
                    let expected = destination
                        .projections
                        .iter()
                        .chain(relative)
                        .cloned()
                        .collect::<Vec<_>>();
                    let destination_flag = destination_flags
                        .iter()
                        .find(|flag| self.leaves[flag].place.projections == expected)
                        .copied()
                        .ok_or_else(|| {
                            plan_error("conditional transfer destination shape is inconsistent")
                        })?;
                    mapped.push(destination_flag);
                }
                mapped_cases.push((case, mapped));
            }
            let variant_id = variant.variant;
            state.conditional_variants.push(ConditionalFlowVariant {
                root: destination.clone(),
                variant: variant_id.clone(),
                cases: mapped_cases,
            });
            self.push_transition(
                block,
                CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    variant: variant_id,
                },
            );
            return Ok(());
        }

        if source_flags.iter().any(|flag| !state.is_live(*flag)) {
            return Err(plan_error(format!(
                "transfer at `{at}` reads a non-live cleanup place"
            )));
        }
        let source_set = source_flags.iter().copied().collect::<BTreeSet<_>>();
        let source_history = state
            .live_order
            .iter()
            .filter(|flag| source_set.contains(flag))
            .copied()
            .collect::<Vec<_>>();
        state.remove(&source_set);

        let destination_order = if normalize_complete_aggregate {
            destination_flags
        } else {
            let mut mapped = Vec::with_capacity(source_history.len());
            for source_flag in source_history {
                let source_leaf = &self.leaves[&source_flag].place;
                let relative = source_leaf
                    .projections
                    .strip_prefix(source.projections.as_slice())
                    .ok_or_else(|| plan_error("invalid cleanup transfer source prefix"))?;
                let expected = destination
                    .projections
                    .iter()
                    .chain(relative)
                    .cloned()
                    .collect::<Vec<_>>();
                let destination_flag = destination_flags
                    .iter()
                    .find(|flag| self.leaves[flag].place.projections == expected)
                    .copied()
                    .ok_or_else(|| plan_error("cleanup transfer leaf mismatch"))?;
                mapped.push(destination_flag);
            }
            mapped
        };
        state.append_distinct(destination_order);
        self.publish_string_append(&at, &destination, state)?;
        if let Some(history) = renewal {
            self.finish_renewal(&at, &destination, &history, state)?;
            self.push_transition(
                block,
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                },
            );
        } else {
            self.push_transition(
                block,
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                },
            );
        }
        Ok(())
    }
}
