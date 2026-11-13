//! Producer branch-join normalization for one deduplicated incoming state.
//!
//! The builder's `merge_states` re-keys conditional variant roots at every
//! ordinary branch join, including a join whose arms carry identical ownership.
//! The incoming `BTreeSet` collapses those identical arms to one state, so the
//! replay must still apply the same join at that block. Loop exits and cleanup
//! exit continuations never merge, and keep their single incoming order.
//!
//! Every root comparison, classifier initialization, edge visit, probe and the
//! self-join itself debit the caller's `ReplayBudget` before they run. The
//! block's `join_units` covers live order and renewals only, never this work.
use super::*;

/// Lazily classified so plans whose conditional roots never cross their key
/// order pay no classification work or storage.
#[derive(Default)]
pub(super) struct BranchJoins(Option<Vec<usize>>);

/// A block reached by any edge other than an unconditional `Goto`.
const MIXED: usize = usize::MAX;

/// Keys one `BTreeMap` search may visit in the declaration index: at most
/// eleven per node, and each B-tree level at least doubles its entries.
const INDEX_SEARCH_COMPARISONS: usize = 11 * usize::BITS as usize;

/// Upper bound on the work of one derived `CleanupPlace` comparison: every
/// identity byte, discriminant, fingerprint and projection boundary it reads.
fn place_key_units(
    place: &CleanupPlace,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    budget.charge(function, 1, "branch join estimate visits")?;
    budget.charge(
        function,
        place.projections.len(),
        "branch join estimate visits",
    )?;
    Ok((|| {
        let storage = match &place.storage {
            StorageId::Value(value) => value.as_str().len().checked_add(1)?,
            StorageId::Temporary(expression) => expression.as_str().len(),
            StorageId::CallArgument {
                call,
                value_expression,
                ..
            } => call
                .as_str()
                .len()
                .checked_add(value_expression.as_str().len())?
                .checked_add(1)?,
            StorageId::ProvisionalResult => 0,
        };
        place
            .projections
            .iter()
            .try_fold(storage.checked_add(2)?, |n, id| {
                n.checked_add(id.as_str().len())?.checked_add(1)
            })
    })())
}

fn root_pair_units(
    left: &CleanupPlace,
    right: &CleanupPlace,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    let left = place_key_units(left, function, budget)?;
    let right = place_key_units(right, function, budget)?;
    Ok(left
        .zip(right)
        .and_then(|(a, b)| a.checked_add(b)?.checked_add(1)))
}

/// `join_conditional_variants(v, v)` before its domain lookups: both root
/// indices (4n searches), the key-set check (n) and the indexed probes (n),
/// each search visiting at most n roots; then per variant its identity
/// comparison, two declaration-index searches (the domain bound's and the
/// join's), both case indices and the case union (12c² case comparisons),
/// the count check and the joined identity.
fn self_join_units(
    variants: &[ReplayConditionalVariant],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    let n = variants.len();
    let mut key = 0;
    for value in variants {
        budget.charge(function, 1, "branch join estimate visits")?;
        let Some(size) = place_key_units(&value.root, function, budget)? else {
            return Ok(None);
        };
        key = key.max(size);
    }
    let Some(mut units) = n
        .checked_mul(n)
        .and_then(|n| n.checked_mul(5))
        .and_then(|v| v.checked_add(n))
        .and_then(|v| key.checked_mul(2)?.checked_add(1)?.checked_mul(v))
    else {
        return Ok(None);
    };
    for value in variants {
        budget.charge(function, 1, "branch join estimate visits")?;
        budget.charge(function, value.cases.len(), "branch join estimate visits")?;
        let c = value.cases.len();
        let case_key = value
            .cases
            .iter()
            .fold(0_usize, |max, (id, _)| max.max(id.as_str().len()));
        let root = place_key_units(&value.root, function, budget)?;
        let next = (|| {
            let variant = value.variant.as_str().len().checked_add(1)?;
            units
                .checked_add(variant)?
                .checked_add(
                    INDEX_SEARCH_COMPARISONS
                        .checked_mul(2)?
                        .checked_mul(variant)?,
                )?
                .checked_add(
                    c.checked_mul(c)?
                        .checked_mul(12)?
                        .checked_mul(case_key.checked_mul(2)?.checked_add(1)?)?,
                )?
                .checked_add(1)?
                .checked_add(root?)?
                .checked_add(variant)
        })();
        let Some(next) = next else { return Ok(None) };
        units = next;
    }
    Ok(Some(units))
}

/// The closed-domain loop of `join_conditional_variants(v, v)` for one
/// variant: per declared case two searches over at most c case keys, one
/// flag-history comparison and one clone of the case identity and flags.
fn domain_units(
    value: &ReplayConditionalVariant,
    domain: &[crate::hir::ResolvedVariantCaseDeclaration],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    // Each scan is prepaid separately before touching its first element.
    budget.charge(function, value.cases.len(), "branch join estimate visits")?;
    let c = value.cases.len();
    let case_key = value
        .cases
        .iter()
        .fold(0_usize, |max, (id, _)| max.max(id.as_str().len()));
    budget.charge(function, value.cases.len(), "branch join estimate visits")?;
    let flags = value
        .cases
        .iter()
        .fold(0_usize, |max, (_, flags)| max.max(flags.len()));
    budget.charge(function, domain.len(), "branch join estimate visits")?;
    Ok(domain.iter().try_fold(0_usize, |units, declared| {
        let id = declared.id.as_str().len();
        units
            .checked_add(
                c.checked_mul(2)?
                    .checked_mul(id.checked_add(case_key)?.checked_add(1)?)?,
            )?
            .checked_add(flags.checked_mul(2)?)?
            .checked_add(id)?
            .checked_add(2)
    }))
}

impl BranchJoins {
    /// Two or more predecessors, each reaching `block` through its own
    /// unconditional `Goto`: the exact CFG shape every `merge_states` emits.
    fn contains(
        &mut self,
        function: &ResolvedFunction,
        block: BlockId,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        if self.0.is_none() {
            let plan = &function.cleanup_plan;
            // One debit for the zeroed shape per block, one visit per edge
            // and this probe. A refusal caches nothing, so every retry pays.
            let units = plan
                .blocks
                .len()
                .checked_add(plan.edges.len())
                .and_then(|n| n.checked_add(1))
                .unwrap_or(usize::MAX);
            budget.charge(function, units, "branch join classification")?;
            budget.reserve_array::<usize>(function, plan.blocks.len(), "branch join shapes")?;
            let mut inputs = vec![0_usize; plan.blocks.len()];
            for edge in &plan.edges {
                let Some(count) = inputs.get_mut(edge.to.0 as usize) else {
                    continue;
                };
                let goto = plan.blocks.get(edge.from.0 as usize).is_some_and(|from| {
                    matches!(&from.terminator, CleanupTerminator::Goto(id) if *id == edge.id)
                });
                let always = matches!(edge.condition, EdgeCondition::Always);
                *count = if *count != MIXED && goto && always {
                    *count + 1
                } else {
                    MIXED
                };
            }
            self.0 = Some(inputs);
        } else {
            budget.charge(function, 1, "branch join classification")?;
        }
        Ok(self
            .0
            .as_ref()
            .and_then(|inputs| inputs.get(block.0 as usize))
            .is_some_and(|count| *count != MIXED && *count >= 2))
    }

    /// Apply the producer join to one deduplicated state at a branch join.
    /// Roots already in key order are exactly what the join yields; anything
    /// else replays the same closed-domain join of the state with itself. An
    /// owning break's exact rejoin is never re-keyed and performs no work.
    pub(super) fn normalize(
        &mut self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        block: BlockId,
        exact_join: bool,
        conditional_variants: Vec<ReplayConditionalVariant>,
        budget: &mut ReplayBudget,
    ) -> Result<Vec<ReplayConditionalVariant>, Diagnostic> {
        if exact_join {
            return Ok(conditional_variants);
        }
        let mut crossed = false;
        for pair in conditional_variants.windows(2) {
            let units = root_pair_units(&pair[0].root, &pair[1].root, function, budget)?;
            budget.charge(
                function,
                units.unwrap_or(usize::MAX),
                "branch join root order",
            )?;
            if pair[0].root >= pair[1].root {
                crossed = true;
                break;
            }
        }
        if !crossed || !self.contains(function, block, budget)? {
            return Ok(conditional_variants);
        }
        let units = self_join_units(&conditional_variants, function, budget)?;
        budget.charge(
            function,
            units.unwrap_or(usize::MAX),
            "branch join conditional roots",
        )?;
        for value in &conditional_variants {
            budget.charge(function, 1, "branch join estimate visits")?;
            let domain = program
                .declarations()
                .variant_cases(&value.variant)
                .unwrap_or_default();
            let units = domain_units(value, domain, function, budget)?;
            budget.charge(
                function,
                units.unwrap_or(usize::MAX),
                "branch join conditional cases",
            )?;
        }
        join_conditional_variants(
            program,
            function,
            &conditional_variants,
            &conditional_variants,
            budget,
        )
    }
}

#[cfg(test)]
mod tests;
