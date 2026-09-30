use super::*;
use crate::reference_service::test_support::TempDir;
use semaprax::project::with_authenticated_project;
use semaprax_native_rust_interop_platform as platform;
use std::ffi::OsStr;

const DEPLOYMENT: &str = "reference-service-test-v1";

struct Fixture {
    _state: TempDir,
    outbound: TempDir,
    _secrets: TempDir,
    host: BoundHost<'static, 'static>,
    committed: CommittedState,
}

// The revision, directories, and secrets outlive the test body through
// intentional leaks: this keeps the fixture's lifetimes simple without
// changing any production signature for tests.
fn fixture() -> Fixture {
    fixture_with_telemetry_origin("https://127.0.0.1:9")
}

fn fixture_with_telemetry_origin(telemetry_origin: &str) -> Fixture {
    // The project loader rejects `.`/`..` components, so the fixture
    // path is canonicalized before loading.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("task-service-project")
        .join("semaprax.toml")
        .canonicalize()
        .expect("canonicalize task-service-project fixture");
    let revision: &'static semaprax::project::ProjectRevision = Box::leak(Box::new(
        with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .expect("load task-service-project fixture"),
    ));
    let (state_dir, state_held) = TempDir::hold("reference-mapping-state");
    let (outbound_dir, outbound_held) = TempDir::hold("reference-mapping-outbound");
    let (secrets_dir, secrets_held) = TempDir::hold("reference-mapping-secrets");
    let state_held: &'static HeldDirectory = Box::leak(Box::new(state_held));
    let outbound_held: &'static HeldDirectory = Box::leak(Box::new(outbound_held));
    write_secret(&secrets_held, "auth.pepper", &[1_u8; 32]);
    write_secret(&secrets_held, "auth.session", &[2_u8; 32]);
    write_secret(&secrets_held, "webhook.signing", &[3_u8; 32]);
    let intent = decode_host_intent(telemetry_origin);
    let secrets = super::super::secrets::resolve(&secrets_held, intent.secrets().unwrap()).unwrap();
    let decisions =
        DecisionEngine::bind(revision, super::super::decisions::DECISION_MAX_STEPS).unwrap();
    let grants = HostGrants::from_trusted_host(
        state_held,
        outbound_held,
        secrets,
        DEPLOYMENT.to_owned(),
        OutboundCheckpointSyncMode::FileOnly,
        DEFAULT_SESSION_IDLE_SECONDS,
        DEFAULT_SESSION_ABSOLUTE_SECONDS,
    )
    .unwrap();
    let (host, committed) = bind(&intent, decisions, grants, InitialState::Genesis).unwrap();
    Fixture {
        _state: state_dir,
        outbound: outbound_dir,
        _secrets: secrets_dir,
        host,
        committed,
    }
}

fn write_secret(directory: &HeldDirectory, name: &str, bytes: &[u8]) {
    let _ = platform::write_file_new(directory, OsStr::new(name), bytes, 0o600).unwrap();
}

fn decode_host_intent(telemetry_origin: &str) -> ServiceHostAdapterRequestV1 {
    let text = r#"{"capabilities":["semaprax.service.http.serve-tls.v1","semaprax.service.secrets.resolve.v1","semaprax.service.telemetry.emit.v1"],"database":{"adapter":"snapshot","migration_table":"semaprax_migrations"},"http":{"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},"mode":"host","schema":"semaprax.service-host-adapter-request.v1","secrets":{"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},"telemetry":{"adapter":"semaprax-json-events","endpoint_origin":"https://127.0.0.1:9"}}"#;
    let text = text.replace("https://127.0.0.1:9", telemetry_origin);
    let mut bytes = text.into_bytes();
    bytes.push(b'\n');
    semaprax::project::service_host_adapter_request::decode(&bytes).unwrap()
}

fn exchange(method: &str, target: &str, body: &str, token: Option<&str>) -> HttpExchange {
    let mut headers = vec![("content-length".to_owned(), body.len().to_string())];
    if let Some(token) = token {
        headers.push(("authorization".to_owned(), format!("Bearer {token}")));
    }
    HttpExchange {
        method: method.to_owned(),
        target: target.to_owned(),
        headers,
        body: body.as_bytes().to_vec(),
    }
}

fn field(body: &str, key: &str) -> JsonValue {
    json::parse(body.as_bytes(), 64 * 1024)
        .unwrap()
        .get(key)
        .unwrap()
        .clone()
}

#[test]
fn register_login_crud_logout_round_trip() {
    // The fixture stays whole: destructuring it would drop the
    // directory guards and delete the held directories mid-test.
    let mut fixture = fixture();
    let health = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/health", "", None),
    );
    assert_eq!(health.status, 200);

    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"alice","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let account_id = field(&registered.body, "account_id").as_i64().unwrap();
    assert_eq!(account_id, 1);

    let duplicate = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"alice","password":"another secret 8"}"#,
            None,
        ),
    );
    assert_eq!(duplicate.status, 409);

    let bad_name = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"1alice","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(bad_name.status, 400);

    let denied = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"alice","password":"wrong password 0"}"#,
            None,
        ),
    );
    assert_eq!(denied.status, 401);

    let logged_in = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"alice","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(logged_in.status, 200, "{}", logged_in.body);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();

    let created = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/tasks",
            r#"{"title":"write the report"}"#,
            Some(&token),
        ),
    );
    assert_eq!(created.status, 201, "{}", created.body);

    let fetched = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(fetched.status, 200);
    assert_eq!(
        field(&fetched.body, "title").as_str().unwrap(),
        "write the report"
    );

    let updated = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("PATCH", "/v1/tasks/1", r#"{"status":"done"}"#, Some(&token)),
    );
    assert_eq!(updated.status, 200);

    let deleted = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("DELETE", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(deleted.status, 200);
    let gone = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(gone.status, 404);

    let logged_out = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/logout", "", Some(&token)),
    );
    assert_eq!(logged_out.status, 200);
    assert_eq!(
        fixture
            .committed
            .state
            .session_by_id(&token[..SESSION_ID_BYTES * 2])
            .unwrap()
            .state,
        5,
        "logout persists the checked terminal state"
    );
    let retired = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(retired.status, 401);
}

#[test]
fn source_selected_expiry_is_persisted_and_sticky() {
    let mut fixture = fixture();
    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"expiring","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let logged_in = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"expiring","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(logged_in.status, 200, "{}", logged_in.body);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();
    let session_id = &token[..SESSION_ID_BYTES * 2];
    let expired = fixture
        .committed
        .state
        .sessions
        .iter_mut()
        .find(|session| session.id == session_id)
        .unwrap();
    expired.idle_deadline_tick = 0;
    expired.absolute_deadline_tick = i64::MAX;

    let first = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(first.status, 401, "{}", first.body);
    assert_eq!(
        fixture
            .committed
            .state
            .session_by_id(session_id)
            .unwrap()
            .state,
        3,
        "the checked access transition selects idle_expired"
    );
    let sequence_after_expiry = fixture.committed.state.seq;
    let second = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(second.status, 401, "{}", second.body);
    assert_eq!(fixture.committed.state.seq, sequence_after_expiry);
    assert_eq!(
        fixture
            .committed
            .state
            .session_by_id(session_id)
            .unwrap()
            .state,
        3
    );
}

#[test]
fn source_selected_absolute_expiry_is_persisted_and_sticky() {
    let mut fixture = fixture();
    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"absolute","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let logged_in = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"absolute","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(logged_in.status, 200, "{}", logged_in.body);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();
    let session_id = &token[..SESSION_ID_BYTES * 2];
    let expired = fixture
        .committed
        .state
        .sessions
        .iter_mut()
        .find(|session| session.id == session_id)
        .unwrap();
    expired.idle_deadline_tick = i64::MAX;
    expired.absolute_deadline_tick = 0;

    let source_state = fixture
        .host
        .decisions
        .session_next_state_on_access(0, 1, i64::MAX as u64, 0)
        .unwrap();
    assert_eq!(source_state, 4, "checked source prioritizes absolute expiry");
    assert!(!fixture
        .host
        .decisions
        .session_is_usable(0, 1, i64::MAX as u64, 0)
        .unwrap());

    let first = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(first.status, 401, "{}", first.body);
    assert_eq!(
        fixture
            .committed
            .state
            .session_by_id(session_id)
            .unwrap()
            .state,
        source_state as u8,
        "the host persists the source-selected absolute-expired state"
    );
    let sequence_after_expiry = fixture.committed.state.seq;
    let second = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some(&token)),
    );
    assert_eq!(second.status, 401, "{}", second.body);
    assert_eq!(fixture.committed.state.seq, sequence_after_expiry);
    assert_eq!(
        fixture
            .committed
            .state
            .session_by_id(session_id)
            .unwrap()
            .state,
        4
    );
}

#[test]
fn fixture_intent_and_bad_deployment_refuse_binding() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("task-service-project")
        .join("semaprax.toml")
        .canonicalize()
        .expect("canonicalize task-service-project fixture");
    let revision = with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
        .expect("load task-service-project fixture");
    let decisions =
        DecisionEngine::bind(&revision, super::super::decisions::DECISION_MAX_STEPS).unwrap();
    let (_temp, directory) = TempDir::hold("reference-mapping-bind");
    let (_secrets_temp, secrets_dir) = TempDir::hold("reference-mapping-bind-secrets");
    write_secret(&secrets_dir, "auth.pepper", &[1_u8; 32]);
    write_secret(&secrets_dir, "auth.session", &[2_u8; 32]);
    write_secret(&secrets_dir, "webhook.signing", &[3_u8; 32]);
    let host_intent = decode_host_intent("https://127.0.0.1:9");
    let secrets =
        super::super::secrets::resolve(&secrets_dir, host_intent.secrets().unwrap()).unwrap();
    // Full grants plus fixture-mode intent still refuse: configuration
    // intent never mints a runner.
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("task-service-project")
        .join("service-host-adapter-request.json");
    let fixture_bytes = std::fs::read(&fixture_path).unwrap();
    let fixture_intent =
        semaprax::project::service_host_adapter_request::decode(&fixture_bytes).unwrap();
    let grants = HostGrants::from_trusted_host(
        &directory,
        &directory,
        secrets,
        DEPLOYMENT.to_owned(),
        OutboundCheckpointSyncMode::FileOnly,
        DEFAULT_SESSION_IDLE_SECONDS,
        DEFAULT_SESSION_ABSOLUTE_SECONDS,
    )
    .unwrap();
    // `grants` is moved by the first bind; rebuild the secrets side for
    // the second attempt below.
    let secrets =
        super::super::secrets::resolve(&secrets_dir, host_intent.secrets().unwrap()).unwrap();
    assert_eq!(
        bind(&fixture_intent, decisions, grants, InitialState::Genesis)
            .err()
            .unwrap(),
        BindRefusal::FixtureMode
    );
    // A deployment binding outside the outbound identity grammar is not
    // a grant at all.
    assert_eq!(
        HostGrants::from_trusted_host(
            &directory,
            &directory,
            secrets,
            "not a valid identity!".to_owned(),
            OutboundCheckpointSyncMode::FileOnly,
            DEFAULT_SESSION_IDLE_SECONDS,
            DEFAULT_SESSION_ABSOLUTE_SECONDS,
        )
        .err()
        .unwrap(),
        BindRefusal::InvalidDeployment
    );
}

#[test]
fn job_enqueue_is_idempotent_and_completion_settles_once() {
    // The fixture stays whole: destructuring it would drop the
    // directory guards and delete the held directories mid-test.
    let mut fixture = fixture();
    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"bob","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let logged_in = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"bob","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(logged_in.status, 200);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();

    let enqueued = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/jobs/enqueue",
            r#"{"key":"job-1","desc":"task-1"}"#,
            Some(&token),
        ),
    );
    assert_eq!(enqueued.status, 200, "{}", enqueued.body);
    assert_eq!(
        field(&enqueued.body, "outcome").as_str().unwrap(),
        "created"
    );

    let duplicate = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/jobs/enqueue",
            r#"{"key":"job-1","desc":"task-1"}"#,
            Some(&token),
        ),
    );
    assert_eq!(duplicate.status, 200);
    assert_eq!(
        field(&duplicate.body, "outcome").as_str().unwrap(),
        "duplicate"
    );
    assert_eq!(
        field(&duplicate.body, "state").as_str(),
        field(&enqueued.body, "state").as_str()
    );

    let conflict = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/jobs/enqueue",
            r#"{"key":"job-1","desc":"other"}"#,
            Some(&token),
        ),
    );
    assert_eq!(conflict.status, 409);

    // No peer listens on 127.0.0.1:9, so the durable attempt fails
    // closed and settles `Uncertain` without redispatch; the job still
    // completes exactly once in state.
    let completed = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
    );
    assert_eq!(completed.status, 200, "{}", completed.body);
    assert_eq!(
        field(&completed.body, "webhook").as_str().unwrap(),
        "uncertain"
    );

    let again = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
    );
    assert_eq!(again.status, 409);

    let queried = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/jobs/1", "", Some(&token)),
    );
    assert_eq!(queried.status, 200);
    assert_eq!(field(&queried.body, "state").as_str().unwrap(), "completed");
}

#[test]
fn completion_export_refusal_keeps_job_pending_without_outbound_attempt() {
    // This is a valid collector origin for the host adapter, but its complete
    // target identifier exceeds the source policy's fixed byte bound.
    let telemetry_origin = format!(
        "https://{}.{}.{}.invalid",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
    );
    let mut fixture = fixture_with_telemetry_origin(&telemetry_origin);
    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"carol","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let logged_in = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"carol","password":"correct horse 7"}"#,
            None,
        ),
    );
    assert_eq!(logged_in.status, 200, "{}", logged_in.body);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();
    let enqueued = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/jobs/enqueue",
            r#"{"key":"job-1","desc":"task-1"}"#,
            Some(&token),
        ),
    );
    assert_eq!(enqueued.status, 200, "{}", enqueued.body);
    let digest_before = fixture.committed.digest.clone();

    let refused = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
    );
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(field(&refused.body, "error").as_str(), Some("export_not_admitted"));
    assert_eq!(field(&refused.body, "state").as_str(), Some(digest_before.as_str()));
    assert!(std::fs::read_dir(fixture.outbound.path())
        .unwrap()
        .next()
        .is_none());

    let queried = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/jobs/1", "", Some(&token)),
    );
    assert_eq!(queried.status, 200, "{}", queried.body);
    assert_eq!(field(&queried.body, "state").as_str(), Some("pending"));
}

/// A throwaway project (nothing else depends on it, so nothing else's
/// `command_succeeded()` shifts if a case below returns non-zero) that
/// depends only on `std.jobs` and calls
/// `std.jobs.idempotency.enqueue_outcome` directly. Each `test_`-prefixed
/// function is auto-discovered as an individually executed case (see
/// `project::execution::cases`) and returns the *raw* usize outcome
/// converted to `i64`, not a pass/fail encoding, so the assertions below
/// compare the real checked standard decision's returned value against the
/// host's admitted scaffold decision for the exact same representative inputs:
/// no existing job (fresh), an equal descriptor (duplicate), a
/// conflicting descriptor, empty descriptors, and descriptor bytes at
/// the exact `u8` boundary (0x00/0xFF). This runs through the project's
/// normal (non-public-API) test-execution path -- the same one
/// `semaprax test` uses -- because the public-API seam refuses this
/// closure (`SPX-F102`, see `decisions.rs`): it reaches the
/// contract-bearing `std.bytes.byte_to_i64`. The host invokes the
/// contract-free scaffold decision, so a change in either source truth fails
/// this parity regression visibly.
#[test]
fn enqueue_outcome_source_and_host_decisions_remain_in_parity() {
    const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"enqueue-outcome-source-host-parity\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"enqueue_outcome_check.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"enqueue_outcome_check.tests\"]\n\n[exports]\nweb = [\"enqueue_outcome_check.app.probe\"]\n\n[dependencies]\nstd.jobs = \"=0.1.0\"\n";
    const APP: &str = "module enqueue_outcome_check.app;\n\n@id(\"enqueue_outcome_check.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool\n{\n    byte_len(view) < 1000000usize\n}\n\n@id(\"enqueue_outcome_check.app.main\")\nfn main() -> i64\n{\n    0\n}\n";
    const TESTS: &str = "module enqueue_outcome_check.tests;\nuse function @id(\"std.jobs.idempotency.enqueue_outcome\") from std.jobs as idempotency_enqueue_outcome;\n\n@id(\"enqueue_outcome_check.tests.to_i64\")\nfn to_i64(value: usize) -> i64\n{\n    if value == 0usize { 0 } else { if value == 1usize { 1 } else { 2 } }\n}\n\n@id(\"enqueue_outcome_check.tests.test_fresh\")\nfn test_fresh() -> i64\n{\n    let existing = [0u8; 0];\n    let candidate = [106u8, 111u8, 98u8, 45u8, 49u8];\n    to_i64(idempotency_enqueue_outcome(false, array_as_slice(existing), array_as_slice(candidate)))\n}\n\n@id(\"enqueue_outcome_check.tests.test_equal_descriptor\")\nfn test_equal_descriptor() -> i64\n{\n    let descriptor = [106u8, 111u8, 98u8, 45u8, 49u8];\n    to_i64(idempotency_enqueue_outcome(true, array_as_slice(descriptor), array_as_slice(descriptor)))\n}\n\n@id(\"enqueue_outcome_check.tests.test_different_descriptor\")\nfn test_different_descriptor() -> i64\n{\n    let existing = [106u8, 111u8, 98u8, 45u8, 49u8];\n    let candidate = [106u8, 111u8, 98u8, 45u8, 50u8];\n    to_i64(idempotency_enqueue_outcome(true, array_as_slice(existing), array_as_slice(candidate)))\n}\n\n@id(\"enqueue_outcome_check.tests.test_empty_descriptors\")\nfn test_empty_descriptors() -> i64\n{\n    let empty = [0u8; 0];\n    to_i64(idempotency_enqueue_outcome(true, array_as_slice(empty), array_as_slice(empty)))\n}\n\n@id(\"enqueue_outcome_check.tests.test_boundary_bytes\")\nfn test_boundary_bytes() -> i64\n{\n    let boundary = [0u8, 255u8];\n    to_i64(idempotency_enqueue_outcome(true, array_as_slice(boundary), array_as_slice(boundary)))\n}\n\n@id(\"enqueue_outcome_check.tests.main\")\nfn main() -> i64\n{\n    0\n}\n";

    let (temp, _held) = TempDir::hold("enqueue-outcome-source-host-parity");
    std::fs::create_dir_all(temp.join("src")).unwrap();
    std::fs::write(temp.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(temp.join("src/app.spx"), APP).unwrap();
    std::fs::write(temp.join("src/tests.spx"), TESTS).unwrap();

    let revision = with_authenticated_project(&temp.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .expect("load the throwaway enqueue-outcome-source-host-parity project");
    let execution = revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .expect("execute the throwaway project's test module");

    let raw = |name: &str| -> i64 {
        let case = execution
            .cases()
            .iter()
            .find(|case| case.name() == name)
            .unwrap_or_else(|| panic!("missing test case `{name}`"));
        match case.outcome() {
            semaprax::project::ProjectExecutionOutcome::Returned(value) => *value,
            other => panic!("case `{name}` did not return a value: {other:?}"),
        }
    };
    let fixture = fixture();

    assert_eq!(
        raw("test_fresh"),
        fixture
            .host
            .decisions
            .enqueue_outcome(false, b"", b"job-1")
            .unwrap() as i64
    );
    assert_eq!(
        raw("test_equal_descriptor"),
        fixture
            .host
            .decisions
            .enqueue_outcome(true, b"job-1", b"job-1")
            .unwrap() as i64
    );
    assert_eq!(
        raw("test_different_descriptor"),
        fixture
            .host
            .decisions
            .enqueue_outcome(true, b"job-1", b"job-2")
            .unwrap() as i64
    );
    assert_eq!(
        raw("test_empty_descriptors"),
        fixture
            .host
            .decisions
            .enqueue_outcome(true, b"", b"")
            .unwrap() as i64
    );
    assert_eq!(
        raw("test_boundary_bytes"),
        fixture
            .host
            .decisions
            .enqueue_outcome(true, &[0u8, 255u8], &[0u8, 255u8])
            .unwrap() as i64
    );
}
