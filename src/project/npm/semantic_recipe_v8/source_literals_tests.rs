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

#[test]
fn bounded_string_literal_preserves_bytes_and_restores_candidate_selection() {
    let value = "\u{feff}\0世é🙂\"\\\n\r\t\u{7f}";
    let expected = concat!("\"\u{feff}", r#"\u{0}世é🙂\"\\\n\r\t\u{7f}"#, "\"");
    let (literal, counts) = crate::kernel_zero::rung_two_authority::with_counts(|| {
        bounded_string_literal(value, expected.len())
    });
    assert_eq!(literal.unwrap(), expected);
    assert_eq!(
        counts, [0; 5],
        "bounded literal must not run proof candidates"
    );
    assert!(crate::bounded_output::active_limit().is_none());
    let (ordinary, counts) = crate::kernel_zero::rung_two_authority::with_counts(|| {
        crate::format::canonical_string("a")
    });
    assert_eq!(ordinary, "\"a\"");
    assert_eq!(counts, [0, 0, 0, 0, 1]);
}

#[test]
fn bounded_string_literal_refuses_plus_one_and_accounts_for_parent_budget() {
    let error = bounded_string_literal("\0", 6).unwrap_err();
    assert_eq!(error.code, "SPX-W120");
    assert_eq!(
        error.message,
        "owned-data semantic recipe exceeds its byte limit"
    );
    assert!(crate::bounded_output::active_limit().is_none());
    assert_eq!(bounded_string_literal("\0", 7).unwrap(), r#""\u{0}""#);
    let ((literal, remaining), overflowed) = crate::bounded_output::with_limit(8, || {
        let literal = bounded_string_literal("\0", MAX_RECIPE_BYTES).unwrap();
        (literal, crate::bounded_output::active_remaining())
    });
    assert_eq!(literal, r#""\u{0}""#);
    assert_eq!(remaining, Some(1));
    assert!(!overflowed);
    let ((result, remaining), overflowed) = crate::bounded_output::with_limit(6, || {
        let result = bounded_string_literal("\0", MAX_RECIPE_BYTES);
        (result, crate::bounded_output::active_remaining())
    });
    assert!(result.is_err());
    assert_eq!(remaining, Some(0));
    // The nested writer refuses the seventh byte without overspending its
    // parent. Its own Result carries the refusal, as with_limit contracts.
    assert!(!overflowed);
}
