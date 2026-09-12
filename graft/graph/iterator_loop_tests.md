# graph/iterator_loop_tests.rs

- SOURCE · constant · L5-L13 — const SOURCE: &str = r#"
- checked · function · L15-L18 — fn checked() -> crate::ast::Program
- resolved · function · L20-L22 — fn resolved() -> crate::hir::ResolvedProgram
- main_function · function · L24-L30 — fn main_function(program: &crate::hir::ResolvedProgram) -> &crate::hir::ResolvedFunction
- iterator_loop_mut · function · L32-L63 — fn iterator_loop_mut(
- iterator_loop_is_canonical_and_selects_v39_v11 · function · L66-L92 — fn iterator_loop_is_canonical_and_selects_v39_v11()
- changed_iterator_loop_condition_or_next_is_rejected · function · L95-L122 — fn changed_iterator_loop_condition_or_next_is_rejected()
- iterator_loop_cache_tag_is_additive_and_cleanup_downgrade_rejects · function · L125-L168 — fn iterator_loop_cache_tag_is_additive_and_cleanup_downgrade_rejects()
- iterator_loop_unused_template_retains_v39_without_concrete_plan · function · L171-L235 — fn iterator_loop_unused_template_retains_v39_without_concrete_plan()
