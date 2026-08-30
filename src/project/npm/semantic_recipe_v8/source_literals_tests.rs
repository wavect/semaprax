use super::*;

const SOURCE: &str = r#"module recipe.source_literals;
@id("recipe.record") record Packet {
    @id("recipe.record.field") value: i64,
}
@id("recipe.variant") variant Choice {
    @id("recipe.variant.case") Ready {
        @id("recipe.variant.case.field") value: i64,
    },
}
@id("recipe.function") fn main() -> i64 { 0 }
"#;

const HISTORICAL_RECIPE: &str = "module semaprax_npm_recipe;\n\n\
@id(\"recipe.record\")\nrecord Packet {\n    @id(\"recipe.record.field\")\n    value: i64,\n}\n\n\
@id(\"recipe.variant\")\nvariant Choice {\n    @id(\"recipe.variant.case\")\n    Ready {\n        @id(\"recipe.variant.case.field\")\n        value: i64,\n    },\n}\n\n\
@id(\"recipe.function\")\nfn main() -> i64\n{ 0 }\n\n";

fn resolved(source: &str) -> crate::hir::ResolvedProgram {
    let checked = crate::check(source, "recipe-source-literals.spx").unwrap();
    crate::hir::resolve(&checked).unwrap()
}

#[test]
fn source_identity_quoting_preserves_historical_valid_recipe_bytes() {
    let program = resolved(SOURCE);
    assert_eq!(render(&program).unwrap(), HISTORICAL_RECIPE);
    let replayed = replay_against(&program, HISTORICAL_RECIPE).unwrap();
    assert_eq!(render(&replayed).unwrap(), HISTORICAL_RECIPE);
}

#[test]
fn every_authored_identity_role_uses_source_escapes_not_json_escapes() {
    // Exercise the six @id sites independently. The source and expected recipe
    // suffixes are literal oracles, not calls to the implementation's formatter.
    const SOURCE_SUFFIX: &str = r#"\u{8}\u{c}\u{7f}\u{85}é\n\r\t\"\\"#;
    const VALUE_SUFFIX: &str = "\u{8}\u{c}\u{7f}\u{85}é\n\r\t\"\\";
    const RECIPE_SUFFIX: &str = "\\u{8}\\u{c}\\u{7f}\u{85}é\\n\\r\\t\\\"\\\\";
    for id in [
        "recipe.function",
        "recipe.record",
        "recipe.record.field",
        "recipe.variant",
        "recipe.variant.case",
        "recipe.variant.case.field",
    ] {
        let old_annotation = format!("@id(\"{id}\")");
        assert_eq!(SOURCE.matches(old_annotation.as_str()).count(), 1);
        let source = SOURCE.replacen(&old_annotation, &format!("@id(\"{id}{SOURCE_SUFFIX}\")"), 1);
        let program = resolved(&source);
        let identity = DeclarationId::new(format!("{id}{VALUE_SUFFIX}"));
        assert!(program.declarations.declaration(&identity).is_some());
        let recipe = render(&program).unwrap();
        let annotation = format!("@id(\"{id}{RECIPE_SUFFIX}\")");
        assert_eq!(recipe.matches(annotation.as_str()).count(), 1, "{id}");
        let replayed = replay_against(&program, &recipe).unwrap();
        assert!(replayed.declarations.declaration(&identity).is_some());
        assert_eq!(render(&replayed).unwrap(), recipe);

        let json_annotation = format!("@id({})", crate::diagnostic::quote_json(identity.as_str()));
        assert!(json_annotation.contains("\\u0008"));
        let old_json_recipe = recipe.replacen(&annotation, &json_annotation, 1);
        assert_ne!(old_json_recipe, recipe);
        let error = replay(&old_json_recipe).unwrap_err();
        assert_eq!(error.code, "SPX-W120");
        assert!(error.message.contains("does not parse"));
    }
}
