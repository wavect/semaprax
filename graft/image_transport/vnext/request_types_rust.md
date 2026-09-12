# image_transport/vnext/request_types_rust.rs

- SUPPORT · constant · L8-L31 — const SUPPORT: &str = r#"
- LITERAL_SUPPORT · constant · L33-L51 — const LITERAL_SUPPORT: &str = r#"
- emit · function · L53-L151 — pub(super) fn emit(model: &Model) -> Result<String>
- transparent · function · L153-L155 — fn transparent(source: &mut String, name: &str, inner: &str)
- literal · function · L157-L180 — fn literal(source: &mut String, name: &str, value: &serde_json::Value) -> Result<()>
- rust_field · function · L182-L189 — fn rust_field(name: &str) -> bool
- sequence · function · L191-L214 — fn sequence(source: &mut String, name: &str, items: &[String])
- tests · module · L217-L350 — mod tests
- recursive_edges_are_structural_newtypes_and_boxed_without_json_fallback · function · L224-L261 — fn recursive_edges_are_structural_newtypes_and_boxed_without_json_fallback()
- nullable_required_fields_and_optional_fields_keep_different_wire_presence · function · L264-L317 — fn nullable_required_fields_and_optional_fields_keep_different_wire_presence()
- literal_markers_preserve_negative_and_full_unsigned_json_integer_ranges · function · L320-L334 — fn literal_markers_preserve_negative_and_full_unsigned_json_integer_ranges()
- long_sequences_retain_positions_and_reject_extra_items_without_tuple_trait_limits · function · L337-L349 — fn long_sequences_retain_positions_and_reject_extra_items_without_tuple_trait_limits()
