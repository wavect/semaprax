//! Job completion is selected by the retained source decision before delivery.
use super::*;
use semaprax::interpreter::{PublicApiArgument, PublicApiEvaluationOutcome, PublicApiValue};
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;
const SUCCESS_STATE: u64 = 4;

#[derive(Clone, Copy)]
enum CompletionSource {
    Success,
    Refuse,
    ExhaustBudget,
    MetricRefuse,
    MetricExhaustBudget,
}

fn generated(source: CompletionSource) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("completion-policy-project");
    let scaffold = derive_project_scaffold_v1_with_layout(
        "completion-parity",
        "service",
        ScaffoldLayout::Tables,
    )
    .unwrap();
    for file in scaffold.files() {
        let path = directory.join(file.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.bytes()).unwrap();
    }
    install_generated_create_decision(directory.path());
    if matches!(
        source,
        CompletionSource::Refuse | CompletionSource::ExhaustBudget
    ) {
        let path = directory.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let marker = "    4usize\n}";
        assert_eq!(original.matches(marker).count(), 1);
        let replacement = match source {
            CompletionSource::Success => unreachable!(),
            CompletionSource::Refuse => "    6usize\n}",
            CompletionSource::ExhaustBudget => {
                "    let mut remaining = 1000usize;\n    while remaining > 0usize {\n        remaining = remaining - 1usize;\n        remaining > 0usize\n    }\n    4usize\n}"
            }
            CompletionSource::MetricRefuse | CompletionSource::MetricExhaustBudget => {
                unreachable!()
            }
        };
        let changed = original.replace(marker, replacement);
        let parsed = semaprax::parse(&changed, &path).unwrap();
        std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    }
    if matches!(
        source,
        CompletionSource::MetricRefuse | CompletionSource::MetricExhaustBudget
    ) {
        let path = directory.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let module = original
            .lines()
            .find_map(|line| line.strip_prefix("module "))
            .and_then(|line| line.strip_suffix(';'))
            .expect("generated core module declaration");
        let identity = format!("@id(\"{module}.completed_job_metric_is_admitted\")");
        let start = original
            .find(&identity)
            .expect("generated completed-job metric decision");
        let end = original[start + identity.len()..]
            .find("\n@id(\"")
            .map(|offset| start + identity.len() + offset + 1)
            .unwrap_or_else(|| original.len());
        let body = match source {
            CompletionSource::MetricRefuse => "    false\n",
            CompletionSource::MetricExhaustBudget => {
                "    let mut remaining = 1000usize;\n    while remaining > 0usize {\n        remaining = remaining - 1usize;\n        remaining > 0usize\n    }\n    true\n"
            }
            CompletionSource::Success
            | CompletionSource::Refuse
            | CompletionSource::ExhaustBudget => unreachable!(),
        };
        let replacement = format!(
            "{identity}\nfn completed_job_metric_is_admitted(label: borrow Slice<u8>, value: borrow Slice<u8>, carries_secret: bool) -> bool\n{{\n{body}}}\n"
        );
        let changed = format!("{}{}{}", &original[..start], replacement, &original[end..]);
        let parsed = semaprax::parse(&changed, &path).unwrap();
        std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
    }
    let revision = with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    (directory, revision)
}

fn source_status(revision: &ProjectRevision, attempt: u8, max_attempts: u8) -> u64 {
    let evaluation = revision
        .evaluate_service_decision_v1(
            "completion_parity.core.mark_job_succeeded",
            &[
                PublicApiArgument::U8(attempt),
                PublicApiArgument::U8(max_attempts),
            ],
            STEPS,
        )
        .unwrap();
    match evaluation.outcome {
        PublicApiEvaluationOutcome::Returned(PublicApiValue::Usize(value)) => value,
        other => panic!("unexpected checked decision: {other:?}"),
    }
}

#[test]
fn completion_state_matches_reference_and_generated_scaffold() {
    let (_directory, revision) = generated(CompletionSource::Success);
    let generated_engine = DecisionEngine::bind(&revision, STEPS).unwrap();
    let reference = fixture();
    for (attempt, max_attempts) in [(0, 0), (0, 3), (3, 3), (255, 0)] {
        let expected = source_status(&revision, attempt, max_attempts);
        assert_eq!(expected, SUCCESS_STATE);
        for engine in [&generated_engine, &reference.host.decisions] {
            assert_eq!(
                engine.mark_job_succeeded(attempt, max_attempts).unwrap(),
                expected,
                "{} attempt={attempt} max_attempts={max_attempts}",
                engine.identities().prefix()
            );
        }
    }
}

#[test]
fn source_completion_refusal_and_evaluation_failure_preserve_host_state() {
    let (_directory, revision) = generated(CompletionSource::Refuse);
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    let mut fixture = fixture();
    fixture.committed.state.jobs.push(Job {
        id: 1,
        owner: 1,
        key: "completion".to_owned(),
        desc: "before".to_owned(),
        state: JobState::Pending,
        webhook: WebhookSettlement::None,
    });
    let authenticated = Authenticated {
        account: 1,
        session: String::new(),
    };
    let before = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();
    let outbound_count = std::fs::read_dir(fixture.outbound.path()).unwrap().count();

    fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
    let refused = complete_job(&mut fixture.host, &mut fixture.committed, 1, &authenticated);
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(
        field(&refused.body, "error").as_str(),
        Some("completion_not_admitted")
    );

    let (_directory, exhausted) = generated(CompletionSource::ExhaustBudget);
    let exhausted: &'static ProjectRevision = Box::leak(Box::new(exhausted));
    fixture.host.decisions = DecisionEngine::bind(exhausted, 100).unwrap();
    let failed = complete_job(&mut fixture.host, &mut fixture.committed, 1, &authenticated);
    assert_eq!(failed.status, 500, "{}", failed.body);
    assert_eq!(
        field(&failed.body, "error").as_str(),
        Some("decision_failed")
    );
    assert_eq!(fixture.committed.state.render(), before);
    assert_eq!(fixture.committed.digest, digest);
    assert_eq!(
        std::fs::read_dir(fixture.outbound.path()).unwrap().count(),
        outbound_count
    );
}

#[test]
fn source_metric_refusal_and_evaluation_failure_preserve_host_state() {
    let (_directory, revision) = generated(CompletionSource::MetricRefuse);
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    let mut fixture = fixture();
    fixture.committed.state.jobs.push(Job {
        id: 1,
        owner: 1,
        key: "completion".to_owned(),
        desc: "before".to_owned(),
        state: JobState::Pending,
        webhook: WebhookSettlement::None,
    });
    let authenticated = Authenticated {
        account: 1,
        session: String::new(),
    };
    let before = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();
    let outbound_count = std::fs::read_dir(fixture.outbound.path()).unwrap().count();

    fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
    let refused = complete_job(&mut fixture.host, &mut fixture.committed, 1, &authenticated);
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(
        field(&refused.body, "error").as_str(),
        Some("metric_not_admitted")
    );

    let (_directory, exhausted) = generated(CompletionSource::MetricExhaustBudget);
    let exhausted: &'static ProjectRevision = Box::leak(Box::new(exhausted));
    fixture.host.decisions = DecisionEngine::bind(exhausted, 100).unwrap();
    let failed = complete_job(&mut fixture.host, &mut fixture.committed, 1, &authenticated);
    assert_eq!(failed.status, 500, "{}", failed.body);
    assert_eq!(
        field(&failed.body, "error").as_str(),
        Some("decision_failed")
    );
    assert_eq!(fixture.committed.state.render(), before);
    assert_eq!(fixture.committed.digest, digest);
    assert_eq!(
        std::fs::read_dir(fixture.outbound.path()).unwrap().count(),
        outbound_count
    );
}

#[test]
fn completion_metric_matches_std_metrics_and_generated_scaffold() {
    fn canonical(path: &std::path::Path, source: &str) {
        let parsed = semaprax::parse(source, path).unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
    }
    fn byte_array(bytes: &[u8]) -> String {
        if bytes.is_empty() {
            "[0u8; 0]".to_owned()
        } else {
            format!(
                "[{}]",
                bytes
                    .iter()
                    .map(|byte| format!("{byte}u8"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
    let (_generated, generated_revision) = generated(CompletionSource::Success);
    let generated_engine = DecisionEngine::bind(&generated_revision, STEPS).unwrap();
    let reference = fixture();
    let mut rows = vec![
        (b"job_state".to_vec(), b"succeeded".to_vec(), false),
        (b"job_state".to_vec(), b"succeeded".to_vec(), true),
        (Vec::new(), b"value".to_vec(), false),
        (b"1bad".to_vec(), b"value".to_vec(), false),
        (b"label".to_vec(), Vec::new(), false),
        (vec![b'a'; 64], vec![b'b'; 128], false),
        (vec![b'a'; 65], b"value".to_vec(), false),
        (b"label".to_vec(), vec![b'b'; 129], false),
    ];
    for unsafe_byte in [0, b'\n', b'\r', b'"', b'\\'] {
        rows.push((b"label".to_vec(), vec![unsafe_byte], false));
    }
    let (oracle, _held) = TempDir::hold("metric-policy-oracle");
    std::fs::create_dir_all(oracle.join("src")).unwrap();
    std::fs::write(oracle.join("semaprax.toml"), concat!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"metric-oracle\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n",
        "\n[modules]\nentry = \"metric_oracle.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"metric_oracle.tests\"]\n",
        "\n[exports]\nweb = [\"metric_oracle.app.probe\"]\n\n[dependencies]\nstd.metrics = \"=0.1.0\"\n",
    )).unwrap();
    canonical(&oracle.join("src/app.spx"), "module metric_oracle.app;\n@id(\"metric_oracle.app.probe\")\nfn probe(view: borrow Slice<u8>) -> bool { byte_len(view) < 1000000usize }\n@id(\"metric_oracle.app.main\")\nfn main() -> i64 { 0 }\n");
    let mut source = String::from(concat!(
        "module metric_oracle.tests;\n",
        "use function @id(\"std.metrics.counter-increment\") from std.metrics as increment;\n",
        "use function @id(\"std.metrics.try-admit-labeled-series-guarded\") from std.metrics as admit;\n",
    ));
    for (index, (label, value, secret)) in rows.iter().enumerate() {
        let label_source = byte_array(label);
        let value_source = byte_array(value);
        source.push_str(&format!("@id(\"metric_oracle.tests.test_case_{index}\")\nfn test_case_{index}() -> i64 {{ let label = {label_source}; let value = {value_source}; if increment(0, 1) == 1 && admit(0, array_as_slice(label), array_as_slice(value), {secret}, false, false, false, false, false) == 1 {{ 1 }} else {{ 0 }} }}\n"));
    }
    source.push_str("@id(\"metric_oracle.tests.main\")\nfn main() -> i64 { 0 }\n");
    canonical(&oracle.join("src/tests.spx"), &source);
    let revision = with_authenticated_project(&oracle.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let execution = revision
        .execute_test(&semaprax::project::ProjectExecutionOptions::default())
        .unwrap();
    for (index, (label, value, secret)) in rows.into_iter().enumerate() {
        let name = format!("test_case_{index}");
        let cases: Vec<_> = execution
            .cases()
            .iter()
            .filter(|case| case.name() == name)
            .collect();
        assert_eq!(cases.len(), 1);
        let semaprax::project::ProjectExecutionOutcome::Returned(expected) = cases[0].outcome()
        else {
            panic!("metric oracle {name}: {:?}", cases[0].outcome());
        };
        for engine in [&reference.host.decisions, &generated_engine] {
            let actual = engine
                .completed_job_metric_is_admitted(&label, &value, secret)
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
