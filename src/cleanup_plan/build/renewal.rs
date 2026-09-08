use super::*;
impl PlanBuilder<'_> {
    pub(super) fn merge_states(
        &self,
        left: &FlowState,
        right: &FlowState,
    ) -> Result<FlowState, Diagnostic> {
        if left.renewals != right.renewals {
            return Err(plan_error(
                "branch join has conflicting renewal reservations",
            ));
        }
        let flags = left
            .live_order
            .iter()
            .chain(&right.live_order)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut successors = flags
            .iter()
            .map(|flag| (*flag, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut indegree = flags
            .iter()
            .map(|flag| (*flag, 0_u32))
            .collect::<BTreeMap<_, _>>();
        for history in [&left.live_order, &right.live_order] {
            for pair in history.windows(2) {
                if successors
                    .get_mut(&pair[0])
                    .expect("joined flag is indexed")
                    .insert(pair[1])
                {
                    *indegree.get_mut(&pair[1]).expect("joined flag is indexed") += 1;
                }
            }
        }
        let mut ready = indegree
            .iter()
            .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
            .collect::<BTreeSet<_>>();
        let mut live_order = Vec::with_capacity(flags.len());
        while let Some(flag) = ready.pop_first() {
            live_order.push(flag);
            for successor in successors[&flag].iter().copied() {
                let degree = indegree
                    .get_mut(&successor)
                    .expect("joined flag is indexed");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(successor);
                }
            }
        }
        if live_order.len() != flags.len() {
            return Err(plan_error(
                "branch join has conflicting cleanup initialization histories",
            ));
        }
        let left_variants = left
            .conditional_variants
            .iter()
            .map(|variant| (&variant.root, variant))
            .collect::<BTreeMap<_, _>>();
        let right_variants = right
            .conditional_variants
            .iter()
            .map(|variant| (&variant.root, variant))
            .collect::<BTreeMap<_, _>>();
        if left_variants.len() != left.conditional_variants.len()
            || right_variants.len() != right.conditional_variants.len()
            || left_variants.keys().ne(right_variants.keys())
        {
            return Err(plan_error(
                "branch join disagrees on conditional variant roots",
            ));
        }
        let mut conditional_variants = Vec::with_capacity(left_variants.len());
        for (root, left_variant) in left_variants {
            let right_variant = right_variants[root];
            if left_variant.variant != right_variant.variant {
                return Err(plan_error(
                    "branch join disagrees on conditional variant identity",
                ));
            }
            let left_cases = left_variant
                .cases
                .iter()
                .map(|(case, flags)| (case, flags))
                .collect::<BTreeMap<_, _>>();
            let right_cases = right_variant
                .cases
                .iter()
                .map(|(case, flags)| (case, flags))
                .collect::<BTreeMap<_, _>>();
            let domain = self
                .program
                .declarations
                .variant_cases(&left_variant.variant)
                .ok_or_else(|| plan_error("branch join variant has no closed case domain"))?;
            let mut cases = Vec::new();
            for declared_case in domain {
                let left_flags = left_cases.get(&declared_case.id).copied();
                let right_flags = right_cases.get(&declared_case.id).copied();
                let flags = match (left_flags, right_flags) {
                    (Some(left_flags), Some(right_flags)) if left_flags == right_flags => {
                        left_flags
                    }
                    (Some(_), Some(_)) => {
                        return Err(plan_error(
                            "branch join disagrees on conditional case payload liveness",
                        ));
                    }
                    (Some(flags), None) | (None, Some(flags)) => flags,
                    (None, None) => continue,
                };
                cases.push((declared_case.id.clone(), flags.clone()));
            }
            if cases.len()
                != left_cases
                    .keys()
                    .chain(right_cases.keys())
                    .collect::<BTreeSet<_>>()
                    .len()
            {
                return Err(plan_error(
                    "branch join conditional state references a foreign case",
                ));
            }
            conditional_variants.push(ConditionalFlowVariant {
                root: root.clone(),
                variant: left_variant.variant.clone(),
                cases,
            });
        }
        Ok(FlowState {
            renewals: left.renewals.clone(),
            live_order,
            conditional_variants,
        })
    }
}

impl PlanBuilder<'_> {
    pub(super) fn reserve_statement_renewal(
        &mut self,
        statement: &ResolvedStatement,
        block: BlockId,
        state: &mut FlowState,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        let ResolvedStatement::Assign {
            binding,
            field: None,
            value,
            ..
        } = statement
        else {
            return Ok(());
        };
        if self.schema != super::super::CLEANUP_PLAN_SCHEMA_V12
            || crate::hir::iterator_loop::renewal_binding(self.function, &value.id).is_none()
        {
            return Ok(());
        }
        let place = self
            .binding_slot(binding, region)?
            .ok_or_else(|| plan_error("renewal has no owned binding slot"))?;
        let flags = self.flags_under(&place);
        if flags.len() != 1 || !state.live_order.contains(&flags[0]) || !state.renewals.is_empty() {
            return Err(plan_error(
                "renewal reservation requires one live unreserved Vec owner",
            ));
        }
        state
            .renewals
            .insert(value.id.clone(), state.live_order.clone());
        self.push_transition(
            block,
            CleanupTransition::ReserveRenewal {
                at: value.id.clone(),
                binding: place,
            },
        );
        Ok(())
    }
    pub(super) fn finish_renewal(
        &self,
        at: &ExpressionId,
        destination: &CleanupPlace,
        history: &[LivenessFlagId],
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let flags = self.flags_under(destination);
        if flags.len() != 1 {
            return Err(plan_error("renewal destination is not one Vec leaf"));
        }
        let flag = flags[0];
        let expected = history
            .iter()
            .filter(|candidate| **candidate != flag)
            .copied()
            .collect::<Vec<_>>();
        let actual = state
            .live_order
            .iter()
            .filter(|candidate| **candidate != flag)
            .copied()
            .collect::<Vec<_>>();
        if expected != actual || !history.contains(&flag) || !state.live_order.contains(&flag) {
            return Err(plan_error(
                "renewal changed unrelated cleanup initialization history",
            ));
        }
        state.live_order = history.to_vec();
        state
            .renewals
            .remove(at)
            .ok_or_else(|| plan_error("renewal has no reservation"))?;
        Ok(())
    }
}
