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
    let big = write_temp(PARITY_FIXTURE);
    let (code, _, err) = cli(&[
        "interpret",
        big.to_str().unwrap(),
        "--function",
        "case.mutate.chain",
        "--max-bytes",
        "1024",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-F104"), "{err}");
    cleanup(&big);
}
