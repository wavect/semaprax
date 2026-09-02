use super::*;
use crate::interpreter::{evaluate_resolved_zero_arg_i64, ResolvedEvaluationOutcome};
use crate::{format, hir, parse};
use serde_json::json;

fn program(source: &str) -> Program {
    parse(source, "signature.spx").unwrap()
}

fn outcome(program: &Program) -> ResolvedEvaluationOutcome {
    let canonical = format::canonical(program);
    let reparsed = parse(&canonical, "signature.spx").unwrap();
    let resolved = hir::resolve(&reparsed).unwrap();
    evaluate_resolved_zero_arg_i64(&resolved, "app.main", 100_000)
        .unwrap()
        .outcome
}

fn evolve(programs: &mut [Program], parameters: Value) -> Result<super::super::IntentSummary> {
    super::super::apply(
        programs,
        &json!({
            "kind":"change_function_signature","target":"math.select","parameters":parameters
        }),
    )
}

#[test]
fn implicit_string_retention_guard_agrees_with_real_checked_parameter_ownership() {
    let source = program(
        r#"module test.signature;
@id("text.inspect") fn inspect(owned: string, view: borrow str, scalar: i64) -> i64 { scalar }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let checked = hir::resolve(&source).unwrap();
    let original = source
        .functions
        .iter()
        .find(|function| function.stable_id == "text.inspect")
        .unwrap();
    let resolved = checked
        .functions
        .iter()
        .find(|function| function.id.as_str() == "text.inspect")
        .unwrap();
    assert_eq!(original.params[0].mode, ParamMode::Value);
    assert_eq!(original.params[0].ty, Type::String);
    assert_eq!(resolved.params[0].ownership, OwnershipMode::Own);
    assert!(owning_parameter(&original.params[0]));
    assert!(!legacy_parameter(&original.params[0]));
    assert!(!owning_parameter(&original.params[1]));
    assert!(!legacy_parameter(&original.params[1]));
    assert!(!owning_parameter(&original.params[2]));
    assert!(legacy_parameter(&original.params[2]));
    let facts = checked
        .declarations
        .type_facts(&resolved.params[0].ty)
        .unwrap();
    assert!(!facts.copy);
    assert!(facts.sized && facts.needs_drop);
    assert!(!facts.contains_resource);
    assert_eq!(
        format::canonical(&source),
        format::canonical(&program(&format::canonical(&source)))
    );
}

#[test]
fn reordered_copy_parameters_preserve_value_and_stage_all_original_arguments() {
    let mut programs = vec![program(
        r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left * 10 + right }
@id("app.main") fn main() -> i64 { select(2, 3) }
"#,
    )];
    let before = outcome(&programs[0]);
    let summary = evolve(&mut programs, json!([{"from":"right"},{"from":"left"}])).unwrap();
    assert_eq!(summary.migrated_calls, 1);
    assert_eq!(before, ResolvedEvaluationOutcome::ReturnedI64(23));
    assert_eq!(outcome(&programs[0]), before);
    let canonical = format::canonical(&programs[0]);
    assert!(canonical.contains("fn select(right: i64, left: i64)"));
    assert!(canonical.contains("let spx_sig_stage_0 = 2; let spx_sig_stage_1 = 3; select(spx_sig_stage_1, spx_sig_stage_0)"));
    assert_eq!(format::canonical(&program(&canonical)), canonical);
}

#[test]
fn dropped_argument_and_reordered_arguments_preserve_first_checked_failure() {
    for parameters in [
        json!([{"from":"right"}]),
        json!([{"from":"right"},{"from":"left"}]),
    ] {
        let mut programs = vec![program(
            r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { right }
@id("app.main") fn main() -> i64 { select(1 / 0, 9223372036854775807 + 1) }
"#,
        )];
        let before = outcome(&programs[0]);
        assert!(matches!(
            before,
            ResolvedEvaluationOutcome::LanguageFailure(_)
        ));
        evolve(&mut programs, parameters).unwrap();
        assert_eq!(outcome(&programs[0]), before);
        let canonical = format::canonical(&programs[0]);
        assert!(
            canonical.find("= 1 / 0;").unwrap()
                < canonical.find("= 9223372036854775807 + 1;").unwrap()
        );
    }
}

#[test]
fn generated_staging_names_do_not_capture_existing_parameters_or_local_bindings() {
    let mut programs = vec![program(
        r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left * 10 + right }
@id("math.nested") fn nested(spx_sig_stage_0: i64) -> i64 {
    let spx_sig_stage_1 = 3;
    select(spx_sig_stage_0, spx_sig_stage_1)
}
@id("app.main") fn main() -> i64 { nested(2) }
"#,
    )];
    let before = outcome(&programs[0]);
    evolve(&mut programs, json!([{"from":"right"},{"from":"left"}])).unwrap();
    assert_eq!(outcome(&programs[0]), before);
    let canonical = format::canonical(&programs[0]);
    assert!(canonical
        .contains("let spx_sig_stage_2 = spx_sig_stage_0; let spx_sig_stage_3 = spx_sig_stage_1;"));
    assert!(canonical.contains("select(spx_sig_stage_3, spx_sig_stage_2)"));
}

#[test]
fn import_alias_and_declared_effect_calls_keep_original_staging_order() {
    let mut programs = vec![
        program(
            r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left + right }
"#,
        ),
        program(
            r#"module test.consumer;
use function @id("math.select") from test.signature as choose;
permit { clock.read }
@id("math.first") fn first() -> i64 uses { clock.read } { 2 }
@id("math.second") fn second() -> i64 uses { clock.read } { 3 }
@id("app.main") fn main() -> i64 uses { clock.read } { choose(first(), second()) }
"#,
        ),
    ];
    evolve(&mut programs, json!([{"from":"right"},{"from":"left"},{"name":"extra","type":"bool","argument":{"kind":"bool","value":true}}])).unwrap();
    let canonical = format::canonical(&programs[1]);
    assert!(canonical.contains("from test.signature as choose"));
    assert!(canonical.contains("let spx_sig_stage_0 = first(); let spx_sig_stage_1 = second(); choose(spx_sig_stage_1, spx_sig_stage_0, true)"));
    assert_eq!(programs[1].permits, ["clock.read"]);
    assert_eq!(programs[1].functions[2].effects, ["clock.read"]);
}

#[test]
fn removal_of_used_parameter_fails_real_verifier_and_type_guesses_reject() {
    let base = r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left + right }
@id("app.main") fn main() -> i64 { select(2, 3) }
"#;
    let mut used = vec![program(base)];
    evolve(&mut used, json!([{"from":"right"}])).unwrap();
    assert!(hir::resolve(&program(&format::canonical(&used[0]))).is_err());
    for parameters in [
        json!([{"from":"left","type":"bool"},{"from":"right"}]),
        json!([{"name":"left","type":"bool","argument":{"kind":"bool","value":true}}]),
        json!([{"from":"left"},{"from":"left"}]),
    ] {
        let mut programs = vec![program(base)];
        let before = format::canonical(&programs[0]);
        let errors = match evolve(&mut programs, parameters) {
            Ok(_) => panic!("unsupported signature mapping succeeded"),
            Err(errors) => errors,
        };
        assert!(errors.iter().any(|error| error.code == "SPX-G225"));
        assert_eq!(format::canonical(&programs[0]), before);
    }
}

#[test]
fn simultaneous_parameter_renames_preserve_contracts_and_avoid_local_capture() {
    let mut programs = vec![program(
        r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64
requires left >= 0
ensures result >= right
{
    let mut renamed = right;
    renamed = renamed + left;
    match renamed { local if local > 0 => left + local, _ => right, }
}
@id("app.main") fn main() -> i64 { select(2, 3) }
"#,
    )];
    let before = outcome(&programs[0]);
    evolve(
        &mut programs,
        json!([{"from":"right","name":"left"},{"from":"left","name":"local"}]),
    )
    .unwrap();
    assert_eq!(outcome(&programs[0]), before);
    let canonical = format::canonical(&programs[0]);
    assert!(canonical.contains("fn select(left: i64, local: i64)"));
    assert!(canonical.contains("requires local >= 0"));
    assert!(canonical.contains("ensures result >= left"));
    assert!(canonical.contains("renamed = renamed + local"));
    assert!(canonical.contains("spx_sig_bind_0 if spx_sig_bind_0 > 0 => local + spx_sig_bind_0"));
}

#[test]
fn local_initializer_and_assignment_follow_their_original_binding() {
    let mut programs = vec![program(
        r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 {
    let mut renamed = left + right;
    renamed = renamed + left;
    renamed
}
@id("app.main") fn main() -> i64 { select(2, 3) }
"#,
    )];
    let before = outcome(&programs[0]);
    evolve(
        &mut programs,
        json!([{"from":"left","name":"renamed"},{"from":"right"}]),
    )
    .unwrap();
    assert_eq!(outcome(&programs[0]), before);
    let canonical = format::canonical(&programs[0]);
    assert!(canonical.contains("let mut spx_sig_bind_0 = renamed + right"));
    assert!(canonical.contains("spx_sig_bind_0 = spx_sig_bind_0 + renamed"));
}

#[test]
fn renaming_to_a_removed_parameter_name_cannot_capture_a_live_reference() {
    let mut programs = vec![program(
        r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { right }
"#,
    )];
    let errors = match evolve(&mut programs, json!([{"from":"left","name":"right"}])) {
        Ok(_) => panic!("removed binding was captured"),
        Err(errors) => errors,
    };
    assert!(errors.iter().any(|error| error.code == "SPX-G259"));
}

#[test]
fn owned_and_borrowed_signature_migrations_reject_before_mutation() {
    for parameter in ["own Bytes", "borrow str", "shared Bytes", "string"] {
        let source = format!("module test.signature;\n@id(\"math.select\") fn select(value: {parameter}) -> i64 {{ 0 }}\n");
        let mut programs = vec![program(&source)];
        let before = format::canonical(&programs[0]);
        let errors = match evolve(&mut programs, json!([])) {
            Ok(_) => panic!("non-Copy signature mapping succeeded"),
            Err(errors) => errors,
        };
        let expected = if parameter == "own Bytes" {
            "SPX-G260"
        } else {
            "SPX-G225"
        };
        assert!(errors.iter().any(|error| error.code == expected));
        assert_eq!(format::canonical(&programs[0]), before);
    }
}
