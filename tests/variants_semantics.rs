use std::path::Path;

use semaprax::{format, graph, parse, verify};

const VARIANTS: &str = r#"
module test.variants;

@id("test.choice")
variant Choice {
    @id("test.choice.none")
    None,
    @id("test.choice.number")
    Number {
        @id("test.choice.number.value")
        value: i64,
    },
    @id("test.choice.flag")
    Flag {
        @id("test.choice.flag.value")
        value: bool,
    },
}

@id("test.choose")
fn choose(choice: Choice) -> i64 {
    match choice {
        Choice::Number { value: number } => number,
        Choice::Flag { value } => if value { 1 } else { 0 },
        Choice::None {} => 0,
    }
}

@id("app.main")
fn main() -> i64 {
    choose(Choice::Number { value: 42 })
}
"#;

fn codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("variants-semantics.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn variants_and_exhaustive_match_have_a_canonical_human_projection() {
    let program = parse(VARIANTS, Path::new("variants.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("variants-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert!(canonical.contains("variant Choice {"));
    assert!(canonical.contains("Choice::Number { value: 42 }"));
    assert!(canonical.contains("Choice::Number { value: number } => number,"));
    assert!(canonical.contains("Choice::None {} => 0,"));
}

#[test]
fn wildcard_is_an_explicit_final_exhaustive_fallback() {
    let source = VARIANTS.replace(
        "Choice::Flag { value } => if value { 1 } else { 0 },\n        Choice::None {} => 0,",
        "_ => 0,",
    );
    assert!(codes(&source).is_empty());
}

#[test]
fn construction_diagnostics_preserve_authored_then_declaration_order() {
    let source = r#"
module test.variant_constructor_errors;
@id("test.payload")
variant Payload {
    @id("test.payload.item")
    Item {
        @id("test.payload.item.value") value: i64,
        @id("test.payload.item.flag") flag: bool,
    },
}
@id("app.main")
fn main() -> i64 {
    let value = Payload::Item { missing: 1, value: true, value: 2 };
    match value { _ => 0, }
}
"#;
    assert_eq!(
        codes(source),
        ["SPX-T212", "SPX-T215", "SPX-T212", "SPX-T213"]
    );
}

#[test]
fn empty_variants_and_non_scalar_payloads_are_rejected() {
    let source = r#"
module test.variant_shape_errors;
@id("test.record")
record Record { @id("test.record.value") value: i64, }
@id("test.empty")
variant Empty {}
@id("test.invalid")
variant Invalid {
    @id("test.invalid.record") RecordCase {
        @id("test.invalid.record.value") value: Record,
    },
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(codes(source), ["SPX-T215", "SPX-T215"]);
}

#[test]
fn match_reports_a_deterministic_first_missing_witness() {
    let source = VARIANTS.replace(
        "        Choice::Flag { value } => if value { 1 } else { 0 },\n        Choice::None {} => 0,\n",
        "",
    );
    let program = parse(&source, Path::new("missing-case.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    let missing = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-M101")
        .unwrap();
    assert!(missing.message.contains("Choice::None {}"));
}

#[test]
fn duplicate_and_post_wildcard_arms_are_unreachable() {
    let duplicate = VARIANTS.replace(
        "        Choice::Flag { value } => if value { 1 } else { 0 },",
        "        Choice::Number { value } => value,\n        Choice::Flag { value } => if value { 1 } else { 0 },",
    );
    assert_eq!(codes(&duplicate), ["SPX-M102"]);

    let post_wildcard = VARIANTS.replace(
        "        Choice::Number { value: number } => number,",
        "        _ => 0,\n        Choice::Number { value: number } => number,",
    );
    assert_eq!(codes(&post_wildcard), ["SPX-M102", "SPX-M102", "SPX-M102"]);
}

#[test]
fn patterns_and_arm_results_are_checked_independently() {
    let incompatible = VARIANTS.replace(
        "        Choice::Number { value: number } => number,",
        "        Other::Number { value: number } => number,",
    );
    assert_eq!(
        codes(&incompatible),
        ["SPX-M103", "SPX-M104", "SPX-T202", "SPX-M101"]
    );

    let fields = VARIANTS.replace(
        "Choice::Number { value: number } => number,",
        "Choice::Number { missing, value: number } => number,",
    );
    assert_eq!(codes(&fields), ["SPX-M104"]);

    let arm_type = VARIANTS.replace("Choice::None {} => 0,", "Choice::None {} => false,");
    assert_eq!(codes(&arm_type), ["SPX-T216"]);
}

#[test]
fn malformed_qualification_and_match_arrows_are_parser_errors() {
    let bad_constructor = VARIANTS.replace(
        "Choice::Number { value: 42 }",
        "Choice:Number { value: 42 }",
    );
    assert_eq!(
        parse(&bad_constructor, Path::new("bad-constructor.spx"))
            .unwrap_err()
            .code,
        "SPX-P106"
    );

    let bad_arrow = VARIANTS.replacen("=> number", "-> number", 1);
    assert_eq!(
        parse(&bad_arrow, Path::new("bad-arrow.spx"))
            .unwrap_err()
            .code,
        "SPX-P106"
    );
}
