# arc_zones/tests.rs

- MAIN · constant · L3-L3 — const MAIN: &str = "main";
- two_zone_model · function · L5-L18 — fn two_zone_model() -> ArcZonesModel
- drain · function · L20-L30 — fn drain(run: &mut ArcZonesRun<'_>) -> Vec<ArcZoneEvent>
- constructor_rejects_structural_ambiguity · function · L33-L86 — fn constructor_rejects_structural_ambiguity()
- hostile_operations_fail_closed · function · L89-L168 — fn hostile_operations_fail_closed()
- link_and_unlink_hostility_and_cascade_order · function · L171-L234 — fn link_and_unlink_hostility_and_cascade_order()
- self_loop_is_rejected_at_zone_exit_with_canonical_witness · function · L237-L259 — fn self_loop_is_rejected_at_zone_exit_with_canonical_witness()
- determinism_survives_inventory_permutation · function · L262-L289 — fn determinism_survives_inventory_permutation()
- strong_count_tracks_fan_out_and_demotion_preconditions · function · L292-L317 — fn strong_count_tracks_fan_out_and_demotion_preconditions()
- projections_are_valid_json_and_domain_separated · function · L320-L343 — fn projections_are_valid_json_and_domain_separated()
