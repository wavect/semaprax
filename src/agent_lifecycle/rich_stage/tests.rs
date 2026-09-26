use std::path::PathBuf;
use std::process::Command;

use crate::agent_runtime::AgentCancellation;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{
    PreparedRetainedCall, RetainedCallEvaluation, RetainedCallOutcome, RetainedField,
    RetainedRecord, RetainedValue,
};

use super::{
    bind_rich_proposal_stages, run_rich_turn, run_rich_turn_on, RichProposalStages,
    RichStageBackend, RichTurnOutcome,
};

// NOTE on scope: Proposal here is a genuinely multi-field record crossing
// the checked `authorize`/`reduce` stages as ONE nominal argument -- not
// exploded into per-field scalar parameters the way
// `stages::proposal_projection` requires. It is not *nested* (no
// record-in-record): a nested Copy-only Proposal record does not clear the
// retained-call seam today (`interpreter::retained_call`'s
// `resolved_data_parameter_is_admitted` admits a Value-mode nominal
// parameter only for `class`-kind declarations or a fully flat `record`;
// `agent_interaction_schema::shape::derive` in turn refuses a `class` root
// outright (`type.kind`)). That specific gap -- confirmed empirically while
// building this fixture, not asserted from documentation -- is recorded in
// the module's top-level "Known limitation" and in the final report; this
// corpus proves what is achievable today (a flat multi-field nominal
// Proposal, decoded through the real Agent Interaction Schema v1 grammar
// rather than the old per-field scalar grammar) rather than overclaiming
// nesting this compiler does not yet admit through this seam.
const FIXTURE: &str = r#"
module test.agent_lifecycle_rich_stage;

@id("rich.type.proposal")
record Proposal {
    @id("rich.field.urgent")
    urgent: bool,
    @id("rich.field.weight")
    weight: i64,
}

@id("rich.type.state")
record State {
    @id("rich.field.count")
    count: i64,
}

@id("rich.type.decision")
variant Decision {
    @id("rich.decision.grant")
    Grant {
        @id("rich.decision.grant.flag")
        flag: bool,
        @id("rich.decision.grant.seal")
        seal: Bytes,
    },
    @id("rich.decision.refuse")
    Refuse {
        @id("rich.decision.refuse.code")
        code: i64,
    },
}

@id("rich.type.transition")
variant Transition {
    @id("rich.transition.continue")
    Continue {
        @id("rich.transition.continue.next_count")
        next_count: i64,
        @id("rich.transition.continue.marker")
        marker: Bytes,
    },
    @id("rich.transition.fail")
    Fail {
        @id("rich.transition.fail.code")
        code: u8,
    },
}

@id("rich.fn.authorize")
fn authorize(state: State, proposal: Proposal) -> Decision {
    let seal = [1u8, 2u8];
    if proposal.urgent {
        Decision::Grant { flag: true, seal: bytes_copy(array_as_slice(seal)) }
    } else {
        Decision::Refuse { code: proposal.weight }
    }
}

@id("rich.fn.reduce")
fn reduce(state: State, proposal: Proposal, outcome: own Bytes) -> Transition {
    let marker = [3u8, 4u8];
    if proposal.weight == -1 {
        Transition::Fail { code: 0u8 }
    } else {
        if proposal.weight < 0 {
            Transition::Fail { code: 255u8 }
        } else {
            Transition::Continue {
                next_count: state.count + proposal.weight,
                marker: bytes_copy(array_as_slice(marker)),
            }
        }
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A second fixture, structurally identical except every declared name is
/// renamed. Its schema revision (and therefore its schema digest) must be
/// unchanged, proving field identity persists through display renaming.
const RENAMED_FIXTURE: &str = r#"
module test.agent_lifecycle_rich_stage_renamed;

@id("rich.type.proposal")
record ProposalXX {
    @id("rich.field.urgent")
    uu: bool,
    @id("rich.field.weight")
    ww: i64,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// A structurally different fixture (an added field), whose schema revision
/// must therefore differ from [`FIXTURE`]'s.
const STRUCTURAL_FIXTURE: &str = r#"
module test.agent_lifecycle_rich_stage_structural;

@id("rich.type.proposal")
record Proposal {
    @id("rich.field.urgent")
    urgent: bool,
    @id("rich.field.weight")
    weight: i64,
    @id("rich.field.extra")
    extra: i64,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(source: &str, label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-agent-lifecycle-rich-stage-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn bind(label: &str) -> RichProposalStages {
    let path = write_temp(FIXTURE, label);
    bind_rich_proposal_stages(
        FIXTURE,
        &path,
        "rich.type.proposal",
        "rich.type.state",
        "rich.fn.authorize",
        "rich.fn.reduce",
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}

fn state(count: i64) -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new("rich.type.state"),
        fields: vec![RetainedField {
            field: DeclarationId::new("rich.field.count"),
            value: RetainedValue::I64(count),
        }],
    })
}

fn target_tools_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
        && Command::new("node")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
}

/// One canonical `semaprax.agent-interaction-value.v1` document for
/// `rich.type.proposal`, built by hand in exact declared field order so it
/// is admitted by the byte-exact canonical-replay decoder.
fn proposal_document(schema_digest: &str, urgent: bool, weight: i64) -> String {
    let digest = crate::diagnostic::quote_json(schema_digest);
    format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"rich.type.proposal\",\"schema_digest\":{digest},\"value\":{{\"fields\":{{\"rich.field.urgent\":{urgent},\"rich.field.weight\":\"{weight}\"}}}}}}\n"
    )
}

fn run_on(
    stages: &RichProposalStages,
    backend: RichStageBackend<'_>,
    urgent: bool,
    weight: i64,
    cancellation: &AgentCancellation,
) -> Result<RichTurnOutcome, crate::diagnostic::Diagnostic> {
    let document = proposal_document(stages.schema().schema().digest(), urgent, weight);
    run_rich_turn_on(
        stages,
        state(10),
        document.as_bytes(),
        Vec::new(),
        10_000,
        cancellation,
        backend,
    )
}

fn admitted_proposal(stages: &RichProposalStages, urgent: bool, weight: i64) -> RetainedValue {
    let document = proposal_document(stages.schema().schema().digest(), urgent, weight);
    let decoded = stages
        .schema
        .decode(document.as_bytes())
        .expect("fixture proposal decodes");
    let admitted = stages
        .proposal_binding
        .admit(decoded)
        .expect("fixture proposal is admitted");
    super::to_retained(&stages.proposal_graph, &admitted).expect("fixture proposal projects")
}

fn raw_stage(
    stages: &RichProposalStages,
    backend: RichStageBackend<'_>,
    prepared: &PreparedRetainedCall,
    arguments: &[RetainedValue],
) -> RetainedCallEvaluation {
    crate::agent_lifecycle::authorization::dispatch_on(
        stages.backend(backend),
        &stages.program,
        prepared,
        arguments,
        10_000,
    )
    .unwrap_or_else(|error| panic!("{backend:?}: {error:?}"))
}

fn assert_raw_target_parity(
    label: &str,
    expected: &RetainedCallEvaluation,
    actual: &RetainedCallEvaluation,
    observes_owned_copy_out: bool,
) {
    // This is deliberately before `run_rich_turn_on` reduces the variants to
    // a turn outcome: raw grant seals and Continue markers must be equal as
    // bytes, not merely as a matching branch label or scalar state.
    assert_eq!(actual.outcome, expected.outcome, "{label}: raw outcome");
    assert_eq!(actual.failure, expected.failure, "{label}: failure");
    let RetainedCallOutcome::Returned(RetainedValue::Variant(expected_value)) = &expected.outcome
    else {
        panic!("{label}: expected returned variant");
    };
    let expected_owned_leaves = expected_value
        .fields
        .iter()
        .filter(|field| matches!(field.value, RetainedValue::Bytes(_)))
        .count();
    if observes_owned_copy_out {
        assert_eq!(
            actual.cleanup_events,
            vec![
                crate::interpreter::OwnedDataCleanupEvent::CopyOutAndSettleBytes;
                expected_owned_leaves
            ],
            "{label}: each owned result leaf is copied out and settled exactly once"
        );
    } else {
        // Variant Bytes leave the Core Wasm stage through an indexed scalar
        // projection. This proves exact payload bytes but has no settled
        // owned-result copy-out event to observe at the boundary.
        assert!(
            actual.cleanup_events.is_empty(),
            "{label}: no fabricated cleanup event"
        );
    }
}

/// The turn facade intentionally reduces Decision/Transition into a small
/// public outcome, so inspect the raw retained results here. This keeps the
/// owned grant seal and Continue marker semantically observed through every
/// target codec rather than letting matching scalar branches hide byte drift.
#[test]
fn rich_target_backends_preserve_raw_grant_and_continue_byte_payloads() {
    if !target_tools_available() {
        eprintln!("skipping rich target raw-byte parity: clang or node unavailable");
        return;
    }
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("availability retains native host");
    let stages = bind("target-raw-payloads");
    let proposal = admitted_proposal(&stages, true, 7);
    let authorize_args = [state(10), proposal.clone()];
    let decision = raw_stage(
        &stages,
        RichStageBackend::Interpreter,
        &stages.authorize,
        &authorize_args,
    );
    let RetainedCallOutcome::Returned(RetainedValue::Variant(decision_value)) = &decision.outcome
    else {
        panic!("fixture authorize did not return Decision::Grant");
    };
    assert_eq!(
        decision_value.case,
        DeclarationId::new("rich.decision.grant")
    );
    assert_eq!(
        decision_value
            .fields
            .iter()
            .find(|field| field.field == DeclarationId::new("rich.decision.grant.seal"))
            .expect("Grant seal field")
            .value,
        RetainedValue::Bytes(vec![1, 2])
    );

    let reduce_args = [
        state(10),
        proposal,
        // A non-empty owned argument exercises the target's direct Bytes ABI
        // and its generated callee cleanup path, rather than a zero-length
        // carrier which could conceal a pointer/length mismatch.
        RetainedValue::Bytes(vec![0xa5, 0x5a]),
    ];
    let transition = raw_stage(
        &stages,
        RichStageBackend::Interpreter,
        &stages.reduce,
        &reduce_args,
    );
    let RetainedCallOutcome::Returned(RetainedValue::Variant(transition_value)) =
        &transition.outcome
    else {
        panic!("fixture reduce did not return Transition::Continue");
    };
    assert_eq!(
        transition_value.case,
        DeclarationId::new("rich.transition.continue")
    );
    assert_eq!(
        transition_value
            .fields
            .iter()
            .find(|field| field.field == DeclarationId::new("rich.transition.continue.marker"))
            .expect("Continue marker field")
            .value,
        RetainedValue::Bytes(vec![3, 4])
    );

    for (target, backend) in [
        ("native -O0", RichStageBackend::Native(&native_host)),
        (
            "native -O2",
            RichStageBackend::NativeOptimized(&native_host),
        ),
        ("Core Wasm", RichStageBackend::Wasm),
    ] {
        let actual_decision = raw_stage(&stages, backend, &stages.authorize, &authorize_args);
        assert_raw_target_parity(
            &format!("{target}: authorize"),
            &decision,
            &actual_decision,
            !matches!(backend, RichStageBackend::Wasm),
        );
        let actual_transition = raw_stage(&stages, backend, &stages.reduce, &reduce_args);
        assert_raw_target_parity(
            &format!("{target}: reduce"),
            &transition,
            &actual_transition,
            !matches!(backend, RichStageBackend::Wasm),
        );
    }
}

/// Rich Proposal uses one nominal Proposal argument rather than the frozen
/// scalar projection. This proves that the same checked proposal, grant
/// branch, transition, and u8 failure carrier agree on the interpreter,
/// native C11 at both optimization levels, and Core Wasm.
#[test]
fn every_target_backend_executes_rich_proposal_grant_refusal_and_fail_transitions() {
    if !target_tools_available() {
        eprintln!("skipping rich target parity: clang or node unavailable");
        return;
    }
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("availability retains native host");
    let stages = bind("target-parity");
    for (label, urgent, weight) in [
        ("grant", true, 7),
        ("refusal", false, 42),
        ("fail-zero", true, -1),
        ("fail-max", true, -3),
    ] {
        let expected = run_on(
            &stages,
            RichStageBackend::Interpreter,
            urgent,
            weight,
            &AgentCancellation::new(),
        )
        .unwrap_or_else(|error| panic!("{label}: interpreter: {error:?}"));
        for (target, backend) in [
            ("native -O0", RichStageBackend::Native(&native_host)),
            (
                "native -O2",
                RichStageBackend::NativeOptimized(&native_host),
            ),
            ("Core Wasm", RichStageBackend::Wasm),
        ] {
            assert_eq!(
                run_on(&stages, backend, urgent, weight, &AgentCancellation::new(),)
                    .unwrap_or_else(|error| panic!("{label}: {target}: {error:?}")),
                expected,
                "{label}: {target}"
            );
        }
    }
}

/// Cancellation and malformed proposal bytes are rejected by the semantic
/// kernel before target selection can build an artifact or execute either
/// stage. The exact diagnostic stays backend-independent.
#[test]
fn rich_target_backends_keep_cancellation_and_malformed_proposals_pre_dispatch() {
    let native_host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("native stage test host is available");
    let stages = bind("target-pre-dispatch");
    for backend in [
        RichStageBackend::Interpreter,
        RichStageBackend::Native(&native_host),
        RichStageBackend::NativeOptimized(&native_host),
        RichStageBackend::Wasm,
    ] {
        let cancellation = AgentCancellation::new();
        cancellation.cancel();
        let cancelled = run_on(&stages, backend, true, 1, &cancellation)
            .expect_err("a cancelled rich turn refuses before target execution");
        assert_eq!(cancelled.code, "SPX-G588");
        assert!(cancelled.message.contains("turn.cancelled"));

        let malformed = run_rich_turn_on(
            &stages,
            state(10),
            b"not json",
            Vec::new(),
            10_000,
            &AgentCancellation::new(),
            backend,
        )
        .expect_err("malformed proposal bytes refuse before target execution");
        assert_eq!(malformed.code, "SPX-G588");
        assert!(malformed.message.contains("turn.proposal_malformed"));

        // A forged variant carrying State's nominal ID is not State: state
        // is a record-only stage input. This check runs before decoding or
        // selecting a target artifact, so its exact refusal is shared by all
        // sealed backends.
        let malformed_state =
            RetainedValue::Variant(crate::interpreter::retained_call::RetainedVariant {
                variant: DeclarationId::new("rich.type.state"),
                case: DeclarationId::new("rich.state.not_a_record"),
                fields: Vec::new(),
            });
        let malformed_state = run_rich_turn_on(
            &stages,
            malformed_state,
            b"not json",
            Vec::new(),
            10_000,
            &AgentCancellation::new(),
            backend,
        )
        .expect_err("a variant is never admitted as a State record");
        assert_eq!(malformed_state.code, "SPX-G588");
        assert!(malformed_state.message.contains("turn.state_identity"));
    }
}

#[test]
fn granted_urgent_proposal_reaches_reduce_and_the_harvested_next_state_is_exact() {
    let stages = bind("granted");
    let digest = stages.schema().schema().digest().to_owned();
    let document = proposal_document(&digest, true, 7);
    let outcome = run_rich_turn(
        &stages,
        state(10),
        document.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new(),
    )
    .unwrap();
    let RichTurnOutcome::Continue(next) = outcome else {
        panic!("expected Continue, got {outcome:?}");
    };
    let RetainedValue::Record(record) = next else {
        panic!("expected a State record");
    };
    assert_eq!(record.record, DeclarationId::new("rich.type.state"));
    assert_eq!(record.fields.len(), 1);
    assert_eq!(
        record.fields[0].field,
        DeclarationId::new("rich.field.count")
    );
    // 10 (prior state) + 7 (the decoded Proposal's own field value, carried
    // through the real interpreter as one nominal argument) -- proves the
    // Proposal's actual VALUE, not merely a matching digest, drove the
    // subsequent state transition.
    assert_eq!(record.fields[0].value, RetainedValue::I64(17));
}

#[test]
fn non_urgent_proposal_is_refused_before_reduce_ever_dispatches() {
    let stages = bind("refused");
    let digest = stages.schema().schema().digest().to_owned();
    let document = proposal_document(&digest, false, 42);
    let outcome = run_rich_turn(
        &stages,
        state(10),
        document.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new(),
    )
    .unwrap();
    // The exact refusal code is the Proposal's real value, harvested
    // through `authorize`, not a placeholder.
    assert!(matches!(outcome, RichTurnOutcome::Refused(42)));
}

#[test]
fn granted_proposal_with_a_negative_weight_reaches_reduce_and_fails_with_the_exact_code() {
    let stages = bind("fail-transition");
    let digest = stages.schema().schema().digest().to_owned();
    let document = proposal_document(&digest, true, -3);
    let outcome = run_rich_turn(
        &stages,
        state(10),
        document.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert!(matches!(outcome, RichTurnOutcome::Fail(255)));
}

#[test]
fn malformed_and_wrong_nominal_and_stale_schema_proposals_refuse_before_any_stage_evaluates() {
    let stages = bind("refusals");
    let digest = stages.schema().schema().digest().to_owned();

    // Malformed: not valid canonical JSON for the schema at all.
    assert!(run_rich_turn(
        &stages,
        state(10),
        b"not json",
        Vec::new(),
        10_000,
        &AgentCancellation::new()
    )
    .is_err());

    // Wrong nominal root type: a document that decodes, but against a
    // different root_type_id.
    let wrong_root = format!(
        "{{\"schema\":\"semaprax.agent-interaction-value.v1\",\"root_type_id\":\"rich.type.state\",\"schema_digest\":{},\"value\":{{\"fields\":{{\"rich.field.count\":\"1\"}}}}}}\n",
        crate::diagnostic::quote_json(&digest)
    );
    assert!(run_rich_turn(
        &stages,
        state(10),
        wrong_root.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new()
    )
    .is_err());

    // Stale schema digest: a structurally correct document bound to a
    // digest this binder never derived.
    let stale = proposal_document(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        true,
        1,
    );
    assert!(run_rich_turn(
        &stages,
        state(10),
        stale.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new()
    )
    .is_err());
}

#[test]
fn cancellation_before_dispatch_refuses_before_any_decode() {
    let stages = bind("cancelled");
    let digest = stages.schema().schema().digest().to_owned();
    let document = proposal_document(&digest, true, 1);
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    assert!(run_rich_turn(
        &stages,
        state(10),
        document.as_bytes(),
        Vec::new(),
        10_000,
        &cancellation,
    )
    .is_err());
}

#[test]
fn a_display_only_rename_keeps_the_same_schema_digest_but_a_structural_edit_changes_it() {
    let renamed_path = write_temp(RENAMED_FIXTURE, "renamed");
    let renamed_schema = crate::agent_interaction_schema::compile_agent_interaction_schema(
        &renamed_path,
        "rich.type.proposal",
    )
    .unwrap();
    let stages = bind("rename-baseline");
    assert_eq!(
        renamed_schema.schema().digest(),
        stages.schema().schema().digest(),
        "a display-only rename must not change the schema digest"
    );

    let structural_path = write_temp(STRUCTURAL_FIXTURE, "structural");
    let structural_schema = crate::agent_interaction_schema::compile_agent_interaction_schema(
        &structural_path,
        "rich.type.proposal",
    )
    .unwrap();
    assert_ne!(
        structural_schema.schema().digest(),
        stages.schema().schema().digest(),
        "an added field must change the schema digest"
    );

    // A value decoded under the renamed-only module is still admitted by a
    // binding built from the original module, because the digest matches.
    let document = proposal_document(stages.schema().schema().digest(), true, 5);
    let outcome = run_rich_turn(
        &stages,
        state(0),
        document.as_bytes(),
        Vec::new(),
        10_000,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert!(matches!(outcome, RichTurnOutcome::Continue(_)));
}
