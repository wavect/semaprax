use super::*;
use crate::interpreter::{self as engine, RenderFacts};

const SOURCE: &str = r#"module test.internal_string_profile;
@id("helper") fn helper(value: string) -> string { value }
@id("entry") fn main() -> i64 { string_len(helper("a\u{0}b")) }
@id("scalar") fn scalar() -> i64 { 42 }
"#;

fn resolved() -> hir::ResolvedProgram {
    let ast = crate::parse(SOURCE, Path::new("profile.spx")).unwrap();
    let diagnostics = crate::verify::verify(&ast);
    assert!(
        !diagnostics.iter().any(|item| item.severity.is_error()),
        "{diagnostics:?}"
    );
    hir::resolve(&ast).unwrap()
}

#[test]
fn only_the_opt_in_map_admits_string_signatures() {
    let program = resolved();
    assert_eq!(program.entrypoint.as_str(), "entry");
    let legacy = engine::admitted_resolved_functions(&program);
    let explicit_legacy =
        engine::admitted_resolved_functions_with_profile(&program, SourceProfile::Legacy);
    assert_eq!(
        legacy.keys().collect::<Vec<_>>(),
        explicit_legacy.keys().collect::<Vec<_>>()
    );
    assert!(!legacy.contains_key("helper"));
    assert!(engine::scan_closure("entry", &legacy, &program.declarations).is_err());
    let strings =
        engine::admitted_resolved_functions_with_profile(&program, SourceProfile::InternalStrings);
    assert!(strings.contains_key("helper"));
    engine::scan_closure("entry", &strings, &program.declarations).unwrap();
    let mut helper = (*strings["helper"]).clone();
    assert_eq!(helper.params[0].ownership, hir::OwnershipMode::Own);
    assert!(signature_is_admitted(&helper, &program.declarations));
    for mode in [hir::OwnershipMode::Value, hir::OwnershipMode::Borrow] {
        helper.params[0].ownership = mode;
        assert!(!signature_is_admitted(&helper, &program.declarations));
    }
    // The effectful/Project signature gate remains closed, independently of
    // the source-profile function-map wrapper.
    assert!(!engine::resolved_data_signature_is_admitted(
        strings["helper"],
        &program.declarations
    ));
    assert!(engine::evaluate_resolved_zero_arg_i64(&program, "entry", 100).is_err());
}

fn render_fixture(profile: SourceProfile) -> String {
    let program = resolved();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "scalar")
        .unwrap();
    let revision = "revision";
    let digest = engine::source_digest(SOURCE);
    let facts = RenderFacts {
        path_text: "profile.spx",
        revision,
        digest: &digest,
        function,
        arguments_json: &[],
        max_bytes: 4096,
        max_steps: 100,
        steps_used: 1,
        exhausted: false,
        outcome_json: "{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"42\"}",
    };
    let (envelope, overflowed) =
        crate::bounded_output::with_limit(4096, || engine::render_with_profile(&facts, profile));
    assert!(!overflowed);
    // The separate work budget must accommodate the exact final carrier.
    let (again, overflowed) = crate::bounded_output::with_limit(envelope.len() * 3, || {
        engine::render_with_profile(&facts, profile)
    });
    assert!(!overflowed);
    assert_eq!(again, envelope);
    envelope
}

#[test]
fn schemas_are_separate_but_scalar_facts_remain_identical() {
    let legacy = render_fixture(SourceProfile::Legacy);
    let strings = render_fixture(SourceProfile::InternalStrings);
    engine::verify_envelope(&legacy).unwrap();
    verify_envelope(&strings).unwrap();
    assert!(engine::verify_envelope(&strings).is_err());
    assert!(verify_envelope(&legacy).is_err());
    let legacy: serde_json::Value = serde_json::from_str(&legacy).unwrap();
    let strings: serde_json::Value = serde_json::from_str(&strings).unwrap();
    for key in [
        "source",
        "function",
        "arguments",
        "limits",
        "fuel",
        "outcome",
        "nonclaims",
    ] {
        assert_eq!(legacy["payload"][key], strings["payload"][key]);
    }
}

#[test]
fn canonical_guard_rejects_duplicate_reordered_and_alternatively_escaped_keys() {
    let envelope = render_fixture(SourceProfile::InternalStrings);
    for hostile in [
        envelope.replacen("\"schema\":", "\"schema\":\"ignored\",\"schema\":", 1),
        envelope.replacen("\"schema\"", "\"\\u0073chema\"", 1),
        envelope.replacen("{", "{ ", 1),
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&envelope).unwrap())
            .unwrap(),
    ] {
        assert!(verify_envelope(&hostile).is_err());
    }
}

#[test]
fn forged_options_fail_before_source_access_and_large_envelopes_before_parse() {
    let errors = interpret(
        Path::new("does-not-exist.spx"),
        "entry",
        &[],
        &InterpreterOptions {
            max_bytes: 0,
            max_steps: 1,
        },
    )
    .err()
    .unwrap();
    assert_eq!(errors[0].code, "SPX-F101");
    let error = verify_envelope(&"x".repeat(MAX_ENVELOPE_BYTES + 1)).unwrap_err();
    assert_eq!(error.code, "SPX-F106");
}
