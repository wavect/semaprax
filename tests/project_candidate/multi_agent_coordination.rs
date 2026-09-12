//! #207: multi-agent semantic transaction coordination as proof data, not a
//! scheduler. See `docs/MULTI-AGENT-COORDINATION-V1.md`.
//!
//! The focused unit suite in
//! `src/project/candidate/multi_agent_coordination.rs` covers every
//! diagnostic path and capacity bound; this harness exercises the same
//! surface through the crate's public `semaprax::project` re-export (the
//! path a real caller uses) and adds one case the unit suite cannot: a
//! session going stale against a *genuinely different* candidate produced by
//! an actual applied semantic change, not a hand-tampered JSON field.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::project::{record_scheduling_comparison, SemanticChange};
use semaprax::project::{
    with_authenticated_project, AgentProposal, CoordinationParticipant, OperationClass,
    ProjectCandidate, ProjectRevision, SchedulingObservation,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

const APP: &str = "module coordination.harness;\n\
@id(\"coordination.harness.divide\") fn divide(left: i64, right: i64) -> i64\n\
    requires right != 0\n\
{\n    left / right\n}\n\
@id(\"coordination.harness.main\") fn main() -> i64 { divide(4, 2) }\n\
@id(\"coordination.harness.helper\") fn helper() -> i64 { 7 }\n";

const OTHER: &str = "module coordination.harness.tests;\n\
@id(\"coordination.harness.tests.check\") fn main() -> i64 { 0 }\n";

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-multi-agent-coordination-harness-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "multi-agent-coordination-harness"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "coordination.harness"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = []
tests = ["coordination.harness.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [("src/app.spx", APP), ("src/tests.spx", OTHER)] {
            let parsed = semaprax::parse(text, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root)
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open(revision: &Arc<ProjectRevision>) -> ProjectCandidate {
    ProjectCandidate::open(Arc::clone(revision), revision.project_revision()).unwrap()
}

#[test]
fn a_session_cannot_be_governed_by_one_of_its_own_participants() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.harness.divide"],
        budget_units: 10,
    }];
    let error = candidate
        .open_coordination_session(candidate.candidate_digest(), "agent-a", &participants)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z504");
}

#[test]
fn compatible_disjoint_proposals_across_agents_are_returned_in_order_never_merged() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.harness.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.harness.helper"],
            budget_units: 10,
        },
    ];
    let session = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &participants)
        .unwrap();
    let session_value: Value = serde_json::from_str(&session).unwrap();
    let base = session_value["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.harness.divide"],
            operation_class: OperationClass::Contract,
            intention: "tighten divide's precondition",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.harness.helper"],
            operation_class: OperationClass::Call,
            intention: "rename helper's call site",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    assert_eq!(evaluation["conflicts"], json!([]));
    assert_eq!(evaluation["rejected"], json!([]));
    assert_eq!(evaluation["compatible_order"].as_array().unwrap().len(), 2);
    // No path here ever grants merge, publication, or execution authority.
    for key in [
        "merge_authority",
        "publication_authority",
        "execution_authority",
    ] {
        assert_eq!(evaluation[key], json!(false), "{key} must stay false");
    }
}

#[test]
fn two_agents_on_the_same_target_id_conflict_and_neither_is_silently_chosen() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.harness.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.harness.divide"],
            budget_units: 10,
        },
    ];
    let session = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &participants)
        .unwrap();
    let session_value: Value = serde_json::from_str(&session).unwrap();
    let base = session_value["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.harness.divide"],
            operation_class: OperationClass::Contract,
            intention: "add a stricter precondition",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.harness.divide"],
            operation_class: OperationClass::Effect,
            intention: "add a logging effect",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let conflicts = evaluation["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["conflict_class"], json!("same_target"));
    assert_eq!(evaluation["compatible_order"], json!([]));
}

#[test]
fn a_session_is_stale_once_a_real_intervening_semantic_change_lands() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let candidate = open(&revision);
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.harness.divide"],
        budget_units: 10,
    }];
    let session = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &participants)
        .unwrap();

    // A genuine intervening change: add a sibling declaration, producing a
    // real new candidate with a different `candidate_revision` -- not a
    // hand-edited session record.
    let intent = json!({"kind":"add_declaration","target":"coordination.harness.divide","declaration":{
        "id":"coordination.harness.divide-wrapper","name":"divide_wrapper",
        "parameters":[{"name":"left","type":"i64","mode":"value"},{"name":"right","type":"i64","mode":"value"}],
        "return_type":"i64","effects":[],"requires":[],"ensures":[],
        "body":{"kind":"call","target":"coordination.harness.divide","arguments":[
            {"kind":"place","name":"left"},{"kind":"place","name":"right"}
        ]}
    }});
    let change = SemanticChange::new(candidate.revision().project_revision(), &intent).unwrap();
    let changed_candidate = candidate
        .apply(candidate.candidate_digest(), &change)
        .unwrap();
    assert_ne!(
        changed_candidate.candidate_digest(),
        candidate.candidate_digest()
    );

    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: changed_candidate.base_revision().project_revision(),
        target_ids: &["coordination.harness.divide"],
        operation_class: OperationClass::Call,
        intention: "proposal against the pre-change session",
    }];
    let error = changed_candidate
        .evaluate_agent_proposals(changed_candidate.candidate_digest(), &session, &proposals)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z503");
}

#[test]
fn scheduling_comparison_is_evidence_only_and_grants_no_execution_authority() {
    let sequential = SchedulingObservation {
        agent_count: 1,
        wall_clock_units: 100,
        retries: 1,
        cost_units: 10,
        review_items_opened: 2,
    };
    let parallel = SchedulingObservation {
        agent_count: 3,
        wall_clock_units: 50,
        retries: 4,
        cost_units: 25,
        review_items_opened: 5,
    };
    let comparison = record_scheduling_comparison(sequential, parallel).unwrap();
    let comparison: Value = serde_json::from_str(&comparison).unwrap();
    assert_eq!(comparison["wall_clock_speedup"], json!(2.0));
    assert_eq!(comparison["execution_authority"], json!(false));
    assert_eq!(comparison["publication_authority"], json!(false));
}
