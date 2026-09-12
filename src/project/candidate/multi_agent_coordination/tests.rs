use super::*;
use crate::project::{with_authenticated_project, ProjectRevision, SemanticChange};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

// `divide` lives in a separate module/file from `main` so that
// `main`'s call to it is a genuine *cross-file* dependency edge: the
// reverse impact artifact this module's cross-target detection reads
// only records the six cross-file edge families (see
// `WorkspaceAnalysis::image_symbol`'s `edge_scope:
// "six_cross_file_families"`), never same-file/same-module calls.
const APP: &str = "module coordination.app;\n\
use function @id(\"coordination.divide\") from coordination.lib as divide;\n\
@id(\"coordination.main\") fn main() -> i64 { divide(4, 2) }\n\
@id(\"coordination.helper\") fn helper() -> i64 { 7 }\n";

const LIB: &str = "module coordination.lib;\n\
@id(\"coordination.divide\") fn divide(left: i64, right: i64) -> i64\n\
requires right != 0\n\
{\n    left / right\n}\n";

const OTHER: &str = "module coordination.tests;\n\
@id(\"coordination.tests.check\") fn main() -> i64 { 0 }\n";

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-multi-agent-coordination-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "multi-agent-coordination"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "coordination.app"
sources = ["src/app.spx", "src/lib.spx", "src/tests.spx"]
web_exports = []
tests = ["coordination.tests"]
"#,
        )
        .unwrap();
        for (path, text) in [
            ("src/app.spx", APP),
            ("src/lib.spx", LIB),
            ("src/tests.spx", OTHER),
        ] {
            let parsed = crate::parse(text, path).unwrap();
            std::fs::write(root.join(path), crate::format::canonical(&parsed)).unwrap();
        }
        Self(root)
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    /// Exact on-disk bytes of every authoritative source file, for
    /// asserting a failed or stale transaction never touched them (see
    /// the analogous `bytes()`/`unchanged_raw_sources` idiom used
    /// elsewhere in this repository's candidate fixtures).
    fn bytes(&self) -> Vec<Vec<u8>> {
        ["semaprax.toml", "src/app.spx", "src/lib.spx", "src/tests.spx"]
            .iter()
            .map(|path| std::fs::read(self.0.join(path)).unwrap())
            .collect()
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

fn session_for<'a>(
    candidate: &ProjectCandidate,
    coordinator: &str,
    participants: &[CoordinationParticipant<'a>],
) -> Value {
    let session = candidate
        .open_coordination_session(candidate.candidate_digest(), coordinator, participants)
        .unwrap();
    serde_json::from_str(&session).unwrap()
}

#[test]
fn a_session_cannot_be_governed_by_one_of_its_own_participants() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let error = candidate
        .open_coordination_session(candidate.candidate_digest(), "agent-a", &participants)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z504");

    // A genuinely distinct coordinator over the exact same participants
    // succeeds.
    let session = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &participants)
        .unwrap();
    let session: Value = serde_json::from_str(&session).unwrap();
    assert_eq!(session["coordination_authority"], json!(false));
}

#[test]
fn a_granted_scope_id_that_does_not_exist_is_rejected_independently() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.does-not-exist"],
        budget_units: 10,
    }];
    let error = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &participants)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z503");
}

#[test]
fn session_open_capacity_and_identity_bounds_are_enforced() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());

    let too_many: Vec<CoordinationParticipant<'_>> = (0..MAX_COORDINATION_PARTICIPANTS + 1)
        .map(|i| CoordinationParticipant {
            agent_id: Box::leak(format!("agent-{i}").into_boxed_str()),
            granted_scope: &["coordination.divide"],
            budget_units: 1,
        })
        .collect();
    let error = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &too_many)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z502");

    let duplicate = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide"],
            budget_units: 1,
        },
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.helper"],
            budget_units: 1,
        },
    ];
    let error = candidate
        .open_coordination_session(candidate.candidate_digest(), "coordinator", &duplicate)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z501");
}

#[test]
fn compatible_independent_proposals_from_different_agents_are_both_returned_in_order() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.helper"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Contract,
            intention: "tighten divide's precondition",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.helper"],
            operation_class: OperationClass::Call,
            intention: "rename helper's call site",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    assert_eq!(evaluation["rejected"], json!([]));
    assert_eq!(evaluation["conflicts"], json!([]));
    let compatible = evaluation["compatible_order"].as_array().unwrap();
    assert_eq!(compatible.len(), 2);
    assert_eq!(compatible[0]["agent_id"], json!("agent-a"));
    assert_eq!(compatible[1]["agent_id"], json!("agent-b"));
    assert_eq!(evaluation["merge_authority"], json!(false));
    assert_eq!(evaluation["publication_authority"], json!(false));
    assert_eq!(evaluation["execution_authority"], json!(false));
}

#[test]
fn two_agents_targeting_the_same_id_are_a_same_target_conflict_not_a_merge() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Contract,
            intention: "add a stricter precondition",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Effect,
            intention: "add a logging effect",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    assert_eq!(evaluation["rejected"], json!([]));
    let conflicts = evaluation["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["conflict_class"], json!("same_target"));
    assert_eq!(conflicts[0]["affected_ids"], json!(["coordination.divide"]));
    let resolutions = conflicts[0]["allowed_resolution_choices"]
        .as_array()
        .unwrap();
    assert!(resolutions.iter().any(|c| c == "manual_reconciliation"));
    // Neither conflicting proposal is silently chosen into the
    // compatible order.
    assert_eq!(evaluation["compatible_order"], json!([]));
}

#[test]
fn a_proposal_naming_a_target_outside_its_granted_scope_is_rejected_at_scope_not_earlier() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    // "coordination.helper" exists in the candidate (so this is not a
    // stale/does-not-exist case) but was never granted to agent-a.
    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &["coordination.helper"],
        operation_class: OperationClass::Call,
        intention: "agent-a reaches outside its granted scope",
    }];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let rejected = evaluation["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["reason"], json!("scope_violation"));
    assert_eq!(evaluation["compatible_order"], json!([]));
}

#[test]
fn a_proposal_from_an_unknown_agent_is_rejected_as_unknown_agent() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let proposals = [AgentProposal {
        agent_id: "agent-never-granted",
        declared_base_project_revision: &base,
        target_ids: &["coordination.divide"],
        operation_class: OperationClass::Call,
        intention: "an agent the session never admitted",
    }];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let rejected = evaluation["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["reason"], json!("unknown_agent"));
}

#[test]
fn a_proposal_with_a_stale_base_revision_is_rejected_as_stale_not_as_a_conflict() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.helper"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let session_text = session.to_string();

    // agent-b declares a base revision that never matches the session's
    // exact bound base (simulating it working off an intervening,
    // different snapshot); agent-a is fresh and unaffected.
    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: session["base_project_revision"].as_str().unwrap(),
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Call,
            intention: "agent-a proposes against the current base",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: "sha256:stale-simulated-base",
            target_ids: &["coordination.helper"],
            operation_class: OperationClass::Call,
            intention: "agent-b proposes against a base that has drifted",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let rejected = evaluation["rejected"].as_array().unwrap();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0]["agent_id"], json!("agent-b"));
    assert_eq!(rejected[0]["reason"], json!("stale_base_revision"));
    assert_eq!(evaluation["conflicts"], json!([]));
    let compatible = evaluation["compatible_order"].as_array().unwrap();
    assert_eq!(compatible.len(), 1);
    assert_eq!(compatible[0]["agent_id"], json!("agent-a"));
}

#[test]
fn a_session_bound_to_a_different_candidate_is_rejected_as_stale() {
    let fixture = Fixture::new();
    let candidate_one = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session_text = candidate_one
        .open_coordination_session(
            candidate_one.candidate_digest(),
            "coordinator",
            &participants,
        )
        .unwrap();

    // A distinct candidate opened over the same base revision (this
    // module never mutates it, so the digest is unchanged here, but
    // simulate a genuinely different candidate by asserting a
    // structurally-altered session is rejected instead): corrupt the
    // recorded candidate_revision to a value that can never be this
    // exact candidate's digest.
    let mut tampered: Value = serde_json::from_str(&session_text).unwrap();
    tampered["candidate_revision"] = json!("sha256:not-this-candidate");
    let tampered_text = tampered.to_string();

    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: tampered["base_project_revision"].as_str().unwrap(),
        target_ids: &["coordination.divide"],
        operation_class: OperationClass::Call,
        intention: "proposal against a session for a different candidate",
    }];
    let error = candidate_one
        .evaluate_agent_proposals(candidate_one.candidate_digest(), &tampered_text, &proposals)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z503");
}

#[test]
fn disjoint_but_graph_dependent_targets_are_flagged_as_a_cross_target_conflict() {
    // `coordination.main` calls `coordination.divide`: a real dependency
    // edge between two disjoint stable ids, already recorded in
    // `coordination.divide`'s own reverse impact artifact (`main` is a
    // "consumer" of `divide`). Two different agents independently
    // proposing against these two dependent-but-distinct ids must not
    // come back "compatible": that is exactly the case multi-agent
    // scheduling most needs to get right, so this asserts a
    // `cross_target` conflict, not a same-target one (the ids are
    // disjoint) and not silence.
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.main"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Contract,
            intention: "change divide's contract",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.main"],
            operation_class: OperationClass::Call,
            intention: "change main's call site",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let conflicts = evaluation["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["conflict_class"], json!("cross_target"));
    let affected_ids: BTreeSet<String> = conflicts[0]["affected_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        affected_ids,
        BTreeSet::from([
            "coordination.divide".to_owned(),
            "coordination.main".to_owned()
        ])
    );
    let witnesses = conflicts[0]["dependency_witnesses"].as_array().unwrap();
    assert!(witnesses
        .iter()
        .any(|w| w
            == &json!({"upstream": "coordination.divide", "downstream": "coordination.main"})));
    // Neither conflicting proposal is silently chosen into the
    // compatible order.
    assert_eq!(evaluation["compatible_order"], json!([]));
    let nonclaims = evaluation["nonclaims"].as_array().unwrap();
    assert!(nonclaims
        .iter()
        .any(|c| c == "graph_independence_can_miss_hidden_external_or_generated_coupling"));
}

#[test]
fn cross_target_conflict_direction_is_symmetric_regardless_of_proposal_order() {
    // Same dependency edge as above, but the caller's target
    // (`coordination.main`) is proposed by the *first* agent and the
    // callee (`coordination.divide`) by the second -- proving the
    // pairwise check does not depend on which side of the `a`/`b` pair
    // happens to hold the upstream id.
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.main"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.divide"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let proposals = [
        AgentProposal {
            agent_id: "agent-a",
            declared_base_project_revision: &base,
            target_ids: &["coordination.main"],
            operation_class: OperationClass::Call,
            intention: "change main's call site",
        },
        AgentProposal {
            agent_id: "agent-b",
            declared_base_project_revision: &base,
            target_ids: &["coordination.divide"],
            operation_class: OperationClass::Contract,
            intention: "change divide's contract",
        },
    ];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let conflicts = evaluation["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0]["conflict_class"], json!("cross_target"));
    assert_eq!(evaluation["compatible_order"], json!([]));
}

#[test]
fn evaluation_is_deterministic_so_a_repeated_call_can_never_adopt_a_partial_merge() {
    // This module holds no mutable server-side state across calls: every
    // evaluation is a pure recomputation from the caller-supplied
    // session and proposals. Proving two identical calls render
    // byte-identical output is the local evidence that a crash between
    // them can never leave behind a partially-adopted merge for a third
    // call to pick up -- there is no partial state to adopt.
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();
    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &["coordination.divide"],
        operation_class: OperationClass::Contract,
        intention: "tighten divide's precondition",
    }];

    let first = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    let second = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn proposal_batch_capacity_bounds_are_enforced() {
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let too_many_targets: Vec<&str> = (0..MAX_TARGET_IDS_PER_PROPOSAL + 1)
        .map(|_| "coordination.divide")
        .collect();
    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &too_many_targets,
        operation_class: OperationClass::Call,
        intention: "too many target ids",
    }];
    let error = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &proposals)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z502");
}

#[test]
fn scheduling_comparison_computes_bounded_ratios_and_rejects_out_of_bounds_metrics() {
    let sequential = SchedulingObservation {
        agent_count: 1,
        wall_clock_units: 100,
        retries: 2,
        cost_units: 10,
        review_items_opened: 3,
    };
    let parallel = SchedulingObservation {
        agent_count: 4,
        wall_clock_units: 40,
        retries: 5,
        cost_units: 30,
        review_items_opened: 6,
    };
    let comparison = record_scheduling_comparison(sequential, parallel).unwrap();
    let comparison: Value = serde_json::from_str(&comparison).unwrap();
    assert_eq!(comparison["wall_clock_speedup"], json!(2.5));
    assert_eq!(comparison["cost_ratio"], json!(3.0));
    assert_eq!(comparison["review_burden_delta"], json!(3));
    assert_eq!(comparison["retries_delta"], json!(3));
    assert_eq!(comparison["execution_authority"], json!(false));

    let zero_wall_clock = SchedulingObservation {
        wall_clock_units: 0,
        ..parallel
    };
    let error = record_scheduling_comparison(sequential, zero_wall_clock).unwrap_err();
    assert_eq!(error[0].code, "SPX-Z502");
}

#[test]
fn target_overlap_is_the_only_variable_between_a_parallelizable_pair_and_a_refused_pair() {
    // A direct control for "two genuinely disjoint transactions may
    // proceed in parallel, and the scheduler would have refused them had
    // they overlapped": same coordinator, same two agents, each granted
    // *both* stable ids up front, same operation classes and intentions
    // in both cases below. The only field that changes between the two
    // evaluations is agent-b's target id -- so overlap, not agent
    // identity, scope shape, or proposal count, is what flips the
    // verdict from safely parallelizable to refused.
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [
        CoordinationParticipant {
            agent_id: "agent-a",
            granted_scope: &["coordination.divide", "coordination.helper"],
            budget_units: 10,
        },
        CoordinationParticipant {
            agent_id: "agent-b",
            granted_scope: &["coordination.divide", "coordination.helper"],
            budget_units: 10,
        },
    ];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let agent_a = AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &["coordination.divide"],
        operation_class: OperationClass::Contract,
        intention: "agent-a works on divide",
    };
    let agent_b_disjoint = AgentProposal {
        agent_id: "agent-b",
        declared_base_project_revision: &base,
        target_ids: &["coordination.helper"],
        operation_class: OperationClass::Call,
        intention: "agent-b works on helper",
    };

    // Disjoint targets: genuinely safe to run in parallel.
    let parallel_case = [agent_a, agent_b_disjoint];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &parallel_case)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    assert_eq!(evaluation["conflicts"], json!([]));
    let compatible = evaluation["compatible_order"].as_array().unwrap();
    assert_eq!(compatible.len(), 2);

    // The control: only agent-b's target id changes, from the disjoint
    // "helper" to agent-a's own "divide". The scheduler must now refuse
    // to treat the pair as parallelizable.
    let agent_b_overlapping = AgentProposal {
        target_ids: &["coordination.divide"],
        ..agent_b_disjoint
    };
    let refused_case = [agent_a, agent_b_overlapping];
    let evaluation = candidate
        .evaluate_agent_proposals(candidate.candidate_digest(), &session_text, &refused_case)
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    let conflicts = evaluation["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1);
    // Specific, not a shared catch-all: this is exactly `same_target`,
    // never a generic "conflict" label that would also cover the
    // cross_target class proven above.
    assert_eq!(conflicts[0]["conflict_class"], json!("same_target"));
    assert_eq!(evaluation["compatible_order"], json!([]));
}

#[test]
fn a_genuine_source_drift_makes_the_session_stale_and_leaves_authoritative_source_unchanged() {
    // Unlike `a_session_bound_to_a_different_candidate_is_rejected_as_stale`,
    // which corrupts a session's `candidate_revision` field by hand, this
    // drifts the *real* source on disk -- simulating another agent's
    // work landing between session-open and evaluation -- then proves
    // the stale evaluation fails closed and never itself touches the
    // (already-drifted) authoritative files.
    let fixture = Fixture::new();
    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.divide"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    // A real intervening edit: widen `divide`'s precondition on disk and
    // re-author the fixture's own source, exactly the way a second,
    // separately-authorized transaction would land a change between this
    // session's open and its evaluation.
    let drifted_lib = "module coordination.lib;\n\
@id(\"coordination.divide\") fn divide(left: i64, right: i64) -> i64\n\
requires right != 0 && left >= 0\n\
{\n    left / right\n}\n";
    let parsed = crate::parse(drifted_lib, "src/lib.spx").unwrap();
    let drifted_bytes = crate::format::canonical(&parsed);
    std::fs::write(fixture.0.join("src/lib.spx"), &drifted_bytes).unwrap();
    let after_drift = fixture.bytes();

    let drifted_candidate = open(&fixture.revision());
    assert_ne!(
        drifted_candidate.candidate_digest(),
        candidate.candidate_digest()
    );

    let proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &["coordination.divide"],
        operation_class: OperationClass::Contract,
        intention: "agent-a still proposes against the pre-drift session",
    }];
    let error = drifted_candidate
        .evaluate_agent_proposals(drifted_candidate.candidate_digest(), &session_text, &proposals)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z503");

    // The failed, stale evaluation performed no further write: the
    // already-drifted source is exactly what was written, byte for byte.
    assert_eq!(fixture.bytes(), after_drift);
}

#[test]
fn a_rename_rebase_lets_a_fresh_session_resolve_the_same_stable_id_while_the_old_session_goes_stale(
) {
    // "Rename/move rebase using stable IDs": an intervening
    // `rename_declaration` change is applied through the same
    // `ProjectCandidate::apply` path any real caller uses, changing
    // `coordination.helper`'s *display name* while its stable `@id` is
    // untouched by construction. The rename lives only in an
    // unpublished, in-memory `ProjectCandidate` -- it never rewrites
    // `src/app.spx` on disk, matching "a successful managed-workspace
    // transaction ... does not rewrite original source files".
    let fixture = Fixture::new();
    let before = fixture.bytes();

    let candidate = open(&fixture.revision());
    let participants = [CoordinationParticipant {
        agent_id: "agent-a",
        granted_scope: &["coordination.helper"],
        budget_units: 10,
    }];
    let session = session_for(&candidate, "coordinator", &participants);
    let base = session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_text = session.to_string();

    let rename_intent = json!({
        "kind": "rename_declaration",
        "target": "coordination.helper",
        "name": "helper_renamed",
    });
    let rename_change =
        SemanticChange::new(candidate.revision().project_revision(), &rename_intent).unwrap();
    let renamed = candidate
        .apply(candidate.candidate_digest(), &rename_change)
        .unwrap();
    assert_ne!(renamed.candidate_digest(), candidate.candidate_digest());

    // The rename really happened, in-memory only.
    let renamed_source = renamed
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .unwrap()
        .source()
        .to_owned();
    assert!(renamed_source.contains("helper_renamed"));
    assert!(!renamed_source.contains("fn helper("));

    // The old session, evaluated against the now-drifted (renamed)
    // candidate, fails closed: an intervening edit landed since the
    // session opened.
    let stale_proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &base,
        target_ids: &["coordination.helper"],
        operation_class: OperationClass::Call,
        intention: "agent-a still proposes by the pre-rename session",
    }];
    let error = renamed
        .evaluate_agent_proposals(renamed.candidate_digest(), &session_text, &stale_proposals)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-Z503");

    // A fresh session, opened against the renamed candidate and granting
    // the *same stable id*, still resolves it (existence is checked by
    // `@id`, never by display name) and a proposal against it is
    // accepted: multi-agent scope/target tracking survives the rename
    // exactly as "rename/move rebase using stable IDs" requires.
    let fresh_session = session_for(&renamed, "coordinator", &participants);
    let fresh_base = fresh_session["base_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let fresh_session_text = fresh_session.to_string();
    let fresh_proposals = [AgentProposal {
        agent_id: "agent-a",
        declared_base_project_revision: &fresh_base,
        target_ids: &["coordination.helper"],
        operation_class: OperationClass::Call,
        intention: "agent-a proposes by the same stable id after the rename",
    }];
    let evaluation = renamed
        .evaluate_agent_proposals(
            renamed.candidate_digest(),
            &fresh_session_text,
            &fresh_proposals,
        )
        .unwrap();
    let evaluation: Value = serde_json::from_str(&evaluation).unwrap();
    assert_eq!(evaluation["rejected"], json!([]));
    assert_eq!(evaluation["conflicts"], json!([]));
    assert_eq!(evaluation["compatible_order"].as_array().unwrap().len(), 1);

    // Authoritative source is unchanged throughout: neither the rename,
    // the failed stale evaluation, nor the successful post-rename
    // evaluation ever touched the fixture's files on disk.
    assert_eq!(fixture.bytes(), before);
}
