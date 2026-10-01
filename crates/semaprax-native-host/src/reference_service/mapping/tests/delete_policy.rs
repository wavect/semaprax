//! Task deletion is selected by the checked scaffold transaction decision.
use super::*;
use semaprax::interpreter::{PublicApiArgument, PublicApiEvaluationOutcome, PublicApiValue};
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectRevision, ScaffoldLayout};
use std::sync::Arc;

const STEPS: usize = crate::reference_service::decisions::DECISION_MAX_STEPS;

fn generated(deny: bool) -> (TempDir, Arc<ProjectRevision>) {
    let (directory, _held) = TempDir::hold("delete-policy-project");
    let scaffold =
        derive_project_scaffold_v1_with_layout("delete-parity", "service", ScaffoldLayout::Tables)
            .unwrap();
    for file in scaffold.files() {
        let path = directory.join(file.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file.bytes()).unwrap();
    }
    if deny {
        let path = directory.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let expression = "transaction_state == 0usize";
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

fn source_bool(revision: &ProjectRevision, state: u64) -> bool {
    let evaluation = revision
        .evaluate_service_decision_v1(
            "delete_parity.core.delete_is_committed",
            &[PublicApiArgument::Usize(state)],
            STEPS,
        )
        .unwrap();
    match evaluation.outcome {
        PublicApiEvaluationOutcome::Returned(PublicApiValue::Bool(value)) => value,
        other => panic!("unexpected checked decision: {other:?}"),
    }
}

#[test]
fn delete_admission_matches_reference_and_generated_scaffold() {
    let (_directory, revision) = generated(false);
    let generated_engine = DecisionEngine::bind(&revision, STEPS).unwrap();
    let reference = fixture();
    for state in [0, 1, 2, 255] {
        let expected = source_bool(&revision, state);
        for engine in [&generated_engine, &reference.host.decisions] {
            assert_eq!(
                engine.delete_is_committed(state).unwrap(),
                expected,
                "{} transaction_state={state}",
                engine.identities().prefix()
            );
        }
    }
}

#[test]
fn source_delete_denial_and_evaluation_failure_preserve_host_state() {
    let (_directory, revision) = generated(true);
    let revision: &'static ProjectRevision = Box::leak(Box::new(revision));
    let mut fixture = fixture();
    fixture.committed.state.tasks.push(Task {
        id: 1,
        owner: 1,
        title: "before".to_owned(),
        status: TaskStatus::Open,
    });
    let authenticated = Authenticated {
        account: 1,
        session: String::new(),
    };
    let before = fixture.committed.state.render();
    let digest = fixture.committed.digest.clone();

    fixture.host.decisions = DecisionEngine::bind(revision, STEPS).unwrap();
    let refused = delete_task(
        &mut fixture.host,
        &mut fixture.committed,
        1,
        &authenticated,
    );
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert_eq!(
        field(&refused.body, "error").as_str(),
        Some("delete_not_admitted")
    );

    fixture.host.decisions = DecisionEngine::bind(revision, 1).unwrap();
    let failed = delete_task(
        &mut fixture.host,
        &mut fixture.committed,
        1,
        &authenticated,
    );
    assert_eq!(failed.status, 500, "{}", failed.body);
    assert_eq!(
        field(&failed.body, "error").as_str(),
        Some("decision_failed")
    );
    assert_eq!(fixture.committed.state.render(), before);
    assert_eq!(fixture.committed.digest, digest);
}
