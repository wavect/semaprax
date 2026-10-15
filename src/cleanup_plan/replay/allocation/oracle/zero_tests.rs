//! Exact independent zero-charge frontier, including otherwise unreachable overflow.
use super::*;
#[test]
fn held_oracle_unit_zero_keeps_complete_charge_receipt_counter_overflow_and_expiration() {
    for calls in [0,usize::MAX-1,usize::MAX] {
        for sticky in [false,true] {
            let observe=|unit| {
                let scope=OracleScope::open(0).unwrap();let lease=scope.lease();
                ACCOUNT.with(|account|{
                    let mut state=account.get().unwrap();state.receipt.calls=calls;
                    if sticky { state.receipt.physical_failure=Some("first physical"); }
                    account.set(Some(state));
                });
                let admitted=if unit { lease.authenticate_zero() } else { lease.charge(0).is_ok() };
                let receipt=scope.receipt();drop(scope);(admitted,receipt)
            };
            assert_eq!(observe(false),observe(true));
        }
    }
    let expired={let scope=OracleScope::open(0).unwrap();scope.lease()};
    assert!(!expired.authenticate_zero());assert_eq!(expired.charge(0),Err(OracleRefusal::Missing));
    let scope=OracleScope::open(0).unwrap();let before=scope.receipt();
    assert!(!expired.authenticate_zero());assert_eq!(expired.charge(0),Err(OracleRefusal::Expired));
    assert_eq!(scope.receipt(),before);
}
