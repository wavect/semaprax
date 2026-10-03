//! Checked webhook admission on the explicit signed-timestamp v2 route.
use super::*;
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;
const ADAPTER: &str = "semaprax-json-events-v2";

fn canonical(path: &Path, source: &str) {
    let parsed = semaprax::parse(source, path).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

fn generated(body: Option<&str>) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("webhook-policy-project");
    let scaffold =
        derive_project_scaffold_v1_with_layout("webhook-parity", "service", ScaffoldLayout::Tables)
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
            .find("@id(\"webhook_parity.core.webhook_delivery_policy_is_admitted\")")
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

fn pending(adapter: &str) -> (Fixture, String) {
    let mut fixture = fixture_with_telemetry("https://127.0.0.1:9", adapter);
    let registered = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/register",
            r#"{"username":"webhookpolicy","password":"correct horse 7"}"#,
            None,
        ),
        &mut || Some(1000),
    );
    assert_eq!(registered.status, 201, "{}", registered.body);
    let logged_in = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/login",
            r#"{"username":"webhookpolicy","password":"correct horse 7"}"#,
            None,
        ),
        &mut || Some(1000),
    );
    assert_eq!(logged_in.status, 200, "{}", logged_in.body);
    let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();
    let enqueued = handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange(
            "POST",
            "/v1/jobs/enqueue",
            r#"{"key":"webhook-policy","desc":"public"}"#,
            Some(&token),
        ),
        &mut || Some(1000),
    );
    assert_eq!(enqueued.status, 200, "{}", enqueued.body);
    (fixture, token)
}

fn finish(fixture: &mut Fixture, token: &str, now: u64) -> PendingResponse {
    handle_with_clock(
        &mut fixture.host,
        &mut fixture.committed,
        &exchange("POST", "/v1/jobs/1/complete", "", Some(token)),
        &mut || Some(now),
    )
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

#[test]
fn source_webhook_denial_and_evaluator_failure_precede_every_write() {
    for (body, budget, status, error) in [
        ("false", STEPS, 403, "webhook_not_admitted"),
        ("let mut remaining = 1000000usize; while remaining > 0usize { remaining = remaining - 1usize; remaining > 0usize } true", 10_000, 500, "decision_failed"),
    ] {
        let (_directory, revision) = generated(Some(body));
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let (mut fixture, token) = pending(ADAPTER);
        fixture.host.decisions = DecisionEngine::bind(revision, budget).unwrap();
        assert!(fixture.host.decisions.completed_job_metric_is_admitted(COMPLETED_JOB_METRIC_LABEL, COMPLETED_JOB_METRIC_VALUE, false).unwrap());
        let state = fixture.committed.state.render();
        let digest = fixture.committed.digest.clone();
        let state_files = inventory(fixture._state.path());
        let outbound = inventory(fixture.outbound.path());
        let response = finish(&mut fixture, &token, 1000);
        assert_eq!(response.status, status, "{}", response.body);
        assert_eq!(field(&response.body, "error").as_str(), Some(error));
        assert_unchanged(&fixture, &state, &digest, &state_files, &outbound);
    }
}

#[test]
fn webhook_v2_default_and_exact_facts_complete_while_v1_and_otlp_skip_policy() {
    let placeholder_body = format!(
        r#"{{"desc":"public","event":"job.completed","job_id":1,"owner":1,"schema":"semaprax.json-event.v2","signature":"{}","signed_at":1000}}"#,
        "0".repeat(64)
    );
    let exact = format!("byte_len(signature) == 64usize && payload_len == {}usize && signed_at == 1000 && now == 1000 && !key_exists && byte_len(existing_descriptor) == 0usize && byte_len(candidate_descriptor) == 71usize && attempt == 1 && !carries_secret", placeholder_body.len());
    for (adapter, body) in [
        (ADAPTER, None),
        (ADAPTER, Some(exact.as_str())),
        ("semaprax-json-events", Some("false")),
        ("otlp-http-json", Some("false")),
    ] {
        let (_directory, revision) = generated(body);
        let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
        let (mut fixture, token) = pending(adapter);
        fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
        let response = finish(&mut fixture, &token, 1000);
        assert_eq!(response.status, 200, "{}", response.body);
        assert_eq!(
            fixture.committed.state.job_by_id(1).unwrap().state,
            JobState::Completed
        );
        assert!(!inventory(fixture.outbound.path()).is_empty());
    }
}

struct Accepted;
impl semaprax::outbound_host_adapter::OutboundAdapter for Accepted {
    fn send(
        &mut self,
        _: &semaprax::outbound_host_adapter::PreparedRequest,
    ) -> semaprax::outbound_host_adapter::AdapterObservation {
        semaprax::outbound_host_adapter::AdapterObservation::Response {
            status: 202,
            body: Vec::new(),
        }
    }
}

fn seed(fixture: &mut Fixture, legacy: bool, desc: &str) {
    if legacy {
        delivery::deliver_completion_telemetry(
            &mut fixture.host.outbound_store,
            &fixture.host.deployment_binding,
            &fixture.host.telemetry_origin,
            ServiceTelemetryAdapter::SemapraxJsonEvents,
            1,
            1,
            desc,
            fixture.host.secrets.webhook_key(),
            &mut Accepted,
        )
        .unwrap();
    } else {
        delivery::webhook_v2::prepare(
            &fixture.host.outbound_store,
            &fixture.host.deployment_binding,
            &fixture.host.telemetry_origin,
            1,
            1,
            desc,
            fixture.host.secrets.webhook_key(),
            1000,
        )
        .unwrap()
        .deliver(
            &mut fixture.host.outbound_store,
            fixture.host.secrets.webhook_key(),
            &mut Accepted,
        )
        .unwrap();
    }
}

#[test]
fn webhook_v2_authenticated_restart_reuses_time_and_stale_or_conflicting_facts_deny() {
    for (desc, now, status) in [
        ("public", 1300, 200),
        ("public", 1301, 403),
        ("different", 1001, 403),
    ] {
        let (mut fixture, token) = pending(ADAPTER);
        seed(&mut fixture, false, desc);
        let reopened = Box::leak(Box::new(
            platform::hold_directory(fixture.outbound.path()).unwrap(),
        ));
        fixture.host.outbound_store = OutboundDeliveryStore::new(reopened);
        let state = fixture.committed.state.render();
        let digest = fixture.committed.digest.clone();
        let state_files = inventory(fixture._state.path());
        let outbound = inventory(fixture.outbound.path());
        let response = finish(&mut fixture, &token, now);
        assert_eq!(response.status, status, "{}", response.body);
        assert_eq!(
            inventory(fixture.outbound.path()),
            outbound,
            "no redispatch or marker rewrite"
        );
        if status == 200 {
            assert_eq!(
                fixture.committed.state.job_by_id(1).unwrap().webhook,
                WebhookSettlement::Uncertain
            );
        } else {
            assert_eq!(
                field(&response.body, "error").as_str(),
                Some("webhook_not_admitted")
            );
            assert_unchanged(&fixture, &state, &digest, &state_files, &outbound);
        }
    }
}

#[test]
fn webhook_v2_legacy_tamper_and_clock_failure_provide_no_invented_facts() {
    for mode in ["legacy", "tamper", "clock", "overflow"] {
        let (mut fixture, token) = pending(ADAPTER);
        if mode == "legacy" || mode == "tamper" {
            seed(&mut fixture, mode == "legacy", "public");
            if mode == "tamper" {
                let marker = std::fs::read_dir(fixture.outbound.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == "marker")
                    })
                    .unwrap();
                std::fs::write(marker, b"tampered\n").unwrap();
            }
        }
        let state = fixture.committed.state.render();
        let digest = fixture.committed.digest.clone();
        let state_files = inventory(fixture._state.path());
        let outbound = inventory(fixture.outbound.path());
        let mut sample = 0;
        let response = handle_with_clock(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
            &mut || {
                sample += 1;
                if sample == 1 {
                    Some(1000)
                } else {
                    match mode {
                        "clock" => None,
                        "overflow" => Some(u64::MAX),
                        _ => Some(1000),
                    }
                }
            },
        );
        assert_eq!(
            response.status,
            if mode == "clock" || mode == "overflow" {
                500
            } else {
                503
            },
            "{}",
            response.body
        );
        assert_unchanged(&fixture, &state, &digest, &state_files, &outbound);
    }
}

#[test]
fn webhook_source_adapter_matches_std_webhook_jobs_and_generated_scaffold() {
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
    type Row = (Vec<u8>, u64, i64, i64, bool, Vec<u8>, Vec<u8>);
    let signature = vec![b'a'; 64];
    let mut rows: Vec<Row> = vec![
        (signature.clone(), 200, 1000, 1000, false, vec![], vec![1]),
        (signature.clone(), 65536, 1000, 1300, true, vec![1], vec![1]),
        (signature.clone(), 65537, 1000, 1000, false, vec![], vec![1]),
        (signature.clone(), 200, 1000, 1301, false, vec![], vec![1]),
        (signature.clone(), 200, 1300, 1000, false, vec![], vec![1]),
        (signature.clone(), 200, 1301, 1000, false, vec![], vec![1]),
        (signature.clone(), 200, 1000, 1000, true, vec![1], vec![2]),
        (signature.clone(), 200, 1000, 1000, true, vec![], vec![]),
    ];
    rows.push((vec![b'g'; 64], 200, 1000, 1000, false, vec![], vec![1]));
    rows.push((vec![b'a'; 63], 200, 1000, 1000, false, vec![], vec![1]));
    let (oracle, _held) = TempDir::hold("webhook-policy-oracle");
    std::fs::create_dir_all(oracle.join("src")).unwrap();
    std::fs::write(oracle.join("semaprax.toml"), concat!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"webhook-oracle\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n",
        "\n[modules]\nentry = \"webhook_oracle.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"webhook_oracle.tests\"]\n",
        "\n[exports]\nweb = [\"webhook_oracle.app.probe\"]\n\n[dependencies]\nstd.jobs = \"=0.1.0\"\nstd.webhook = \"=0.1.0\"\n",
    )).unwrap();
    canonical(&oracle.join("src/app.spx"), "module webhook_oracle.app;\n@id(\"webhook_oracle.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool { byte_len(view) < 1000000usize }\n@id(\"webhook_oracle.app.main\")\nfn main() -> i64 { 0 }\n");
    let mut source = String::from(concat!(
        "module webhook_oracle.tests;\n",
        "use function @id(\"std.webhook.delivery_admitted_guarded\") from std.webhook as admit;\n",
        "use function @id(\"std.webhook.attempt_admitted\") from std.webhook as attempt;\n",
        "use function @id(\"std.jobs.idempotency.enqueue_outcome\") from std.jobs as enqueue;\n",
    ));
    for (index, (signature, len, signed_at, now, exists, prior, candidate)) in
        rows.iter().enumerate()
    {
        source.push_str(&format!("@id(\"webhook_oracle.tests.test_case_{index}\")\nfn test_case_{index}() -> i64 {{ let signature = {}; let prior = {}; let candidate = {}; if admit(array_as_slice(signature), {len}usize, {signed_at}, {now}, false, false, false, false, false, false) && attempt(1) && enqueue({exists}, array_as_slice(prior), array_as_slice(candidate)) != 2usize {{ 1 }} else {{ 0 }} }}\n", bytes(signature), bytes(prior), bytes(candidate)));
    }
    source.push_str("@id(\"webhook_oracle.tests.main\")\nfn main() -> i64 { 0 }\n");
    canonical(&oracle.join("src/tests.spx"), &source);
    let oracle_revision = with_authenticated_project(&oracle.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let execution = oracle_revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .unwrap();
    let (_generated, revision) = generated(None);
    let generated_engine = DecisionEngine::bind(&revision, STEPS).unwrap();
    let reference = fixture();
    for (index, (signature, len, signed_at, now, exists, prior, candidate)) in
        rows.iter().enumerate()
    {
        let name = format!("test_case_{index}");
        let cases: Vec<_> = execution
            .cases()
            .iter()
            .filter(|case| case.name() == name)
            .collect();
        assert_eq!(cases.len(), 1);
        let semaprax::project::ProjectExecutionOutcome::Returned(expected) = cases[0].outcome()
        else {
            panic!("oracle {name}: {:?}", cases[0].outcome());
        };
        for engine in [&reference.host.decisions, &generated_engine] {
            assert_eq!(
                i64::from(
                    engine
                        .completed_job_webhook_is_admitted(
                            signature, *len, *signed_at, *now, *exists, prior, candidate
                        )
                        .unwrap()
                ),
                *expected,
                "{name}"
            );
        }
    }
}

#[test]
fn pre_v2_source_still_serves_v1_and_otlp_but_v2_fails_before_delivery() {
    let (directory, _revision) = generated(None);
    let path = directory.join("src/core.spx");
    let original = std::fs::read_to_string(&path).unwrap();
    let start = original
        .find("@id(\"webhook_parity.core.completed_job_webhook_is_admitted\")")
        .unwrap();
    let end = start + original[start..].find("\n}\n").unwrap() + 3;
    let source = format!("{}{}", &original[..start], &original[end..]).replace(
        "completed_job_webhook_is_admitted(array_as_slice(webhook_signature), 128usize, 1000, 1100, false, array_as_slice(delivery_descriptor), array_as_slice(delivery_descriptor))",
        "webhook_delivery_policy_is_admitted(array_as_slice(webhook_signature), 128usize, 1000, 1100, false, array_as_slice(delivery_descriptor), array_as_slice(delivery_descriptor), 1, false)",
    );
    canonical(&path, &source);
    let revision = with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    for adapter in ["semaprax-json-events", "otlp-http-json", ADAPTER] {
        let (mut fixture, token) = pending(adapter);
        fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
        let response = finish(&mut fixture, &token, 1000);
        if adapter == ADAPTER {
            assert_eq!(response.status, 500, "{}", response.body);
            assert_eq!(
                field(&response.body, "error").as_str(),
                Some("decision_failed")
            );
            assert_eq!(
                fixture.committed.state.job_by_id(1).unwrap().state,
                JobState::Pending
            );
            assert!(inventory(fixture.outbound.path()).is_empty());
        } else {
            assert_eq!(response.status, 200, "{}", response.body);
        }
    }
}
