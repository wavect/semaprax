//! Finite reference-only financing for complete workspace unit fixtures.
//! The builder result still uses its unchanged ordinary 64 MiB account.
//! No compiler, resolver, or production caller selects this test-only scope.
use super::allocation::{opt_in, OracleScope};
// Each receiver retains its independent ordinary and skeleton work ceilings.
// The aggregate account pays both domains for every structurally bounded
// invocation; it never renews either receiver's original allowance.
pub(in crate::cleanup_plan::replay) fn reference_work_bound(receivers: usize) -> usize {
    assert!(
        receivers > 0,
        "a reference scope must have a finite positive receiver bound"
    );
    receivers
        .checked_mul(2)
        .and_then(|domains| domains.checked_mul(super::MAX_REPLAY_WORK_UNITS))
        .expect("finite reference receiver/domain work product")
}

pub(in crate::cleanup_plan::replay) struct ReferenceReceiptOnUnwind<'a> {
    scope: &'a OracleScope,
    receivers: usize,
}
impl<'a> ReferenceReceiptOnUnwind<'a> {
    pub(in crate::cleanup_plan::replay) fn new(scope: &'a OracleScope, receivers: usize) -> Self {
        Self { scope, receivers }
    }
}
impl Drop for ReferenceReceiptOnUnwind<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!(
                "reference scope unwinding: max_receivers={} complete_receipt={:?}",
                self.receivers,
                self.scope.receipt()
            );
        }
    }
}

pub(crate) fn with_financed_workspace_test_oracles<T>(
    receivers: usize,
    body: impl FnOnce() -> T,
) -> (T, usize, usize) {
    // Preserve the existing finite physical account. This byte allowance is
    // declared reference funding, not a derived per-receiver allocation bound
    // and not the ordinary builder's unchanged 64 MiB quota.
    let scope = OracleScope::open_with_work(1 << 28, reference_work_bound(receivers))
        .expect("explicit independent workspace reference account");
    let trace = ReferenceReceiptOnUnwind::new(&scope, receivers);
    let lease = scope.lease();
    let selected = opt_in(lease).expect("one explicit workspace reference account");
    let result = body();
    drop(selected);
    drop(trace);
    let receipt = scope.finish();
    assert!(receipt.started <= receivers, "{receipt:?}");
    eprintln!("workspace reference max_receivers={receivers} complete_receipt={receipt:?}");
    assert!(
        receipt.started > 0,
        "workspace fixture must execute the live reference hook"
    );
    assert_eq!(receipt.completed, receipt.started);
    assert!(
        receipt.first_refusal.is_none()
            && receipt.first_work_refusal.is_none()
            && receipt.physical_failure.is_none(),
        "{receipt:?}"
    );
    (result, receipt.started, receipt.charged)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workspace_reference_scope_rejects_an_empty_comparison_and_restores_opt_in() {
        assert!(
            std::panic::catch_unwind(|| with_financed_workspace_test_oracles(2, || 7)).is_err()
        );
        assert!(super::super::allocation::opted().unwrap().is_none());
    }
    #[test]
    fn workspace_reference_scope_preserves_the_original_panic_and_restores_opt_in() {
        let error = std::panic::catch_unwind(|| {
            with_financed_workspace_test_oracles(2, || panic!("workspace fixture original panic"))
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<&str>(),
            Some(&"workspace fixture original panic")
        );
        assert!(super::super::allocation::opted().unwrap().is_none());
    }
}
