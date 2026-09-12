# project/npm/semantic_recipe_v8/type_names.rs

- PREFIX · constant · L17-L17 — const PREFIX: &str = "// semaprax-owned-data-type-names.v1 ";
- Rows · type · L18-L18 — type Rows = Vec<[String; 2]>;
- alias · function · L20-L22 — fn alias(index: usize) -> String
- apply_aliases · function · L27-L42 — pub(super) fn apply_aliases(
- render_header · function · L44-L61 — fn render_header(rows: &[[String; 2]]) -> Result<String, Diagnostic>
- read_header · function · L67-L104 — pub(super) fn read_header(recipe: &str) -> Result<(Option<Rows>, &str), Diagnostic>
- validate_name · function · L106-L125 — fn validate_name(name: &str) -> Result<(), Diagnostic>
- restore · function · L127-L219 — pub(super) fn restore(
- tests · module · L222-L222 — mod tests;
