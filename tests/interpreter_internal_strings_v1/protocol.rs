use super::*;

#[test]
fn canonical_replay_rejects_domains_duplicates_reordering_and_resigned_contradictions() {
    let mut fixture = Fixture::new(SOURCE);
    let options = InterpreterOptions::default();
    let good = run(&fixture, "case.content", &[], &options).envelope;
    let raw = payload(&good);
    let parsed: serde_json::Value = serde_json::from_str(&good).unwrap();
    let steps = parsed["payload"]["fuel"]["steps_used"].as_u64().unwrap();
    let source_digest = parsed["payload"]["source"]["sha256"].as_str().unwrap();
    let wrong_digest = format!("sha256:{}", "A".repeat(64));
    let contradictions = [
        raw.replacen(
            &format!("\"schema\":\"{SCHEMA}\""),
            &format!("\"schema\":\"{SCHEMA}\",\"schema\":\"{SCHEMA}\""),
            1,
        ),
        raw.replacen("\"returned\"", "\"unknown\"", 1),
        raw.replacen("\"value\":\"42\"", "\"value\":\"042\"", 1),
        raw.replacen(
            &format!("\"steps_used\":{steps},\"budget\":1000000"),
            &format!("\"budget\":1000000,\"steps_used\":{steps}"),
            1,
        ),
        raw.replacen("\"schema\"", "\"\\u0073chema\"", 1),
        raw.replacen('{', "{ ", 1),
        raw.replacen("\"fuel\":{", "\"fuel\":{\"unknown\":0,", 1),
        raw.replacen(
            "\"exhausted\":false",
            "\"exhausted\":false,\"exhausted\":false",
            1,
        ),
        raw.replacen(
            &format!("\"steps_used\":{steps}"),
            "\"steps_used\":1000000",
            1,
        )
        .replacen("\"exhausted\":false", "\"exhausted\":true", 1),
        raw.replacen(source_digest, &wrong_digest, 1),
    ];
    for changed in contradictions {
        assert_ne!(changed, raw);
        let forged = remint(&changed, DOMAIN);
        assert_eq!(
            internal_strings::verify_envelope(&forged).unwrap_err().code,
            "SPX-F106"
        );
    }
    assert_eq!(
        internal_strings::verify_envelope(&remint(raw, LEGACY_DOMAIN))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    let foreign_schema = raw.replacen(SCHEMA, "semaprax.interpret.v1", 1);
    assert_eq!(
        internal_strings::verify_envelope(&remint(&foreign_schema, DOMAIN))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    let duplicate_outer = good.replacen("{\"schema\":", "{\"bytes\":0,\"schema\":", 1);
    assert_eq!(
        internal_strings::verify_envelope(&duplicate_outer)
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    assert_eq!(
        internal_strings::verify_envelope(&good.replace("\"value\":\"42\"", "\"value\":\"43\""))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    let failed = run(&fixture, "case.late", &[], &options).envelope;
    for changed in [
        payload(&failed).replacen("\"code\":4", "\"code\":99", 1),
        payload(&failed).replacen("\"class\":\"arithmetic\"", "\"class\":\"contract\"", 1),
        payload(&failed).replacen("\"retryable\":false", "\"retryable\":true", 1),
    ] {
        assert_ne!(changed, payload(&failed));
        assert_eq!(
            internal_strings::verify_envelope(&remint(&changed, DOMAIN))
                .unwrap_err()
                .code,
            "SPX-F106"
        );
    }
    let failed_value: serde_json::Value = serde_json::from_str(&failed).unwrap();
    let failed_steps = failed_value["payload"]["fuel"]["steps_used"]
        .as_u64()
        .unwrap();
    let contradictory_failure = payload(&failed)
        .replacen(
            &format!("\"steps_used\":{failed_steps}"),
            "\"steps_used\":1000000",
            1,
        )
        .replacen("\"exhausted\":false", "\"exhausted\":true", 1);
    assert_eq!(
        internal_strings::verify_envelope(&remint(&contradictory_failure, DOMAIN))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    fixture.write(
        "source.spx",
        format!("{SOURCE}\n// changed exact source bytes\n"),
    );
    assert_eq!(
        internal_strings::verify_envelope_against_source(&good, &fixture.source)
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    fixture.cleanup();
}

#[test]
fn internal_strings_do_not_admit_effects_imports_generics_or_unsafe_boundaries() {
    for (source, reason) in [
        (
            r#"module guard.effects;
permit { process.stdout.write }
@id("app.main") fn main() -> i64 uses { process.stdout.write } {
    let bytes = [65u8]; let view = array_as_slice(bytes); let written = stdout_write(view); 0
}"#,
            "declared_effects",
        ),
        (
            r#"module guard.imports;
@id("rust.host") interface RustHost permits {} {
    @id("rust.combine") import rust fn combine(value: i64) -> i64 effects {} failure infallible;
}

@id("app.main") fn main() -> i64 { combine(1) }
"#,
            "import_call",
        ),
        (
            r#"module guard.generics;
@id("helper.identity") fn identity<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { identity<i64>(42) }
"#,
            "generic_call",
        ),
        (
            r#"module guard.unsafe_boundary;
permit { unsafe }
@id("app.main") fn main() -> i64 {
    @audit("negative admission fixture; arithmetic only") unsafe { 0 }
    0
}"#,
            "unsafe_boundary",
        ),
    ] {
        semaprax::check(source, "guard.spx").unwrap();
        let fixture = Fixture::new(source);
        let errors = internal_strings::interpret(
            &fixture.source,
            "app.main",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.code == "SPX-F102" && error.message.contains(reason)),
            "{reason}: {errors:?}"
        );
        fixture.cleanup();
    }
}

#[test]
fn internal_string_profile_preserves_the_verifiers_scalar_loop_call_boundary() {
    let source = r#"module guard.string_loop;
@id("helper.identity") fn identity(value: string) -> string { value }
@id("app.main") fn main() -> i64 {
    let mut count = 0;
    while count < 4 {
        let text = identity("loop");
        count = count + string_len(text);
        count
    }
    count
}"#;
    let fixture = Fixture::new(source);
    for interpret in [interpreter::interpret, internal_strings::interpret] {
        let errors = interpret(
            &fixture.source,
            "app.main",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-T252"),
            "{errors:?}"
        );
    }
    fixture.cleanup();
}

#[test]
fn internal_call_fuel_and_depth_are_source_profile_capacity_outcomes() {
    let fixture = Fixture::new(SOURCE);
    for budget in [1, 4, 16] {
        let options = InterpreterOptions::new(65536, budget).unwrap();
        let result = run(&fixture, "case.content", &[], &options);
        assert!(!result.returned);
        let value: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(value["payload"]["outcome"]["kind"], "fuel_exhausted");
        assert_eq!(value["payload"]["fuel"]["steps_used"], budget);
        assert_eq!(value["payload"]["fuel"]["budget"], budget);
        assert_eq!(value["payload"]["fuel"]["exhausted"], true);
        internal_strings::verify_envelope(&result.envelope).unwrap();
    }
    assert!(
        run(
            &fixture,
            "case.content",
            &[],
            &InterpreterOptions::default()
        )
        .returned
    );
    for steps in [0, interpreter::MAX_STEPS_LIMIT + 1] {
        let options = InterpreterOptions {
            max_bytes: 65536,
            max_steps: steps,
        };
        let errors = internal_strings::interpret(&fixture.source, "case.content", &[], &options)
            .unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-F101"));
    }
    fixture.cleanup();

    let recursive = Fixture::new(
        r#"module strings.depth;
@id("helper.recur") fn recur(value: string) -> string { recur(value) }
@id("app.main") fn main() -> i64 { string_len(recur("x\u{0}")) }
"#,
    );
    let result = run(&recursive, "app.main", &[], &InterpreterOptions::default());
    assert!(!result.returned);
    assert_eq!(outcome(&result.envelope)["kind"], "call_depth_exceeded");
    internal_strings::verify_envelope(&result.envelope).unwrap();
    recursive.cleanup();
}

#[test]
fn exact_output_budget_and_source_envelope_caps_fail_before_unbounded_processing() {
    let fixture = Fixture::new(
        r#"module strings.budget;
@id("app.main") fn main() -> i64 { 42 }
@id("budget.read") fn read(value: borrow str) -> i64 { str_len_bytes(value) }
"#,
    );
    let arguments = vec![format!("\"{}\"", "x".repeat(2048))];
    let mut budget = 65536;
    let mut exact = None;
    for _ in 0..4 {
        let result = run(
            &fixture,
            "budget.read",
            &arguments,
            &InterpreterOptions::new(budget, 1000000).unwrap(),
        );
        if result.envelope.len() == budget {
            exact = Some(result.envelope);
            break;
        }
        budget = result.envelope.len();
    }
    let exact = exact.expect("canonical envelope length must reach a fixed point");
    internal_strings::verify_envelope(&exact).unwrap();
    let errors = internal_strings::interpret(
        &fixture.source,
        "budget.read",
        &arguments,
        &InterpreterOptions::new(budget - 1, 1000000).unwrap(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-F104"));
    let forged_budget = payload(&exact).replacen(
        &format!("\"max_bytes\":{budget}"),
        &format!("\"max_bytes\":{}", budget - 1),
        1,
    );
    assert_eq!(
        internal_strings::verify_envelope(&remint(&forged_budget, DOMAIN))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    assert_eq!(
        internal_strings::verify_envelope(&" ".repeat(CAP + 1))
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    fixture.cleanup();

    let fixture = Fixture::new("module strings.cap; @id(\"app.main\") fn main() -> i64 { 42 }\n//");
    // Sparse padding is inside a source comment. The exact-cap source remains
    // valid UTF-8/source; cap+1 must fail before a parser or evaluator can run.
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&fixture.source)
        .unwrap();
    file.set_len(CAP as u64).unwrap();
    let accepted = run(&fixture, "app.main", &[], &InterpreterOptions::default());
    assert!(accepted.returned);
    internal_strings::verify_envelope_against_source(&accepted.envelope, &fixture.source).unwrap();
    file.set_len((CAP + 1) as u64).unwrap();
    let errors = internal_strings::interpret(
        &fixture.source,
        "app.main",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-F104"));
    assert_eq!(
        internal_strings::verify_envelope_against_source(&accepted.envelope, &fixture.source)
            .unwrap_err()
            .code,
        "SPX-F106"
    );
    drop(file);
    fixture.cleanup();
}

#[test]
fn additive_cli_exit_codes_and_argument_boundary_do_not_change_legacy_cli() {
    let fixture = Fixture::new(SOURCE);
    let invoke = |command: &str, arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&fixture.root)
            .arg(command)
            .arg(&fixture.source)
            .args(arguments)
            .output()
            .unwrap()
    };
    let success = invoke("interpret-strings", &["--function", "case.content"]);
    assert_eq!(
        success.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let envelope = String::from_utf8(success.stdout).unwrap();
    internal_strings::verify_envelope(envelope.trim_end()).unwrap();
    assert!(success.stderr.is_empty());
    let failure = invoke("interpret-strings", &["--function", "case.late"]);
    assert_eq!(failure.status.code(), Some(1));
    assert_eq!(
        outcome(String::from_utf8(failure.stdout).unwrap().trim_end())["status"]["code"],
        4
    );
    let external = invoke(
        "interpret-strings",
        &["--function", "helper.identity", "--arg", "\"text\""],
    );
    assert_eq!(external.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&external.stderr).contains("SPX-F102"));
    let argument = invoke(
        "interpret-strings",
        &["--function", "case.scalar", "--arg", "\"text\""],
    );
    assert_eq!(argument.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&argument.stderr).contains("SPX-F103"));
    let missing = invoke("interpret-strings", &[]);
    assert_eq!(missing.status.code(), Some(2));
    let legacy = invoke("interpret", &["--function", "case.content"]);
    assert_eq!(legacy.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("SPX-F102"));
    fixture.cleanup();
}
