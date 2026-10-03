//! OTLP completion evaluates the retained log policy before payload or I/O.
use super::*;
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;
const BOUNDED_STEPS: usize = 10_000;
const EXACT_FACTS: &str = "level == 2u8 && threshold == 2u8 && field_count == 4usize && !(carries_password || carries_api_key || carries_bearer_token || carries_session_token || carries_webhook_signing_secret || carries_smtp_credential)";

fn canonical(path: &Path, source: &str) {
    let parsed = semaprax::parse(source, path).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

fn generated(body: Option<&str>) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("log-policy-project");
    let scaffold =
        derive_project_scaffold_v1_with_layout("log-parity", "service", ScaffoldLayout::Tables)
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
        let marker = "@id(\"log_parity.core.structured_log_policy_is_admitted\")";
        let start = original.find(marker).unwrap();
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

fn pending_fixture(adapter: &str) -> Fixture {
    let mut fixture = fixture_with_telemetry("https://127.0.0.1:9", adapter);
    let registered = handle(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"logpolicy","password":"correct horse 7"}"#,
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
            r#"{"username":"logpolicy","password":"correct horse 7"}"#,
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
            r#"{"key":"log-policy","desc":"public job description"}"#,
            Some(&token),
        ),
    );
    assert_eq!(enqueued.status, 200, "{}", enqueued.body);
    fixture
}

fn inventory(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, entries: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(current).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                entries.push((relative, Vec::new()));
                visit(root, &path, entries);
            } else {
                entries.push((relative, std::fs::read(path).unwrap()));
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries.sort();
    entries
}

#[test]
fn source_log_refusal_and_evaluation_failure_preserve_host_state() {
    for (body, budget, status, error) in [
        ("false", STEPS, 403, "log_not_admitted"),
        ("let mut remaining = 1000000usize;\n    while remaining > 0usize {\n        remaining = remaining - 1usize;\n        remaining > 0usize\n    }\n    true", BOUNDED_STEPS, 500, "decision_failed"),
    ] {
        let (_directory, revision) = generated(Some(body));
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let mut fixture = pending_fixture("otlp-http-json");
        fixture.host.decisions = DecisionEngine::bind(revision, budget).unwrap();
        // Prove the finite-budget failure belongs to the log decision, rather
        // than an earlier authorization/completion/metric decision.
        let decisions = &fixture.host.decisions;
        assert!(decisions.task_owner_authorized(1, 1, true).unwrap());
        assert!(!decisions.job_status_is_complete(0).unwrap());
        assert_eq!(decisions.mark_job_succeeded(0, 3).unwrap(), 4);
        assert!(decisions.completed_job_metric_is_admitted(
            COMPLETED_JOB_METRIC_LABEL, COMPLETED_JOB_METRIC_VALUE, false,
        ).unwrap());
        let before = fixture.committed.state.render();
        let digest = fixture.committed.digest.clone();
        let state_files = inventory(fixture._state.path());
        let outbound_files = inventory(fixture.outbound.path());
        let response = complete_job(
            &mut fixture.host, &mut fixture.committed, 1,
            &Authenticated { account: 1, session: String::new() },
        );
        assert_eq!(response.status, status, "{}", response.body);
        assert_eq!(field(&response.body, "error").as_str(), Some(error));
        assert_eq!(fixture.committed.state.render(), before);
        assert_eq!(fixture.committed.digest, digest);
        assert_eq!(inventory(fixture._state.path()), state_files);
        assert_eq!(inventory(fixture.outbound.path()), outbound_files);
    }
}

#[test]
fn otlp_completion_supplies_exact_log_facts_and_json_events_skip_log_policy() {
    for (adapter, body) in [
        ("otlp-http-json", EXACT_FACTS),
        ("semaprax-json-events", "false"),
    ] {
        let (_directory, revision) = generated(Some(body));
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let mut fixture = pending_fixture(adapter);
        fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
        let response = complete_job(
            &mut fixture.host,
            &mut fixture.committed,
            1,
            &Authenticated {
                account: 1,
                session: String::new(),
            },
        );
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(
            fixture.committed.state.job_by_id(1).unwrap().state,
            JobState::Completed
        );
        assert!(!inventory(fixture.outbound.path()).is_empty());
    }
}

#[test]
fn structured_log_policy_matches_std_log_and_generated_scaffold() {
    let (_generated, generated_revision) = generated(None);
    let generated_engine = DecisionEngine::bind(&generated_revision, STEPS).unwrap();
    let reference = fixture();
    let mut rows: Vec<(u8, u8, u64, [bool; 6])> = [
        (0, 0, 0),
        (1, 2, 4),
        (2, 2, 4),
        (3, 2, 4),
        (5, 5, 4),
        (6, 2, 4),
        (2, 6, 4),
        (2, 2, 32),
        (2, 2, 33),
    ]
    .into_iter()
    .map(|(level, threshold, count)| (level, threshold, count, [false; 6]))
    .collect();
    for secret in 0..6 {
        let mut flags = [false; 6];
        flags[secret] = true;
        rows.push((2, 2, 4, flags));
    }
    // Independent normal project execution retains std.log's requires clause.
    // The host wrapper and Rust comparisons do not supply the oracle.
    let (oracle, _held) = TempDir::hold("log-policy-oracle");
    std::fs::create_dir_all(oracle.join("src")).unwrap();
    std::fs::write(oracle.join("semaprax.toml"), concat!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"log-oracle\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n",
        "\n[modules]\nentry = \"log_oracle.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"log_oracle.tests\"]\n",
        "\n[exports]\nweb = [\"log_oracle.app.probe\"]\n\n[dependencies]\nstd.log = \"=0.1.0\"\nstd.log.redact = \"=0.1.0\"\n",
    )).unwrap();
    canonical(&oracle.join("src/app.spx"), "module log_oracle.app;\n@id(\"log_oracle.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool { byte_len(view) < 1000000usize }\n@id(\"log_oracle.app.main\")\nfn main() -> i64 { 0 }\n");
    let mut source = String::from(concat!(
        "module log_oracle.tests;\n",
        "use function @id(\"std.log.level-enabled\") from std.log as enabled;\n",
        "use function @id(\"std.log.redact.field_count_within_budget\") from std.log.redact as within_budget;\n",
        "use function @id(\"std.log.redact.event_is_safe\") from std.log.redact as safe;\n",
    ));
    for (index, (level, threshold, count, flags)) in rows.iter().enumerate() {
        let [password, api, bearer, session, webhook, smtp] = flags;
        source.push_str(&format!("@id(\"log_oracle.tests.test_case_{index}\")\nfn test_case_{index}() -> i64 {{ if {level}u8 <= 5u8 && {threshold}u8 <= 5u8 && enabled({level}u8, {threshold}u8) && within_budget({count}usize) && safe({password}, {api}, {bearer}, {session}, {webhook}, {smtp}) {{ 1 }} else {{ 0 }} }}\n"));
    }
    source.push_str("@id(\"log_oracle.tests.main\")\nfn main() -> i64 { 0 }\n");
    canonical(&oracle.join("src/tests.spx"), &source);
    let revision = with_authenticated_project(&oracle.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let execution = revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .unwrap();
    for (index, (level, threshold, count, flags)) in rows.into_iter().enumerate() {
        let name = format!("test_case_{index}");
        let cases: Vec<_> = execution
            .cases()
            .iter()
            .filter(|case| case.name() == name)
            .collect();
        assert_eq!(cases.len(), 1, "oracle case executes once");
        let semaprax::project::ProjectExecutionOutcome::Returned(expected) = cases[0].outcome()
        else {
            panic!("oracle {name}: {:?}", cases[0].outcome());
        };
        for engine in [&reference.host.decisions, &generated_engine] {
            let secret_flags = flags
                .iter()
                .enumerate()
                .fold(0u8, |bits, (index, flag)| bits | (u8::from(*flag) << index));
            let actual = engine
                .completed_job_log_is_admitted(level, threshold, count, secret_flags)
                .unwrap();
            assert_eq!(
                i64::from(actual),
                *expected,
                "{} {name}",
                engine.identities().prefix()
            );
        }
    }
}

#[test]
fn completion_log_adapter_refuses_unknown_secret_bits() {
    let (_directory, revision) = generated(None);
    let generated_engine = DecisionEngine::bind(&revision, STEPS).unwrap();
    let reference = fixture();
    for engine in [&reference.host.decisions, &generated_engine] {
        for bits in [64, 128, 255] {
            assert!(!engine.completed_job_log_is_admitted(2, 2, 4, bits).unwrap());
        }
    }
    // Unknown bits must refuse before a callee that would exhaust the budget.
    let (_directory, revision) = generated(Some("let mut remaining = 1000000usize; while remaining > 0usize { remaining = remaining - 1usize; remaining > 0usize } true"));
    let engine = DecisionEngine::bind(&revision, BOUNDED_STEPS).unwrap();
    for bits in [64, 128, 255] {
        assert!(!engine.completed_job_log_is_admitted(2, 2, 4, bits).unwrap());
    }
}

#[test]
fn default_completion_policy_completes_with_the_real_metric_gate() {
    for adapter in ["otlp-http-json", "semaprax-json-events"] {
        let mut fixture = pending_fixture(adapter);
        let response = complete_job(
            &mut fixture.host,
            &mut fixture.committed,
            1,
            &Authenticated {
                account: 1,
                session: String::new(),
            },
        );
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(
            fixture.committed.state.job_by_id(1).unwrap().state,
            JobState::Completed
        );
        assert!(!inventory(fixture.outbound.path()).is_empty());
    }
}
