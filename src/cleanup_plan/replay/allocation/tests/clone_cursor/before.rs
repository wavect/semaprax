use super::*;

impl ReplayBudget {
    #[track_caller]
    pub(super) fn reserve_clone_before<T: ClonePayload>(
        &mut self,
        function: &ResolvedFunction,
        value: &T,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        self.prepare_clone_cursor(function)?;
        #[cfg(test)]
        let payload = match self.oracle_lease() {
            Some(lease) => {
                value.clone_payload_bytes_with_work(&mut |units| lease.charge_work(units).is_ok())
            }
            None => value.clone_payload_bytes(),
        };
        #[cfg(not(test))]
        let payload = value.clone_payload_bytes();
        let bytes = payload.ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay clone payload exceeds the physical bound",
                "",
            )
        })?;
        self.reserve_bytes(function, bytes, phase)
    }
}
