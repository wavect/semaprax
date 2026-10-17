//! Full local scalar state parity against untouched reserve(0).
use super::*;
#[test]
fn held_physical_zero_preserves_all_fields_first_refusal_and_enclosing_floor() {
    for refused in [None,Some(PhysicalRefusal::Overflow),Some(PhysicalRefusal::LocalLimit),Some(PhysicalRefusal::EnclosingBudget)] {
        for floor in [false,true] {
            let observe=|unit| crate::bounded_output::with_limit_usage_refusal(7,||{
                let mut physical=PhysicalReservation { remaining:3,charged:usize::MAX,refusal:refused,direct_anchor:None };
                if floor { assert!(crate::bounded_output::set_active_floor(7)); }
                let admitted=if unit { physical.authenticate_zero() } else { physical.reserve(0).is_ok() };
                (admitted,physical.remaining,physical.charged,physical.refusal)
            });
            assert_eq!(observe(false),observe(true));
        }
    }
}
