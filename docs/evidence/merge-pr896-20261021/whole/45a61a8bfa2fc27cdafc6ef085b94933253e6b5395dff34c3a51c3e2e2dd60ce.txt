//! Lexical retained child-ID growth; the outer reserve_vec Result is absent.
#[cfg(test)]
mod before;
#[cfg(test)]
mod matched;
#[cfg(test)]
mod tests;

// Compile-time identifiers only: no runtime forwarding arguments or Result.
// Keep the complete reserve_vec algorithm and all nested fallible receipts.
macro_rules! prepare {
    ($owner:ident, $successors:ident, $function:ident, $budget:ident, $receiving:ident) => {{
        // This is the original evaluated additional argument / owned formal.
        let additional = $successors.len();
        if $budget.pays_allocations() {
            attempt!($budget.reserve_bytes(
                $function,
                0,
                "canonical ordered child identity scratch"
            ));
            let required =
                attempt!($owner
                    .child_ids
                    .len()
                    .checked_add(additional)
                    .ok_or_else(|| {
                        $budget.budget_refusal(
                            $function,
                            "cleanup replay physical vector length overflowed",
                            "",
                        )
                    }));
            if required > $owner.child_ids.capacity() {
                let target = attempt!(
                    crate::cleanup_plan::replay::physical_storage::replay_growth_capacity(
                        $owner.child_ids.capacity(),
                        required,
                    )
                    .ok_or_else(|| {
                        $budget.budget_refusal(
                            $function,
                            "cleanup replay physical vector capacity overflowed",
                            "",
                        )
                    })
                );
                attempt!($budget.reserve_array::<usize>(
                    $function,
                    target,
                    "canonical ordered child identity scratch",
                ));
                attempt!($owner
                    .child_ids
                    .try_reserve_exact(target - $owner.child_ids.len())
                    .map_err(|_| {
                        $budget.budget_refusal(
                            $function,
                            "cleanup replay physical vector allocation failed",
                            "",
                        )
                    }));
                if $owner.child_ids.capacity() != target {
                    $receiving.refusal = Some($budget.budget_refusal(
                        $function,
                        "cleanup replay physical vector capacity disagrees with its reservation",
                        "",
                    ));
                    $receiving.ready = true;
                    return;
                }
            }
        }
    }};
}
pub(super) use prepare;
