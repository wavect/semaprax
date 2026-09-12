# image_transport/vnext/response_types_rust.rs

- SUPPORT · constant · L6-L43 — const SUPPORT: &str = r#"
- RECURSIVE_SUPPORT · constant · L45-L88 — const RECURSIVE_SUPPORT: &str = r#"
- LITERAL_SUPPORT · constant · L93-L113 — const LITERAL_SUPPORT: &str = r#"
- emit · function · L115-L276 — pub(super) fn emit(model: &Model) -> Result<String>
- transparent · function · L278-L280 — fn transparent(source: &mut String, name: &str, inner: &str)
- recursive_union · function · L282-L317 — fn recursive_union(
- terminal_shape · function · L319-L344 — fn terminal_shape<'a>(
- scalar_guard · function · L346-L365 — fn scalar_guard(shape: &Shape, value: &str) -> Option<String>
- branch_guard · function · L367-L403 — fn branch_guard(target: &str, shapes: &BTreeMap<&str, &Shape>, work: &mut usize) -> Result<String>
- rust_field · function · L405-L412 — fn rust_field(name: &str) -> bool
- sequence · function · L414-L437 — fn sequence(source: &mut String, name: &str, items: &[String])
