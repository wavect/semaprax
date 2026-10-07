//! The declared v16 replacement history boundary, retaining chronological RHS temporaries.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn finish_string_replacement(
        &self,
        at: &ExpressionId,
        destination: &CleanupPlace,
        history: &[LivenessFlagId],
        state: &mut FlowState,
    ) -> Result<(), Diagnostic> {
        let flags = self.flags_under(destination);
        let [target] = flags.as_slice() else {
            return Err(plan_error("String replacement needs one leaf"));
        };
        if !history.contains(target) || !state.live_order.contains(target) {
            return Err(plan_error("String replacement lost its reserved owner"));
        }
        let survivors = history
            .iter()
            .copied()
            .filter(|flag| flag != target && state.live_order.contains(flag))
            .collect::<Vec<_>>();
        let actual = state
            .live_order
            .iter()
            .copied()
            .filter(|flag| flag != target)
            .collect::<Vec<_>>();
        if !actual.starts_with(&survivors)
            || actual[survivors.len()..]
                .iter()
                .any(|flag| history.contains(flag))
        {
            return Err(plan_error(
                "String replacement changed surviving owner history",
            ));
        }
        let mut published = history
            .iter()
            .copied()
            .filter(|flag| flag == target || state.live_order.contains(flag))
            .collect::<Vec<_>>();
        published.extend_from_slice(&actual[survivors.len()..]);
        state.live_order = published;
        state
            .renewals
            .remove(at)
            .ok_or_else(|| plan_error("String replacement lacks a reservation"))?;
        Ok(())
    }
}
