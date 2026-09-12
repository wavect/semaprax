# project/npm/semantic_recipe_v8/source_literals_tests.rs

- SOURCE · constant · L3-L13 — const SOURCE: &str = r#"module recipe.source_literals;
- HISTORICAL_RECIPE · constant · L15-L18 — const HISTORICAL_RECIPE: &str = "module semaprax_npm_recipe;\n\n\
- resolved · function · L20-L23 — fn resolved(source: &str) -> crate::hir::ResolvedProgram
- source_identity_quoting_preserves_historical_valid_recipe_bytes · function · L26-L31 — fn source_identity_quoting_preserves_historical_valid_recipe_bytes()
- every_authored_identity_role_uses_source_escapes_not_json_escapes · function · L34-L69 — fn every_authored_identity_role_uses_source_escapes_not_json_escapes()
- SOURCE_SUFFIX · constant · L37-L37 — const SOURCE_SUFFIX: &str = r#"\u{8}\u{c}\u{7f}\u{85}é\n\r\t\"\\"#;
- VALUE_SUFFIX · constant · L38-L38 — const VALUE_SUFFIX: &str = "\u{8}\u{c}\u{7f}\u{85}é\n\r\t\"\\";
- RECIPE_SUFFIX · constant · L39-L39 — const RECIPE_SUFFIX: &str = "\\u{8}\\u{c}\\u{7f}\u{85}é\\n\\r\\t\\\"\\\\";
