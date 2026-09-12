# cleanup_plan/nested_record_destructure_tests.rs

- SOURCE · constant · L10-L46 — const SOURCE: &str = r#"
- program · function · L48-L54 — fn program() -> hir::ResolvedProgram
- function · function · L56-L62 — fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction
- v8_exact_destructure_transfers_recursive_leaves_atomically_in_declaration_order · function · L65-L142 — fn v8_exact_destructure_transfers_recursive_leaves_atomically_in_declaration_order()
- whole_nested_move_and_flat_record_match_preserve_legacy_schemas · function · L145-L169 — fn whole_nested_move_and_flat_record_match_preserve_legacy_schemas()
- v9_nested_update_replays_subtrees_and_rejects_mutation · function · L172-L248 — fn v9_nested_update_replays_subtrees_and_rejects_mutation()
