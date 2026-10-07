use super::*;

#[test]
fn source_drift_after_generation_is_detected_via_the_source_binding() {
    let path = write_temp(PARITY_FIXTURE);
    let envelope = interpret_case(&path, "case.mutate.chain", &[]).expect("envelope");
    interpreter::verify_envelope_against_source(&envelope, &path)
        .expect("binding holds while bytes are unchanged");

    // The embedded source digest equals an independent domain-separated
    // computation over the exact source bytes.
    let source_bytes = std::fs::read(&path).unwrap();
    assert!(
        envelope.contains(&format!(
            "\"sha256\":\"{}\"",
            source_digest_hex(&String::from_utf8(source_bytes.clone()).unwrap())
        )),
        "{envelope}"
    );

    std::fs::write(&path, format!("{PARITY_FIXTURE}\n// drift\n")).unwrap();
    let error = interpreter::verify_envelope_against_source(&envelope, &path)
        .expect_err("drifted source must fail the binding check");
    assert_eq!(error.code, "SPX-F106");
    cleanup(&path);
}

#[test]
fn failed_status_envelopes_replay_and_pin_compiler_owned_statuses() {
    let path = write_temp(PARITY_FIXTURE);
    let envelope = interpret_case(&path, "case.add", &[]).expect("envelope");
    interpreter::verify_envelope(&envelope).expect("failed envelope verifies");
    assert!(
        envelope.contains(
            "\"outcome\":{\"kind\":\"failed\",\"status\":{\"schema\":\"semaprax.status.v1\",\
\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":1,\"class\":\"arithmetic\",\"retryable\":false}}"
        ),
        "{envelope}"
    );

    // A re-signed status code outside the closed v1 table cannot pass replay.
    let forged_code = remint_digest(&envelope.replace("\"code\":1,", "\"code\":9,"));
    let error = interpreter::verify_envelope(&forged_code)
        .expect_err("forged arithmetic codes are not in the closed v1 table");
    assert_eq!(error.code, "SPX-F106");
    cleanup(&path);
}

// ---------------------------------------------------------------------------
// CLI contracts.
// ---------------------------------------------------------------------------

#[test]
fn cli_exit_codes_follow_the_documented_contract() {
    // 0: returned value.
    let (code, out, _) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--arg",
        "19",
        "--arg",
        "23",
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("\"kind\":\"returned\""));

    // 1: language-visible failure status, envelope still emitted.
    let (code, out, _) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "add",
        "--arg",
        "-19",
        "--arg",
        "23",
    ]);
    assert_eq!(code, 1);
    assert!(out.contains("\"kind\":\"failed\""));
    assert!(
        out.contains("\"domain_id\":\"semaprax.contract.v1\"") && out.contains("\"code\":1"),
        "{out}"
    );

    // 2: usage errors.
    let (code, _, err) = cli(&["interpret"]);
    assert_eq!(code, 2);
    let _ = err;

    let (code, _, err) = cli(&["interpret", MEANING_PATH]);
    assert_eq!(code, 2);
    assert!(err.contains("--function"));

    let (code, _, err) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--bogus",
        "x",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown interpret option"));

    let (code, _, err) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--max-bytes",
        "1024",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate"));

    let (code, _, err) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--max-bytes",
        "-3",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("canonical nonnegative integer"));

    let (code, _, err) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--max-bytes",
        "512",
    ]);
    assert_eq!(code, 2);
    assert!(err.contains("SPX-F101"));

    let (code, _, err) = cli(&["interpret", MEANING_PATH, "--function", ""]);
    assert_eq!(code, 2);
    assert!(err.contains("--function"));

    let (code, _, err) = cli(&["interpret", MEANING_PATH, "--function", "no-such-function"]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-F102"), "{err}");

    let (code, _, err) = cli(&[
        "interpret",
        MEANING_PATH,
        "--function",
        "math.add",
        "--arg",
        "not-a-number",
        "--arg",
        "23",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-F103"), "{err}");

    let (code, _, _) = cli(&["interpret", "missing-file.spx", "--function", "f"]);
    assert_eq!(code, 1);

    // Byte-budget exhaustion fails closed with SPX-F104.
    let oversized_id = format!("case.{}", "x".repeat(1_800));
    let oversized_source = PARITY_FIXTURE.replacen(
        "@id(\"case.mutate.chain\")",
        &format!("@id(\"{oversized_id}\")"),
        1,
    );
    let big = write_temp(&oversized_source);
    let (code, _, err) = cli(&[
        "interpret",
        big.to_str().unwrap(),
        "--function",
        &oversized_id,
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-F104"), "{err}");
    cleanup(&big);
}

/// `run --json` advertises a machine-readable surface, so the preliminary
/// load/verify step publishes diagnostic records on stdout instead of falling
/// back to the human renderer on stderr.
#[test]
fn single_file_run_json_publishes_source_failures_as_diagnostic_records() {
    let type_error =
        write_temp("module test.run_json_type;\n@id(\"app.main\")\nfn main() -> i64 { true }\n");
    let parse_error = write_temp("module test.run_json_parse;\n@id(\"app.main\")\nfn main(\n");
    // The bounded stdout profile is a separate interpreter seam and must not
    // keep its own human-only rejection path.
    let permitted = write_temp(
        "module test.run_json_stdout;\npermit { process.stdout.write }\n@id(\"app.main\")\nfn main() -> i64 uses { process.stdout.write } { true }\n",
    );

    for (path, expected) in [
        (type_error.to_str().unwrap(), "SPX-T103"),
        (permitted.to_str().unwrap(), "SPX-T103"),
        ("missing-run-json-input.spx", "SPX-I001"),
    ] {
        let (code, stdout, stderr) = cli(&["run", path, "--json"]);
        assert_eq!(code, 1, "{path}: {stdout}{stderr}");
        assert_eq!(stderr, "", "{path}: JSON mode leaves stderr empty");
        let record: serde_json::Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|error| panic!("{path}: `{stdout}` is not a record: {error}"));
        assert_eq!(record["code"], expected, "{stdout}");
        assert_eq!(record["severity"], "error", "{stdout}");
        assert!(!stdout.contains("error["), "{stdout}");
    }

    // Parse failures carry their located record too; the code stays whatever
    // the parser already selected.
    let (code, stdout, stderr) = cli(&["run", parse_error.to_str().unwrap(), "--json"]);
    assert_eq!(code, 1);
    assert_eq!(stderr, "");
    let record: serde_json::Value = serde_json::from_str(stdout.trim()).expect("record");
    assert!(
        record["code"]
            .as_str()
            .is_some_and(|code| code.starts_with("SPX-P")),
        "{stdout}"
    );
    assert!(
        record["location"]["line"].as_u64().unwrap() >= 1,
        "{stdout}"
    );
    assert_eq!(
        record["path"],
        *parse_error.to_str().unwrap(),
        "the record binds the exact input path"
    );

    // Type and effect failures keep the located record for source files whose
    // diagnostics also verify: the same file in human mode is unchanged.
    let (code, stdout, stderr) = cli(&["run", type_error.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert_eq!(stdout, "");
    assert!(stderr.starts_with("error[SPX-T103]"), "{stderr}");

    // Capacity envelopes are untouched by the diagnostic routing.
    let runnable =
        write_temp("module test.run_json_ok;\n@id(\"app.main\")\nfn main() -> i64 { 40 + 2 }\n");
    let (code, stdout, stderr) = cli(&["run", runnable.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).expect("envelope");
    assert_eq!(envelope["schema"], "semaprax.interpret.v1");

    let (code, stdout, stderr) = cli(&[
        "run",
        runnable.to_str().unwrap(),
        "--json",
        "--max-steps",
        "1",
    ]);
    assert_eq!(code, 1);
    assert_eq!(stderr, "");
    let envelope: serde_json::Value = serde_json::from_str(stdout.trim()).expect("envelope");
    assert_eq!(envelope["schema"], "semaprax.interpret.v1");
    assert_eq!(envelope["payload"]["outcome"]["kind"], "fuel_exhausted");

    cleanup(&type_error);
    cleanup(&parse_error);
    cleanup(&permitted);
    cleanup(&runnable);
}

#[test]
fn single_file_run_falls_back_to_main_under_any_stable_id() {
    let plain =
        write_temp("module calc.entry;\n@id(\"calc.main\")\nfn main() -> i64\n{\n    42\n}\n");
    let (code, stdout, stderr) = cli(&["run", plain.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str(), stderr.as_str()), (0, "42\n", ""));
    cleanup(&plain);

    let printing = write_temp(
        "module calc.print;\npermit { process.stdout.write }\n@id(\"calc.main\")\nfn main() -> i64\n    uses { process.stdout.write }\n{\n    let text = \"hi\";\n    let view = string_as_str(text);\n    let written = stdout_write(str_as_bytes(view));\n    if written == 2usize { 0 } else { 1 }\n}\n",
    );
    let (code, stdout, _) = cli(&["run", printing.to_str().unwrap()]);
    assert_eq!((code, stdout.as_str()), (0, "hi0\n"));
    cleanup(&printing);
}

#[test]
fn interpreter_admission_refusal_points_at_the_native_route() {
    let path = write_temp(
        // Record update stays outside the bounded interpreter profile.
        "module calc.update;\n@id(\"calc.point\")\nrecord Point {\n    @id(\"calc.point.x\") x: i64,\n    @id(\"calc.point.y\") y: i64,\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    let point = Point { x: 1, y: 2, };\n    let moved = point with { y: 0, };\n    moved.x + moved.y\n}\n",
    );
    let (code, _, stderr) = cli(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stderr.contains("SPX-F102"), "{stderr}");
    assert!(
        stderr
            .contains("help: the bounded reference interpreter does not admit this program shape"),
        "{stderr}"
    );
    let (code, stdout, _) = cli(&["run", path.to_str().unwrap(), "--native"]);
    assert_eq!((code, stdout.as_str()), (0, "1\n"));
    cleanup(&path);
}

#[test]
fn sg03_default_execution_follows_main_name_after_persistent_identity_rename() {
    for helper in [
        "fn previous_main() -> i64 { 7 }",
        "fn previous_main(value: i64) -> i64 { value }",
    ] {
        let source = format!("module review.entry;\n@id(\"app.main\") {helper}\n@id(\"review.actual_main\") fn main() -> i64 {{ 42 }}\n");
        let path = write_temp(&source);
        let (code, output, error) = cli(&["run", path.to_str().unwrap(), "--json"]);
        assert_eq!(code, 0, "{error}");
        assert!(output.contains("review.actual_main"), "{output}");
        assert!(output.contains("\"value\":\"42\""), "{output}");
        if helper.contains("()") {
            let explicit = interpret_case(&path, "app.main", &[]).unwrap();
            assert!(explicit.contains("\"value\":\"7\""), "{explicit}");
        }
        cleanup(&path);
    }
}

#[test]
fn sg03_source_entry_admits_automatic_main_without_widening_explicit_interpretation() {
    let source = "module review.auto_entry;\nfn main() -> i64 { 42 }\n";
    let path = write_temp(source);
    let (code, output, error) = cli(&["run", path.to_str().unwrap(), "--json"]);
    assert_eq!(code, 0, "{error}");
    assert!(output.contains("auto:review.auto_entry.main"), "{output}");
    assert!(output.contains("\"value\":\"42\""), "{output}");
    let (code, _, error) = cli(&[
        "interpret",
        path.to_str().unwrap(),
        "--function",
        "auto:review.auto_entry.main",
    ]);
    assert_eq!(code, 1, "{error}");
    assert!(error.contains("automatic_identity"), "{error}");
    cleanup(&path);
}

#[test]
fn sg03_stdout_entry_uses_the_declared_main_when_old_identity_survives() {
    let source = "module review.stdout_entry;\npermit { process.stdout.write }\n@id(\"app.main\") fn previous_main(value: i64) -> i64 { value }\n@id(\"review.main\") fn main() -> i64 uses { process.stdout.write } { let text = \"hi\"; let view = string_as_str(text); let written = stdout_write(str_as_bytes(view)); if written == 2usize { 0 } else { 1 } }\n";
    for source in [
        source.to_owned(),
        source.replace("@id(\"review.main\") ", ""),
    ] {
        let path = write_temp(&source);
        let (code, output, error) = cli(&["run", path.to_str().unwrap()]);
        assert_eq!((code, output.as_str()), (0, "hi0\n"), "{error}");
        cleanup(&path);
    }
}

#[test]
fn sg03_source_command_entry_keeps_declared_main_and_exit_behavior() {
    let source = "module review.command_entry;\npermit { process.args.read }\n@id(\"app.main\") fn previous_main(value: i64) -> i64 { value }\n@id(\"review.main\") fn main() -> i64 uses { process.args.read } { if args_len() == 1usize { 0 } else { 3 } }\n";
    for source in [
        source.to_owned(),
        source.replace("@id(\"review.main\") ", ""),
    ] {
        let path = write_temp(&source);
        let (code, output, error) = cli(&["run", path.to_str().unwrap(), "--", "one"]);
        assert_eq!((code, output.as_str()), (0, ""), "{error}");
        cleanup(&path);
    }
}
