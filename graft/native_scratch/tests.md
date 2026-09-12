# native_scratch/tests.rs

- root · function · L4-L16 — fn root() -> PathBuf
- finish · function · L20-L64 — fn finish(root: &Path, directories: &[&str], files: &[(&str, &[u8])])
- created_source_is_sealed_and_only_explicit_cleanup_removes_it · function · L67-L87 — fn created_source_is_sealed_and_only_explicit_cleanup_removes_it()
- output_is_absent_until_link_then_adopted_and_cleaned · function · L90-L101 — fn output_is_absent_until_link_then_adopted_and_cleaned()
- colliding_files_and_directories_are_not_adopted_or_modified · function · L104-L123 — fn colliding_files_and_directories_are_not_adopted_or_modified()
- drop_and_unsealed_cleanup_preserve_partial_outputs · function · L126-L149 — fn drop_and_unsealed_cleanup_preserve_partial_outputs()
- foreign_inventory_stops_cleanup_before_the_owned_file_is_removed · function · L152-L164 — fn foreign_inventory_stops_cleanup_before_the_owned_file_is_removed()
- invalid_leaf_names_are_rejected_before_directory_creation · function · L167-L173 — fn invalid_leaf_names_are_rejected_before_directory_creation()
- multiply_linked_output_is_not_adopted_for_cleanup · function · L176-L188 — fn multiply_linked_output_is_not_adopted_for_cleanup()
- replaced_file_and_directory_preserve_original_and_foreign_objects · function · L192-L217 — fn replaced_file_and_directory_preserve_original_and_foreign_objects()
- displaced_parent_preserves_both_trees · function · L221-L248 — fn displaced_parent_preserves_both_trees()
- dangling_collision_and_output_symlink_are_preserved · function · L252-L304 — fn dangling_collision_and_output_symlink_are_preserved()
