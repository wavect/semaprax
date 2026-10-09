use super::ExpressionId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn equal_bytes_keep_value_traits_while_only_clones_share_backing() {
    let first = ExpressionId::from_owned(String::from("expression\0identity"));
    let independent = ExpressionId::from_owned(String::from("expression\0identity"));
    let clone = first.clone();

    assert_eq!(first.as_str(), "expression\0identity");
    assert_eq!(first, independent);
    assert_eq!(first, clone);
    assert_ne!(
        first.shared_allocation_key(),
        independent.shared_allocation_key()
    );
    assert_eq!(first.shared_allocation_key(), clone.shared_allocation_key());
    assert_eq!(
        first.shared_allocation_bytes(),
        clone.shared_allocation_bytes()
    );

    assert_eq!(first.to_string(), "expression\0identity");
    assert_eq!(
        format!("{first:?}"),
        "ExpressionId(\"expression\\0identity\")"
    );
    assert!(first < ExpressionId::from_owned(String::from("expression\0identity!")));
    assert_eq!(hash(&first), hash(&independent));
}

#[test]
fn constructor_charges_the_exact_arc_carrier_and_fails_closed_before_allocating() {
    let value = String::from("budgeted-expression");
    let retained_bytes = ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES + value.capacity();
    let carrier_charge = ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES;
    let (accepted, overflowed, used) =
        crate::bounded_output::with_limit_usage(carrier_charge, || ExpressionId::from_owned(value));
    assert!(!overflowed);
    assert_eq!(used, carrier_charge);
    assert_eq!(accepted.as_str(), "budgeted-expression");
    assert!(accepted.shared_allocation_key().is_some());
    assert_eq!(accepted.shared_allocation_bytes(), Some(retained_bytes));

    let rejected_value = String::from("budgeted-expression");
    let (refused, overflowed, used) =
        crate::bounded_output::with_limit_usage(carrier_charge - 1, || {
            ExpressionId::from_owned(rejected_value)
        });
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(refused.as_str(), "");
    assert_eq!(refused.shared_allocation_key(), None);
    assert_eq!(refused.shared_allocation_bytes(), None);
}

#[test]
fn cloning_an_identity_does_not_allocate_or_charge_again() {
    let original = ExpressionId::from_owned(String::from("clone-without-charge"));
    let (clone, overflowed, used) = crate::bounded_output::with_limit_usage(0, || original.clone());

    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(clone, original);
    assert_eq!(
        clone.shared_allocation_key(),
        original.shared_allocation_key()
    );
    assert_eq!(
        clone.shared_allocation_bytes(),
        original.shared_allocation_bytes()
    );
}
