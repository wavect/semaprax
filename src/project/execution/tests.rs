use super::*;

const PROJECT_REVISION: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const WORKSPACE_REVISION: &str =
    "sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn canonical_return(max_bytes: usize) -> String {
    render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        "calculator",
        ProjectExecutionRole::Entry,
        "calculator.app",
        "calculator.app.main",
        7,
        100,
        max_bytes,
        &ProjectExecutionOutcome::Returned(42),
    )
    .unwrap()
}

fn rendered_return(value: i64) -> serde_json::Value {
    let envelope = render(
        PROJECT_SCHEMA,
        "sha256:project",
        "sha256:workspace",
        "calculator",
        ProjectExecutionRole::Entry,
        "calculator.app",
        "calculator.app.main",
        7,
        100,
        65_536,
        &ProjectExecutionOutcome::Returned(value),
    )
    .unwrap();
    let marker = ",\"payload_digest\":";
    let offset = envelope.rfind(marker).unwrap();
    let payload = format!("{}}}", &envelope[..offset]);
    let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(
        parsed["payload_digest"],
        domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())
    );
    parsed
}

#[test]
fn returned_i64_extremes_are_lossless_decimal_strings() {
    assert_eq!(
        rendered_return(i64::MIN)["outcome"]["value"],
        i64::MIN.to_string()
    );
    assert_eq!(
        rendered_return(i64::MAX)["outcome"]["value"],
        i64::MAX.to_string()
    );
}

#[test]
fn rendering_is_fail_closed_when_the_bound_cannot_hold_the_envelope() {
    let outcome = ProjectExecutionOutcome::Returned(42);
    assert_eq!(
        render(
            PROJECT_SCHEMA,
            "sha256:project",
            "sha256:workspace",
            "calculator",
            ProjectExecutionRole::Entry,
            "calculator.app",
            "calculator.app.main",
            7,
            100,
            1,
            &outcome,
        )
        .unwrap_err()[0]
            .code,
        "SPX-F104"
    );
}

#[test]
fn complete_envelope_is_a_frozen_kat_and_independently_verifies() {
    let envelope = canonical_return(65_536);
    let expected = "{\"schema\":\"semaprax.project-execution.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":\"calculator\",\"project_revision\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"workspace_revision\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",\"role\":\"entry\",\"module\":\"calculator.app\",\"stable_id\":\"calculator.app.main\",\"limits\":{\"max_bytes\":65536,\"max_steps\":100},\"fuel\":{\"steps_used\":7,\"max_steps\":100},\"outcome\":{\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"42\"},\"nonclaims\":[\"in_process_reference_interpreter_only\",\"no_target_execution\",\"no_filesystem_process_or_backend_authority\",\"no_test_discovery\",\"no_cache_or_persistence\"],\"payload_digest\":\"sha256:b47dba4ff0d97550ee68f7879b0bcbf810d9e2ea60c50ac35f0f283a56d7ef61\"}";
    assert_eq!(envelope, expected);
    verify_execution_envelope(&envelope).unwrap();
}

#[test]
fn verifier_rejects_noncanonical_confused_and_mutated_envelopes() {
    let envelope = canonical_return(65_536);
    let mutations = [
            format!(" {envelope}"),
            format!("{envelope}\n"),
            envelope.replacen(
                "{\"schema\":\"semaprax.project-execution.v1\",\"project_schema\":\"semaprax.project.v1\"",
                "{\"project_schema\":\"semaprax.project.v1\",\"schema\":\"semaprax.project-execution.v1\"",
                1,
            ),
            envelope.replacen(
                "{\"schema\":",
                "{\"unknown\":false,\"schema\":",
                1,
            ),
            envelope.replacen(
                "{\"schema\":",
                "{\"schema\":\"semaprax.project-execution.v1\",\"schema\":",
                1,
            ),
            envelope.replacen("\"role\":\"entry\"", "\"role\":\"test\"", 1),
            envelope.replacen(
                "semaprax.project-execution.v1",
                "semaprax.project.v1",
                1,
            ),
            envelope.replacen("\"steps_used\":7", "\"steps_used\":101", 1),
            envelope.replacen(
                "\"no_target_execution\"",
                "\"target_execution\"",
                1,
            ),
        ];
    for mutation in mutations {
        assert!(
            verify_execution_envelope(&mutation).is_err(),
            "mutation unexpectedly verified: {mutation}"
        );
    }
}

#[test]
fn verifier_reconstructs_the_closed_status_table() {
    let status = runtime_status::normalize_arithmetic(StatusCase::DivisionByZero);
    let envelope = render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        "calculator",
        ProjectExecutionRole::Entry,
        "calculator.app",
        "calculator.app.main",
        9,
        100,
        65_536,
        &ProjectExecutionOutcome::LanguageFailure(status),
    )
    .unwrap();
    verify_execution_envelope(&envelope).unwrap();
    assert!(verify_execution_envelope(&envelope.replacen("\"code\":4", "\"code\":9", 1)).is_err());
    assert!(verify_execution_envelope(&envelope.replacen(
        "\"class\":\"arithmetic\"",
        "\"class\":\"contract\"",
        1
    ))
    .is_err());

    let external = NormalizedStatus::try_new(
        "host.failure.v1",
        7,
        crate::conformance::StatusClass::Import,
        crate::conformance::Retryability::Known(false),
    )
    .unwrap();
    let confused = render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        "calculator",
        ProjectExecutionRole::Entry,
        "calculator.app",
        "calculator.app.main",
        9,
        100,
        65_536,
        &ProjectExecutionOutcome::LanguageFailure(external),
    )
    .unwrap();
    assert!(verify_execution_envelope(&confused).is_err());
}

#[test]
fn verifier_rejects_self_consistent_but_impossible_semantic_facts() {
    let premature_exhaustion = render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        "calculator",
        ProjectExecutionRole::Entry,
        "calculator.app",
        "calculator.app.main",
        99,
        100,
        65_536,
        &ProjectExecutionOutcome::FuelExhausted,
    )
    .unwrap();
    assert!(verify_execution_envelope(&premature_exhaustion).is_err());

    let invalid_project = render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        "Calculator",
        ProjectExecutionRole::Entry,
        "calculator..app",
        "calculator.app.main",
        7,
        100,
        65_536,
        &ProjectExecutionOutcome::Returned(42),
    )
    .unwrap();
    assert!(verify_execution_envelope(&invalid_project).is_err());

    let oversized = " ".repeat(graph::MAX_AGENT_CONTEXT_BYTES + 1);
    assert!(verify_execution_envelope(&oversized).is_err());
}

#[test]
fn rendering_and_verification_honor_the_exact_max_bytes_boundary() {
    let project = "p".repeat(MAX_NAME_BYTES);
    let module = "m".repeat(MAX_MODULE_BYTES);
    let stable_id = "s".repeat(MAX_STABLE_ID_BYTES);
    let outcome = ProjectExecutionOutcome::Returned(i64::MIN);
    let mut low = graph::MIN_AGENT_CONTEXT_BYTES;
    let mut high = graph::MAX_AGENT_CONTEXT_BYTES;
    while low < high {
        let midpoint = low + (high - low) / 2;
        if render(
            PROJECT_SCHEMA,
            PROJECT_REVISION,
            WORKSPACE_REVISION,
            &project,
            ProjectExecutionRole::Entry,
            &module,
            &stable_id,
            100,
            100,
            midpoint,
            &outcome,
        )
        .is_ok()
        {
            high = midpoint;
        } else {
            low = midpoint + 1;
        }
    }
    let exact_limit = low;
    let exact = render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        &project,
        ProjectExecutionRole::Entry,
        &module,
        &stable_id,
        100,
        100,
        exact_limit,
        &outcome,
    )
    .unwrap();
    verify_execution_envelope(&exact).unwrap();
    assert!(render(
        PROJECT_SCHEMA,
        PROJECT_REVISION,
        WORKSPACE_REVISION,
        &project,
        ProjectExecutionRole::Entry,
        &module,
        &stable_id,
        100,
        100,
        exact_limit - 1,
        &outcome,
    )
    .is_err());
}
