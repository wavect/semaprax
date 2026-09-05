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
