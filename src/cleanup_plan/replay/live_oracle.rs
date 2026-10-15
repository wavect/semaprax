//! Explicit owning-test account for the live factored comparison hook.
//! Production ActiveBuilder authority is untouched. The fixture's complete
//! receiving body and Drop run inside this finite affine account.
use super::allocation::{opt_in, OracleScope};

pub(crate) fn with_financed_test_oracles(body: impl FnOnce()) {
    // The already exercised prototype allowance is declared once up front;
    // it is not a refill, production cap or cumulative-receipt adjustment.
    let scope =
        OracleScope::open_with_work(1 << 26, 1 << 25).expect("explicit independent oracle account");
    let lease = scope.lease();
    let selected = opt_in(lease).expect("one explicit oracle account");
    body();
    drop(selected);
    let receipt = scope.finish();
    assert!(
        receipt.started > 0,
        "owning fixture must execute the live oracle hook"
    );
    assert_eq!(receipt.completed, receipt.started);
    assert!(
        receipt.first_refusal.is_none()
            && receipt.first_work_refusal.is_none()
            && receipt.physical_failure.is_none(),
        "{receipt:?}"
    );
}

mod source_custody;
