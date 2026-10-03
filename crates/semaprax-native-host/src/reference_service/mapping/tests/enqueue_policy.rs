//! New immediate enqueue admission is selected by the retained source revision.
use super::*;
use semaprax::interpreter::{PublicApiArgument, PublicApiEvaluationOutcome, PublicApiValue};
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;

fn generated(deny: bool) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("enqueue-policy-project");
    let scaffold =
        derive_project_scaffold_v1_with_layout("enqueue-parity", "service", ScaffoldLayout::Tables)
            .unwrap();
    for file in scaffold.files() {
        let path = directory.join(file.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.bytes()).unwrap();
    }
    install_generated_create_decision(directory.path());
    if deny {
        let path = directory.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let expression = "claim_is_legal(pending_state, schedule_is_due(now_tick, next_run_tick))";
        assert_eq!(original.matches(expression).count(), 1);
        let changed = original.replace(expression, "false");
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

fn source_bool(revision: &ProjectRevision, identity: &str, args: &[PublicApiArgument<'_>]) -> bool {
    let evaluation = revision
        .evaluate_service_decision_v1(identity, args, STEPS)
        .unwrap();
    match evaluation.outcome {
        PublicApiEvaluationOutcome::Returned(PublicApiValue::Bool(value)) => value,
        other => panic!("unexpected checked decision: {other:?}"),
    }
}

#[test]
fn enqueue_admission_matches_std_jobs_reference_and_generated_scaffold() {
    let (_directory, revision) = generated(false);
    let generated_engine = DecisionEngine::bind(&revision, STEPS).unwrap();
    let reference = fixture();
    // Pending, scheduled, leased, succeeded, and invalid status; before, at,
    // and after the due boundary. Oracle uses actual std.jobs invocations.
    for state in [0, 1, 2, 4, 255] {
        for (now, next) in [(9, 10), (10, 10), (11, 10)] {
            let due = source_bool(
                &revision,
                "std.jobs.schedule.is_due",
                &[
                    PublicApiArgument::Usize(now),
                    PublicApiArgument::Usize(next),
                ],
            );
            let expected = source_bool(
                &revision,
                "std.jobs.claim.is_legal",
                &[
                    PublicApiArgument::Usize(state),
                    PublicApiArgument::Bool(due),
                ],
            );
            for engine in [&generated_engine, &reference.host.decisions] {
                assert_eq!(
                    engine.enqueue_is_legal(state, now, next).unwrap(),
                    expected,
                    "{} state={state} now={now} next={next}",
                    engine.identities().prefix()
                );
            }
        }
    }
}

#[test]
fn source_enqueue_denial_and_evaluation_failure_preserve_host_state() {
    let (_directory, revision) = generated(true);
    // Test fixture host stores a static revision; keep its checked alternate
    // revision alive using the same test-only lifetime convention.
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    let mut fixture = fixture();
    let authenticated = Authenticated {
        account: 1,
        session: String::new(),
    };
    let request = exchange(
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"key","desc":"work"}"#,
        None,
    );
    let before = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();
    let outbound_count = std::fs::read_dir(fixture.outbound.path()).unwrap().count();
    fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
    let refused = enqueue_job(
        &mut fixture.host,
        &mut fixture.committed,
        &request,
        &authenticated,
    );
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(
        field(&refused.body, "error").as_str(),
        Some("enqueue_not_admitted")
    );
    // Fuel exhaustion also refuses without publishing any candidate state.
    fixture.host.decisions = DecisionEngine::bind(revision, 1).unwrap();
    let failed = enqueue_job(
        &mut fixture.host,
        &mut fixture.committed,
        &request,
        &authenticated,
    );
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
