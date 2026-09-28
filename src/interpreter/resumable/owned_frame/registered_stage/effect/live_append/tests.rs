use super::*;
#[test]
fn owned_wait_live_effect_authorization_ack_requires_exact_closed_adjacent_refs() {
    assert!(closed_references(19, 20, 21));
    for refs in [
        (18, 20, 21),
        (19, 21, 22),
        (19, 20, 22),
        (20, 20, 21),
        (19, 20, 20),
        (u32::MAX, 0, 1),
        (u32::MAX - 1, u32::MAX, 0),
    ] {
        assert!(!closed_references(refs.0, refs.1, refs.2), "{refs:?}");
    }
}
