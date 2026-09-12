# public_generic_abi/wasm/registry/tests.rs

- entry · function · L3-L5 — fn entry(role: HandleRole, offset: u32, len: u32) -> RegistryEntry
- insert_then_get_round_trips · function · L8-L14 — fn insert_then_get_round_trips()
- get_rejects_a_handle_never_registered · function · L17-L23 — fn get_rejects_a_handle_never_registered()
- get_rejects_a_stale_generation · function · L26-L33 — fn get_rejects_a_stale_generation()
- get_rejects_wrong_kind · function · L36-L42 — fn get_rejects_wrong_kind()
- remove_then_get_fails_double_release_closed · function · L45-L53 — fn remove_then_get_fails_double_release_closed()
- remove_rejects_wrong_kind_without_removing · function · L56-L64 — fn remove_rejects_wrong_kind_without_removing()
- live_leaves_in_order_is_structural_not_insertion_order · function · L67-L76 — fn live_leaves_in_order_is_structural_not_insertion_order()
- two_registries_are_independent_so_a_foreign_handle_is_simply_absent · function · L79-L88 — fn two_registries_are_independent_so_a_foreign_handle_is_simply_absent()
