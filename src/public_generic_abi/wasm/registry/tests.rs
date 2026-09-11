use super::*;

fn entry(role: HandleRole, offset: u32, len: u32) -> RegistryEntry {
    RegistryEntry { role, offset, len }
}

#[test]
fn insert_then_get_round_trips() {
    let mut registry = HandleRegistry::new();
    let handle = Handle::root(1);
    registry.insert(handle, entry(HandleRole::InputRoot, 0, 0));
    let found = registry.get(handle, HandleRole::InputRoot).unwrap();
    assert_eq!(found.offset, 0);
}

#[test]
fn get_rejects_a_handle_never_registered() {
    let registry = HandleRegistry::new();
    let error = registry
        .get(Handle::root(1), HandleRole::InputRoot)
        .unwrap_err();
    assert_eq!(error.code, HANDLE_INVALID);
}

#[test]
fn get_rejects_a_stale_generation() {
    let mut registry = HandleRegistry::new();
    registry.insert(Handle::root(1), entry(HandleRole::InputRoot, 0, 0));
    let error = registry
        .get(Handle::root(2), HandleRole::InputRoot)
        .unwrap_err();
    assert_eq!(error.code, HANDLE_INVALID);
}

#[test]
fn get_rejects_wrong_kind() {
    let mut registry = HandleRegistry::new();
    let handle = Handle::leaf(0, 1);
    registry.insert(handle, entry(HandleRole::InputLeaf, 0, 4));
    let error = registry.get(handle, HandleRole::ResultLeaf).unwrap_err();
    assert_eq!(error.code, WRONG_KIND);
}

#[test]
fn remove_then_get_fails_double_release_closed() {
    let mut registry = HandleRegistry::new();
    let handle = Handle::leaf(0, 1);
    registry.insert(handle, entry(HandleRole::InputLeaf, 0, 4));
    registry.remove(handle, HandleRole::InputLeaf).unwrap();
    assert!(registry.is_empty());
    let error = registry.get(handle, HandleRole::InputLeaf).unwrap_err();
    assert_eq!(error.code, HANDLE_INVALID);
}

#[test]
fn remove_rejects_wrong_kind_without_removing() {
    let mut registry = HandleRegistry::new();
    let handle = Handle::leaf(0, 1);
    registry.insert(handle, entry(HandleRole::InputLeaf, 0, 4));
    let error = registry.remove(handle, HandleRole::ResultLeaf).unwrap_err();
    assert_eq!(error.code, WRONG_KIND);
    // The rejected attempt did not remove the entry.
    registry.get(handle, HandleRole::InputLeaf).unwrap();
}

#[test]
fn live_leaves_in_order_is_structural_not_insertion_order() {
    let mut registry = HandleRegistry::new();
    registry.insert(Handle::leaf(2, 1), entry(HandleRole::InputLeaf, 8, 1));
    registry.insert(Handle::leaf(0, 1), entry(HandleRole::InputLeaf, 0, 1));
    registry.insert(Handle::leaf(1, 1), entry(HandleRole::InputLeaf, 4, 1));
    registry.insert(Handle::root(1), entry(HandleRole::InputRoot, 0, 0));
    let leaves = registry.live_leaves_in_order(HandleRole::InputLeaf);
    let ids: Vec<u32> = leaves.iter().map(|(handle, _)| handle.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn two_registries_are_independent_so_a_foreign_handle_is_simply_absent() {
    let mut registry_a = HandleRegistry::new();
    let registry_b = HandleRegistry::new();
    let handle = Handle::leaf(0, 1);
    registry_a.insert(handle, entry(HandleRole::InputLeaf, 0, 4));
    // The identical (id, generation) pair minted by a different provider's
    // registry is not live in this one.
    let error = registry_b.get(handle, HandleRole::InputLeaf).unwrap_err();
    assert_eq!(error.code, HANDLE_INVALID);
}
