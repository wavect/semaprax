//! Deterministic host clock boundaries and independent checked-source parity.
use super::*;

const CREDENTIALS: &str = r#"{"username":"sessionclock","password":"correct horse 7"}"#;

fn write_canonical_source(path: std::path::PathBuf, source: &str) {
    let parsed = semaprax::parse(source, &path).expect("parse generated oracle source");
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

fn at(fixture: &mut Fixture, exchange: &HttpExchange, tick: Option<u64>) -> PendingResponse {
    let mut reads = 0;
    let response = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        exchange,
        &mut || {
            reads += 1;
            tick
        },
    );
    assert_eq!(
        reads, 1,
        "one authenticated exchange observes one host tick"
    );
    response
}

fn login_fixture() -> (Fixture, String) {
    let mut fixture = fixture();
    // These are host-owned policy values, never decoded request/config input.
    fixture.host.session_idle_seconds = 10;
    fixture.host.session_absolute_seconds = 20;
    let registration = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/register", CREDENTIALS, None),
        &mut || panic!("registration has no session-clock dependency"),
    );
    assert_eq!(registration.status, 201, "{}", registration.body);
    let login = at(
        &mut fixture,
        &exchange("POST", "/v1/login", CREDENTIALS, None),
        Some(100),
    );
    assert_eq!(login.status, 200, "{}", login.body);
    let token = field(&login.body, "token").as_str().unwrap().to_owned();
    (fixture, token)
}

#[test]
fn exact_session_deadlines_persist_and_remain_terminal_after_reload() {
    let (mut fixture, token) = login_fixture();
    let initial_state = fixture.committed.state.clone();
    let initial_digest = fixture.committed.digest.clone();
    let session_id = &token[..SESSION_ID_BYTES * 2];
    let request = exchange("GET", "/v1/tasks/1", "", Some(&token));
    // Each row starts from the same authentic login snapshot; prior terminal
    // states cannot hide the absolute-before-idle precedence at tick 120.
    for (tick, expected) in [(109, 0), (110, 3), (111, 3), (119, 3), (120, 4), (121, 4)] {
        fixture.committed = CommittedState {
            state: initial_state.clone(),
            digest: initial_digest.clone(),
        };
        let response = at(&mut fixture, &request, Some(tick));
        assert_eq!(
            response.status,
            if expected == 0 { 404 } else { 401 },
            "tick {tick}: {}",
            response.body
        );
        let session = fixture.committed.state.session_by_id(session_id).unwrap();
        assert_eq!(session.state, expected, "tick {tick}");
        assert_eq!(
            (session.idle_deadline_tick, session.absolute_deadline_tick),
            (110, 120),
            "access never refreshes fixed deadlines"
        );
        if expected == 0 {
            assert_eq!(fixture.committed.digest, initial_digest);
            continue;
        }
        assert_eq!(fixture.committed.state.seq, initial_state.seq + 1);
        let digest = fixture.committed.digest.clone();
        let bytes = fixture
            .host
            .state_store
            .load(OutboundCheckpointKind::ServiceState, &digest)
            .unwrap();
        let restored = ServiceState::decode(&bytes).unwrap();
        assert_eq!(ServiceState::digest(&restored.render()), digest);
        fixture.committed = CommittedState {
            state: restored,
            digest: digest.clone(),
        };
        for replay_tick in [tick, 0, 200] {
            let response = at(&mut fixture, &request, Some(replay_tick));
            assert_eq!(response.status, 401);
            assert_eq!(
                fixture.committed.digest, digest,
                "terminal replay must not commit again"
            );
            assert_eq!(
                fixture
                    .committed
                    .state
                    .session_by_id(session_id)
                    .unwrap()
                    .state,
                expected
            );
        }
    }
}

#[test]
fn unavailable_session_clock_refuses_without_state_changes() {
    let (mut fixture, token) = login_fixture();
    let bytes = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();
    for (request, error_name) in [
        (
            exchange("POST", "/v1/login", CREDENTIALS, None),
            "clock_unavailable",
        ),
        (
            exchange(
                "POST",
                "/v1/tasks",
                r#"{"title":"must not exist"}"#,
                Some(&token),
            ),
            "decision_failed",
        ),
    ] {
        let refused = at(&mut fixture, &request, None);
        assert_eq!(refused.status, 500);
        assert_eq!(field(&refused.body, "error").as_str(), Some(error_name));
        assert_eq!(fixture.committed.state.render(), bytes);
        assert_eq!(fixture.committed.digest, digest);
    }
    let health = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/health", "", None),
        &mut || panic!("health needs no clock"),
    );
    assert_eq!(health.status, 200);
    let denied = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/tasks/1", "", Some("invalid")),
        &mut || panic!("invalid bearer needs no clock"),
    );
    assert_eq!(denied.status, 401);
}

#[test]
fn session_transitions_match_std_auth_and_the_generated_scaffold() {
    use semaprax::project::{derive_project_scaffold_v1_with_layout, ScaffoldLayout};
    let (generated, _held) = TempDir::hold("session-policy-generated");
    let scaffold =
        derive_project_scaffold_v1_with_layout("session-parity", "service", ScaffoldLayout::Tables)
            .unwrap();
    for file in scaffold.files() {
        let path = generated.join(file.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.bytes()).unwrap();
    }
    install_generated_create_decision(generated.path());
    let generated_revision =
        with_authenticated_project(&generated.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    let generated_decisions = DecisionEngine::bind(
        &generated_revision,
        crate::reference_service::decisions::DECISION_MAX_STEPS,
    )
    .unwrap();
    assert_eq!(generated_decisions.identities().prefix(), "session_parity");
    let fixture = fixture();

    // Execute contract-bearing std.auth through normal project test execution.
    // Neither the wrapper under test nor a Rust branch supplies this oracle.
    let (oracle, _held) = TempDir::hold("session-policy-oracle");
    std::fs::create_dir_all(oracle.join("src")).unwrap();
    std::fs::write(oracle.join("semaprax.toml"), concat!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"session-oracle\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n",
        "\n[modules]\nentry = \"session_oracle.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"session_oracle.tests\"]\n",
        "\n[exports]\nweb = [\"session_oracle.app.probe\"]\n\n[dependencies]\nstd.auth = \"=0.1.0\"\n",
    )).unwrap();
    write_canonical_source(oracle.join("src/app.spx"), "module session_oracle.app;\n@id(\"session_oracle.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool { byte_len(view) < 1000000usize }\n@id(\"session_oracle.app.main\")\nfn main() -> i64 { 0 }\n");
    let mut source = String::from(concat!(
        "module session_oracle.tests;\n",
        "use function @id(\"std.auth.session.is_usable\") from std.auth as usable;\n",
        "use function @id(\"std.auth.session.next_state_on_access\") from std.auth as access;\n",
        "use function @id(\"std.auth.session.next_state_on_logout\") from std.auth as logout;\n",
        "@id(\"session_oracle.tests.code\")\nfn code(value: usize) -> i64 { if value == 0usize { 0 } else { if value == 1usize { 1 } else { if value == 2usize { 2 } else { if value == 3usize { 3 } else { if value == 4usize { 4 } else { 5 } } } } } }\n",
    ));
    let mut rows = Vec::new();
    for state in [0, 1, 2, 3, 4, 5, 6, 255] {
        for (now, idle, absolute) in [
            (0, 10, 20),
            (9, 10, 20),
            (10, 10, 20),
            (11, 10, 20),
            (19, 10, 20),
            (20, 10, 20),
            (21, 10, 20),
            (10, 10, 10),
        ] {
            let index = rows.len();
            rows.push((state, now, idle, absolute));
            source.push_str(&format!("@id(\"session_oracle.tests.test_case_{index}\")\nfn test_case_{index}() -> i64 {{ let active = if usable({state}usize, {now}usize, {idle}usize, {absolute}usize) {{ 1 }} else {{ 0 }}; code(access({state}usize, {now}usize, {idle}usize, {absolute}usize)) * 100 + code(logout({state}usize)) * 10 + active }}\n"));
        }
    }
    source.push_str("@id(\"session_oracle.tests.main\")\nfn main() -> i64 { 0 }\n");
    write_canonical_source(oracle.join("src/tests.spx"), &source);
    let revision = with_authenticated_project(&oracle.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let execution = revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .unwrap();
    for (index, (state, now, idle, absolute)) in rows.into_iter().enumerate() {
        let name = format!("test_case_{index}");
        let cases: Vec<_> = execution
            .cases()
            .iter()
            .filter(|case| case.name() == name)
            .collect();
        assert_eq!(
            cases.len(),
            1,
            "oracle case {name} must execute exactly once"
        );
        let semaprax::project::ProjectExecutionOutcome::Returned(expected) = cases[0].outcome()
        else {
            panic!(
                "oracle case {name} did not return: {:?}",
                cases[0].outcome()
            );
        };
        for decisions in [&fixture.host.decisions, &generated_decisions] {
            let active = decisions
                .session_is_usable(state, now, idle, absolute)
                .unwrap();
            let access = decisions
                .session_next_state_on_access(state, now, idle, absolute)
                .unwrap();
            let logout = decisions.session_next_state_on_logout(state).unwrap();
            let observed = access as i64 * 100 + logout as i64 * 10 + i64::from(active);
            assert_eq!(
                observed,
                *expected,
                "{} {name}",
                decisions.identities().prefix()
            );
        }
    }
}
