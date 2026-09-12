# codegen/native_module_lease/tests.rs

- assert_not_impl · function · L6-L16 — macro_rules! assert_not_impl
- FINGERPRINT · constant · L18-L18 — const FINGERPRINT: [u8; 32] = [0xa5; 32];
- ORIGIN · constant · L19-L22 — const ORIGIN: NativeProcessIncarnation = NativeProcessIncarnation
- fixture · function · L24-L29 — fn fixture() -> (NativeModuleLease, Arc<FakeRetainedPinProbe>)
- identical_fingerprints_do_not_conflate_loaded_instances · function · L32-L48 — fn identical_fingerprints_do_not_conflate_loaded_instances()
- draining_rejects_new_retention_without_revoking_existing_leases · function · L51-L70 — fn draining_rejects_new_retention_without_revoking_existing_leases()
- drain_committing_after_temporary_clone_forces_retain_to_reject · function · L73-L101 — fn drain_committing_after_temporary_clone_forces_retain_to_reject()
- wrong_process_incarnation_precedes_state_and_cannot_start_drain · function · L104-L130 — fn wrong_process_incarnation_precedes_state_and_cannot_start_drain()
- concurrent_last_releases_trigger_the_fake_pin_exactly_once · function · L133-L154 — fn concurrent_last_releases_trigger_the_fake_pin_exactly_once()
- THREADS · constant · L134-L134 — const THREADS: usize = 16;
- leaf_pin_has_no_retention_backedge · function · L157-L165 — fn leaf_pin_has_no_retention_backedge()
- fake_construction_rejects_uninitialized_identity_without_releasing · function · L168-L192 — fn fake_construction_rejects_uninitialized_identity_without_releasing()
- lease_traits_are_deliberate · function · L195-L202 — fn lease_traits_are_deliberate()
- assert_send_and_sync · function · L196-L196 — fn assert_send_and_sync<T: Send + Sync>() {}
