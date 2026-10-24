//! Finite reference-only financing for complete workspace unit fixtures.
//! The builder result still uses its unchanged ordinary 64 MiB account.
//! No compiler, resolver, or production caller selects this test-only scope.
use super::allocation::{opt_in, OracleScope};

pub(crate) fn with_financed_workspace_test_oracles<T>(
    body: impl FnOnce() -> T,
) -> (T, usize, usize) {
    // A 1,024-function fixture executes 2,052 complete reference receivers.
    // A private aarch64 Rust 1.97.1 control measured 194,083,774 bytes. Use a finite
    // 256 MiB account up front; the ordinary builder never draws from it.
    let scope = OracleScope::open_with_work(1 << 28, 1 << 25)
        .expect("explicit independent workspace reference account");
    let lease = scope.lease();
    let selected = opt_in(lease).expect("one explicit workspace reference account");
    let result = body();
    drop(selected);
    let receipt = scope.finish();
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
        assert!(std::panic::catch_unwind(|| with_financed_workspace_test_oracles(|| 7)).is_err());
        assert!(super::super::allocation::opted().unwrap().is_none());
    }
    #[test]
    fn workspace_reference_scope_preserves_the_original_panic_and_restores_opt_in() {
        let error = std::panic::catch_unwind(|| {
            with_financed_workspace_test_oracles(|| panic!("workspace fixture original panic"))
        })
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<&str>(),
            Some(&"workspace fixture original panic")
        );
        assert!(super::super::allocation::opted().unwrap().is_none());
    }
}
