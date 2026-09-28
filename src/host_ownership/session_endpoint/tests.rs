use super::*;
use crate::host_ownership::HostIdentity;

fn provenance() -> HostResourceProvenance {
    HostResourceProvenance::try_new(
        HostIdentity::try_new("module").unwrap(),
        HostIdentity::try_new("adapter").unwrap(),
        HostIdentity::try_new(TOKEN_RESOURCE).unwrap(),
        HostIdentity::try_new(TOKEN_LIFECYCLE).unwrap(),
        1,
    )
    .unwrap()
}

#[test]
fn fresh_token_acquisition_owns_distinct_real_cells_and_retires_exactly_once() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let first = registry.acquire_fresh_token(provenance()).unwrap();
    let second = registry.acquire_fresh_token(provenance()).unwrap();
    let a = first.weak(&registry);
    let b = second.weak(&registry);
    assert!(!std::sync::Weak::ptr_eq(&a, &b));
    assert_eq!(a.strong_count(), 1);
    assert_eq!(b.strong_count(), 1);
    assert_eq!(first.producer(), FRESH_TOKEN_PRODUCER);
    assert_eq!(first.generation(), 1);
    assert_eq!(registry.live_owner_count(), 2);
    first.retire(&mut registry).unwrap();
    assert!(a.upgrade().is_none());
    assert_eq!(
        first.retire(&mut registry),
        Err(HostBoundaryRejection::OwnerNotLive)
    );
    assert_eq!(registry.live_owner_count(), 1);
    second.validate(&registry).unwrap();
    second.retire(&mut registry).unwrap();
    assert!(b.upgrade().is_none());
    assert_eq!(registry.live_owner_count(), 0);
}

#[test]
fn fresh_token_acquisition_credential_failure_rollback_restores_slot_not_origin() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let first = registry.acquire_fresh_token(provenance()).unwrap();
    let cell = first.weak(&registry);
    let slot = first.slot();
    first.rollback(&mut registry).unwrap();
    assert!(cell.upgrade().is_none());
    assert_eq!(registry.live_owner_count(), 0);
    let second = registry.acquire_fresh_token(provenance()).unwrap();
    assert_eq!(second.slot(), slot);
    assert_ne!(second.acquisition, first.acquisition);
    // Even an identical slot/generation after a prepublication rollback does
    // not authenticate the previous actual acquisition certificate.
    assert_eq!(
        first.validate(&registry),
        Err(HostBoundaryRejection::StaleOwner)
    );
    second.retire(&mut registry).unwrap();
}

#[test]
fn fresh_token_acquisition_foreign_registry_cannot_retire_or_rollback_owner() {
    let mut one = HostOwnershipRegistry::try_new().unwrap();
    let mut two = HostOwnershipRegistry::try_new().unwrap();
    let owner = one.acquire_fresh_token(provenance()).unwrap();
    let other = two.acquire_fresh_token(provenance()).unwrap();
    assert!(owner.retire(&mut two).is_err());
    assert!(owner.rollback(&mut two).is_err());
    owner.validate(&one).unwrap();
    other.validate(&two).unwrap();
    assert_eq!((one.live_owner_count(), two.live_owner_count()), (1, 1));
    owner.retire(&mut one).unwrap();
    other.retire(&mut two).unwrap();
}

#[test]
fn fresh_token_acquisition_refuses_wrong_shape_and_exhaustion_before_registration() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let mut wrong = provenance();
    wrong.resource_type = HostIdentity::try_new("other.type").unwrap();
    assert!(matches!(
        registry.acquire_fresh_token(wrong),
        Err(HostBoundaryRejection::WrongResourceType)
    ));
    let mut wrong = provenance();
    wrong.lifecycle = HostIdentity::try_new("other.drop").unwrap();
    assert!(matches!(
        registry.acquire_fresh_token(wrong),
        Err(HostBoundaryRejection::WrongLifecycle)
    ));
    let mut raw = provenance();
    raw.resource_type = HostIdentity::try_new("token.type").unwrap();
    assert_eq!(
        registry.acquire_fresh_token(raw).unwrap_err(),
        HostBoundaryRejection::WrongResourceType
    );
    let mut raw = provenance();
    raw.lifecycle = HostIdentity::try_new("token.drop").unwrap();
    assert_eq!(
        registry.acquire_fresh_token(raw).unwrap_err(),
        HostBoundaryRejection::WrongLifecycle
    );
    registry.next_slot = u64::MAX;
    assert!(matches!(
        registry.acquire_fresh_token(provenance()),
        Err(HostBoundaryRejection::RegistryExhausted)
    ));
    assert_eq!(registry.live_owner_count(), 0);
    assert!(registry.fresh_cells.is_empty());
}

#[test]
fn fresh_token_acquisition_poison_refusal_preserves_actual_registered_backing() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry.acquire_fresh_token(provenance()).unwrap();
    let cell = owner.weak(&registry);
    registry.poisoned = true;
    assert_eq!(
        owner.retire(&mut registry),
        Err(HostBoundaryRejection::RegistryPoisoned)
    );
    assert!(matches!(
        registry.acquire_fresh_token(provenance()),
        Err(HostBoundaryRejection::RegistryPoisoned)
    ));
    assert_eq!(registry.live_owner_count(), 1);
    assert_eq!(cell.strong_count(), 1);
}

#[test]
fn fresh_token_acquisition_exact_cell_cap_refuses_plus_one_without_owner_change() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let mut owners = Vec::with_capacity(MAX_FRESH_TOKEN_CELLS);
    for _ in 0..MAX_FRESH_TOKEN_CELLS {
        owners.push(registry.acquire_fresh_token(provenance()).unwrap());
    }
    let next = registry.next_slot;
    assert!(matches!(
        registry.acquire_fresh_token(provenance()),
        Err(HostBoundaryRejection::RegistryExhausted)
    ));
    assert_eq!(registry.next_slot, next);
    assert_eq!(registry.live_owner_count(), MAX_FRESH_TOKEN_CELLS);
    for owner in owners {
        owner.retire(&mut registry).unwrap();
    }
    assert_eq!(registry.live_owner_count(), 0);
    assert!(registry.fresh_cells.is_empty());
}
