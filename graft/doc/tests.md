# doc/tests.rs

- DISORDERED · constant · L9-L55 — const DISORDERED: &str = "module test.docorder;\n\
- parsed · function · L57-L59 — fn parsed(source: &str) -> (Program, Comments)
- ids · function · L61-L67 — fn ids(document: &Document) -> Vec<(&str, &str)>
- declaration_order_is_list_order_then_source_order · function · L75-L111 — fn declaration_order_is_list_order_then_source_order()
- markdown_groups_by_kind_while_json_keeps_list_order · function · L118-L176 — fn markdown_groups_by_kind_while_json_keeps_list_order()
- both_renderings_are_byte_identical_across_repeated_generation · function · L183-L201 — fn both_renderings_are_byte_identical_across_repeated_generation()
- json_escapes_comment_and_identity_text_without_altering_it · function · L208-L264 — fn json_escapes_comment_and_identity_text_without_altering_it()
- the_smallest_admissible_module_still_renders_a_complete_document · function · L273-L350 — fn the_smallest_admissible_module_still_renders_a_complete_document()
