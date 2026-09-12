# filesystem_provider/tests.rs

- relative_byte_paths_have_an_exact_closed_grammar · function · L4-L32 — fn relative_byte_paths_have_an_exact_closed_grammar()
- fixture_read_create_new_and_denial_preserve_exact_bytes · function · L35-L68 — fn fixture_read_create_new_and_denial_preserve_exact_bytes()
- fixture_capacity_and_duplicate_inventory_fail_before_use · function · L71-L80 — fn fixture_capacity_and_duplicate_inventory_fail_before_use()
- fixture_v2_metadata_listing_directories_removal_and_atomic_replacement · function · L83-L136 — fn fixture_v2_metadata_listing_directories_removal_and_atomic_replacement()
- fixture_v2_invalid_operations_cannot_mutate_inventory · function · L139-L155 — fn fixture_v2_invalid_operations_cannot_mutate_inventory()
- physical · module · L158-L346 — mod physical
- NEXT · constant · L166-L166 — static NEXT: AtomicU64 = AtomicU64::new(0);
- Scratch · struct · L167-L167 — struct Scratch(std::path::PathBuf);
- new · function · L169-L177 — fn new() -> Self
- drop · function · L180-L182 — fn drop(&mut self)
- physical_roundtrip_zero_capacity_and_no_overwrite · function · L186-L217 — fn physical_roundtrip_zero_capacity_and_no_overwrite()
- symlinks_directories_and_traversal_cannot_escape_authority · function · L220-L254 — fn symlinks_directories_and_traversal_cannot_escape_authority()
- retained_root_descriptor_survives_path_replacement · function · L257-L271 — fn retained_root_descriptor_survives_path_replacement()
- physical_v2_metadata_listing_directories_removal_and_atomic_replacement · function · L274-L316 — fn physical_v2_metadata_listing_directories_removal_and_atomic_replacement()
- physical_v2_never_follows_or_replaces_symlinks_and_invalid_inputs_do_not_mutate · function · L319-L345 — fn physical_v2_never_follows_or_replaces_symlinks_and_invalid_inputs_do_not_mutate()
