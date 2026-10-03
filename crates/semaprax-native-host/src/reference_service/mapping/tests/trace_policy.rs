//! Incoming trace metadata uses the retained source policy before effects.
use super::*;
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;
const TRACE: &str = "00-1234567890abcdef1234567890abcdef-abcdef1234567890-01";

fn canonical(path: &Path, source: &str) {
    let parsed = semaprax::parse(source, path).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

fn generated(body: Option<&str>) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("trace-policy-project");
    let scaffold =
        derive_project_scaffold_v1_with_layout("trace-parity", "service", ScaffoldLayout::Tables)
            .unwrap();
    for file in scaffold.files() {
        let path = directory.join(file.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.bytes()).unwrap();
    }
    install_generated_create_decision(directory.path());
    if let Some(body) = body {
        let path = directory.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let start = original
            .find("@id(\"trace_parity.core.trace_context_is_admitted\")")
            .unwrap();
        let body_start = start + original[start..].find("\n{\n").unwrap() + 3;
        let body_end = body_start + original[body_start..].find("\n}").unwrap();
        canonical(
            &path,
            &format!(
                "{}    {body}{}",
                &original[..body_start],
                &original[body_end..]
            ),
        );
    }
    let revision = with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    (directory, revision)
}

fn inventory(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().into(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect();
    entries.sort();
    entries
}

fn assert_unchanged(
    fixture: &Fixture,
    state: &str,
    digest: &str,
    state_files: &[(PathBuf, Vec<u8>)],
    outbound: &[(PathBuf, Vec<u8>)],
) {
    assert_eq!(fixture.committed.state.render(), state);
    assert_eq!(fixture.committed.digest, digest);
    assert_eq!(inventory(fixture._state.path()), state_files);
    assert_eq!(inventory(fixture.outbound.path()), outbound);
}

fn traced(method: &str, target: &str, body: &str, trace: &str) -> HttpExchange {
    let mut request = exchange(method, target, body, None);
    request
        .headers
        .push(("TraceParent".to_owned(), trace.to_owned()));
    request
}

#[test]
fn admitted_trace_fields_reach_source_and_allow_durable_registration() {
    let exact = "let expected_trace = [49u8, 50u8, 51u8, 52u8, 53u8, 54u8, 55u8, 56u8, 57u8, 48u8, 97u8, 98u8, 99u8, 100u8, 101u8, 102u8, 49u8, 50u8, 51u8, 52u8, 53u8, 54u8, 55u8, 56u8, 57u8, 48u8, 97u8, 98u8, 99u8, 100u8, 101u8, 102u8]; let expected_parent = [97u8, 98u8, 99u8, 100u8, 101u8, 102u8, 49u8, 50u8, 51u8, 52u8, 53u8, 54u8, 55u8, 56u8, 57u8, 48u8]; let expected_flags = [48u8, 49u8]; enqueue_outcome(true, trace_id, array_as_slice(expected_trace)) == 1usize && enqueue_outcome(true, parent_id, array_as_slice(expected_parent)) == 1usize && enqueue_outcome(true, trace_flags, array_as_slice(expected_flags)) == 1usize && !carries_secret";
    for body in [None, Some(exact)] {
        let (_directory, revision) = generated(body);
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let mut fixture = fixture();
        fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
        let before = fixture.committed.digest.clone();
        let response = handle_with_clock(
            &mut fixture.host,
            &mut fixture.committed,
            &traced(
                "POST",
                "/v1/register",
                r#"{"username":"traceuser","password":"correct horse 7"}"#,
                TRACE,
            ),
            &mut || panic!("registration must not sample a clock"),
        );
        assert_eq!(response.status, 201, "{}", response.body);
        assert_ne!(fixture.committed.digest, before);
        assert!(!inventory(fixture._state.path()).is_empty());
        assert!(inventory(fixture.outbound.path()).is_empty());
    }
}

#[test]
fn trace_denial_and_evaluator_failure_precede_clock_and_every_write() {
    for (body, budget, status, expected) in [
        ("false", STEPS, 400, "trace_not_admitted"),
        ("let mut remaining = 1000000usize; while remaining > 0usize { remaining = remaining - 1usize; remaining > 0usize } true", 10_000, 500, "decision_failed"),
    ] {
        let (_directory, revision) = generated(Some(body));
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let mut fixture = fixture();
        let credentials = r#"{"username":"traceuser","password":"correct horse 7"}"#;
        let registered = handle_with_clock(&mut fixture.host, &mut fixture.committed,
            &exchange("POST", "/v1/register", credentials, None), &mut || Some(1000));
        assert_eq!(registered.status, 201, "{}", registered.body);
        let login = handle_with_clock(&mut fixture.host, &mut fixture.committed,
            &exchange("POST", "/v1/login", credentials, None), &mut || Some(1000));
        assert_eq!(login.status, 200, "{}", login.body);
        let token = field(&login.body, "token").as_str().unwrap().to_owned();
        fixture.host.decisions = DecisionEngine::bind(revision, budget).unwrap();
        assert!(fixture.host.decisions.request_is_admitted(b"POST", b"/v1/login").unwrap());
        let state = fixture.committed.state.render();
        let digest = fixture.committed.digest.clone();
        let state_files = inventory(fixture._state.path());
        let outbound = inventory(fixture.outbound.path());
        for target in ["/v1/register", "/v1/login", "/v1/jobs/1/complete"] {
            let mut request = exchange("POST", target, credentials, Some(&token));
            request.headers.push(("traceparent".to_owned(), TRACE.to_owned()));
            let response = handle_with_clock(&mut fixture.host, &mut fixture.committed,
                &request, &mut || panic!("trace refusal must precede clock"));
            assert_eq!(response.status, status, "{}", response.body);
            assert_eq!(field(&response.body, "error").as_str(), Some(expected));
            assert_unchanged(&fixture, &state, &digest, &state_files, &outbound);
        }
        // Header absence never asks the optional source decision.
        let response = handle_with_clock(&mut fixture.host, &mut fixture.committed,
            &exchange("GET", "/v1/health", "", None), &mut || panic!("health clock"));
        assert_eq!(response.status, 200, "{}", response.body);
    }
}

#[test]
fn malformed_repeated_or_source_invalid_trace_headers_refuse_without_mutation() {
    let mut fixture = fixture();
    let state = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();
    let state_files = inventory(fixture._state.path());
    let outbound = inventory(fixture.outbound.path());
    let mut headers = vec![
        String::new(),
        TRACE[..54].to_owned(),
        format!("{TRACE}0"),
        TRACE.replacen("00-", "01-", 1),
        TRACE.replacen("00-", "00_", 1),
        TRACE.replace(
            "1234567890abcdef1234567890abcdef",
            "00000000000000000000000000000000",
        ),
        TRACE.replace("abcdef1234567890-", "0000000000000000-"),
        TRACE.replace('a', "A"),
        TRACE[..53].to_owned() + "0g",
    ];
    headers.push(TRACE.to_owned());
    for (index, value) in headers.iter().enumerate() {
        let mut request = traced("POST", "/v1/register", "{}", value);
        if index + 1 == headers.len() {
            request
                .headers
                .push(("traceparent".to_owned(), TRACE.to_owned()));
        }
        let response = handle_with_clock(
            &mut fixture.host,
            &mut fixture.committed,
            &request,
            &mut || panic!("invalid trace clock"),
        );
        assert_eq!(response.status, 400, "{value}: {}", response.body);
        assert_eq!(
            field(&response.body, "error").as_str(),
            Some("trace_not_admitted")
        );
        assert_unchanged(&fixture, &state, &digest, &state_files, &outbound);
    }
}

#[test]
fn optional_trace_identity_is_required_only_for_requests_with_traceparent() {
    let (directory, _revision) = generated(None);
    for entry in std::fs::read_dir(directory.join("src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "spx") {
            let source = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                &path,
                source.replace(
                    "trace_parity.core.trace_context_is_admitted",
                    "trace_parity.core.unused_trace_policy",
                ),
            )
            .unwrap();
        }
    }
    let revision = with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    let mut fixture = fixture();
    fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
    let before = fixture.committed.state.render();
    let without = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("GET", "/v1/health", "", None),
        &mut || panic!("health clock"),
    );
    assert_eq!(without.status, 200);
    let with = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &traced("GET", "/v1/health", "", TRACE),
        &mut || panic!("trace clock"),
    );
    assert_eq!(with.status, 500);
    assert_eq!(field(&with.body, "error").as_str(), Some("decision_failed"));
    assert_eq!(fixture.committed.state.render(), before);
    assert!(inventory(fixture._state.path()).is_empty());
    assert!(inventory(fixture.outbound.path()).is_empty());
}

#[test]
fn trace_source_matches_std_tracing_oracle_and_generated_scaffold() {
    fn bytes(value: &[u8]) -> String {
        if value.is_empty() {
            "[0u8; 0]".to_owned()
        } else {
            format!(
                "[{}]",
                value
                    .iter()
                    .map(|byte| format!("{byte}u8"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
    type Row = (Vec<u8>, Vec<u8>, Vec<u8>, bool, bool);
    let trace = TRACE.as_bytes()[3..35].to_vec();
    let parent = TRACE.as_bytes()[36..52].to_vec();
    let mut rows: Vec<Row> = vec![];
    for flags in [b"00", b"01", b"03", b"ff"] {
        rows.push((trace.clone(), parent.clone(), flags.to_vec(), false, true));
    }
    rows.extend([
        (vec![b'0'; 32], parent.clone(), b"01".to_vec(), false, false),
        (trace.clone(), vec![b'0'; 16], b"01".to_vec(), false, false),
        (
            trace[..31].to_vec(),
            parent.clone(),
            b"01".to_vec(),
            false,
            false,
        ),
        (vec![b'a'; 33], parent.clone(), b"01".to_vec(), false, false),
        (trace.clone(), vec![b'a'; 17], b"01".to_vec(), false, false),
        (trace.clone(), vec![], b"01".to_vec(), false, false),
        (vec![b'A'; 32], parent.clone(), b"01".to_vec(), false, false),
        (trace.clone(), vec![b'g'; 16], b"01".to_vec(), false, false),
        (trace.clone(), parent.clone(), b"0G".to_vec(), false, false),
        (trace.clone(), parent.clone(), b"001".to_vec(), false, false),
        (trace.clone(), parent.clone(), vec![], false, false),
        (trace, parent, b"01".to_vec(), true, false),
    ]);
    let (oracle, _held) = TempDir::hold("trace-policy-oracle");
    std::fs::create_dir_all(oracle.join("src")).unwrap();
    std::fs::write(oracle.join("semaprax.toml"), concat!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"trace-oracle\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n",
        "\n[modules]\nentry = \"trace_oracle.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"trace_oracle.tests\"]\n",
        "\n[exports]\nweb = [\"trace_oracle.app.probe\"]\n\n[dependencies]\nstd.tracing = \"=0.1.0\"\n",
    )).unwrap();
    canonical(&oracle.join("src/app.spx"), "module trace_oracle.app;\n@id(\"trace_oracle.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool { byte_len(view) < 1000000usize }\n@id(\"trace_oracle.app.main\")\nfn main() -> i64 { 0 }\n");
    let mut source = String::from("module trace_oracle.tests;\nuse function @id(\"std.tracing.trace_context_fields_admitted_guarded\") from std.tracing as admit;\n");
    for (index, (trace, parent, flags, secret, _)) in rows.iter().enumerate() {
        source.push_str(&format!("@id(\"trace_oracle.tests.test_case_{index}\")\nfn test_case_{index}() -> i64 {{ let trace = {}; let parent = {}; let flags = {}; if admit(array_as_slice(trace), array_as_slice(parent), array_as_slice(flags), {secret}, false, false, false, false, false) {{ 1 }} else {{ 0 }} }}\n", bytes(trace), bytes(parent), bytes(flags)));
    }
    source.push_str("@id(\"trace_oracle.tests.main\")\nfn main() -> i64 { 0 }\n");
    canonical(&oracle.join("src/tests.spx"), &source);
    let revision = with_authenticated_project(&oracle.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let execution = revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .unwrap();
    let (_generated, generated_revision) = generated(None);
    let generated_engine = DecisionEngine::bind(&generated_revision, STEPS).unwrap();
    let reference = fixture();
    for (index, (trace, parent, flags, secret, expected)) in rows.iter().enumerate() {
        let name = format!("test_case_{index}");
        let cases: Vec<_> = execution
            .cases()
            .iter()
            .filter(|case| case.name() == name)
            .collect();
        assert_eq!(cases.len(), 1);
        assert!(
            matches!(cases[0].outcome(), semaprax::project::ProjectExecutionOutcome::Returned(value) if *value == i64::from(*expected)),
            "{name}: {:?}",
            cases[0].outcome()
        );
        for engine in [&reference.host.decisions, &generated_engine] {
            assert_eq!(
                engine
                    .trace_context_is_admitted(trace, parent, flags, *secret)
                    .unwrap(),
                *expected,
                "{name}"
            );
        }
    }
}
