use super::*;
use crate::agent_lifecycle::CheckpointStoreError;
use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureBudgetHook, FixtureModelHandler,
    FixtureObserver, FixturePolicy, FixtureProposalDecoder,
};
use crate::live_invocation::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveKernelError,
};
use crate::live_invocation::model_invoke::{ModelInvocationOutcome, ModelInvokeCapability};
use crate::live_invocation::{LiveInvocationId, LiveInvocationSeed};

const SCHEMA_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn identity() -> LiveInvocationId {
    LiveInvocationId::derive(&LiveInvocationSeed {
        program_root: "sha256:root".into(),
        deployment_policy: "sha256:policy".into(),
        task: b"task".to_vec(),
        budget: 1_000,
        interaction_schema_digest: SCHEMA_DIGEST.into(),
        approved_providers: vec!["fixture".into()],
    })
}

/// A [`CheckpointStore`] that keeps every committed generation and can be
/// told to fail starting from a given call number (1-based), mirroring the
/// fault-injection technique `agent_lifecycle::durable`'s own tests use for
/// its `CrashPoint` enum, but at the granularity of "the Nth store write",
/// which is exactly the resolution this module's own persist call sites
/// need to prove "before dispatch" versus "after dispatch" failures land in
/// the right kernel state.
#[derive(Default)]
struct RecordingStore {
    documents: Vec<String>,
    calls: usize,
    fail_from_call: Option<usize>,
}

impl RecordingStore {
    fn failing_from(call: usize) -> Self {
        Self {
            fail_from_call: Some(call),
            ..Self::default()
        }
    }

    fn last(&self) -> &str {
        self.documents.last().expect("at least one commit")
    }
}

impl CheckpointStore for RecordingStore {
    fn commit(&mut self, _generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.calls += 1;
        if self.fail_from_call == Some(self.calls) {
            return Err(CheckpointStoreError);
        }
        self.documents.push(document.to_owned());
        Ok(())
    }
}

fn sample_journal() -> Vec<JournalEntry> {
    vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: identity().digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"b".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"c".repeat(64),
            reserved_budget: 10,
        },
    ]
}

#[test]
fn encode_then_recover_round_trips_the_exact_entries() {
    let id = identity();
    let entries = sample_journal();
    let document = encode_envelope(id.digest(), 1, &entries);
    let recovered = recover_journal(&document, &id).expect("well-formed, bound document");
    assert_eq!(recovered.entries, entries);
    assert_eq!(recovered.generation, 1);
}

#[test]
fn recovery_rejects_a_document_bound_to_a_different_invocation() {
    let id = identity();
    let other = LiveInvocationId::derive(&LiveInvocationSeed {
        program_root: "sha256:root".into(),
        deployment_policy: "sha256:policy".into(),
        task: b"different-task".to_vec(),
        budget: 1_000,
        interaction_schema_digest: SCHEMA_DIGEST.into(),
        approved_providers: vec!["fixture".into()],
    });
    assert_ne!(id, other, "fixture setup sanity");
    let document = encode_envelope(other.digest(), 1, &sample_journal());
    assert_eq!(
        recover_journal(&document, &id),
        Err(RecoveryError::InvocationMismatch)
    );
}

#[test]
fn recovery_rejects_an_unknown_schema_tag() {
    let id = identity();
    let document = encode_envelope(id.digest(), 1, &sample_journal())
        .replace(PERSISTED_JOURNAL_SCHEMA, "semaprax.something-else.v1");
    assert_eq!(
        recover_journal(&document, &id),
        Err(RecoveryError::SchemaMismatch)
    );
}

#[test]
fn recovery_rejects_malformed_bytes() {
    let id = identity();
    assert_eq!(
        recover_journal("not json", &id),
        Err(RecoveryError::Malformed)
    );
    assert_eq!(recover_journal("{}", &id), Err(RecoveryError::Malformed));
}

#[test]
fn recovery_rejects_a_document_whose_entries_were_tampered_with_after_writing() {
    // The document stays valid JSON with a well-formed digest-shaped string
    // in the tampered field, so `journal::decode`'s own shape check alone
    // would accept it; only the recomputed chain link catches this,
    // exactly the gap the module doc names.
    let id = identity();
    let document = encode_envelope(id.digest(), 1, &sample_journal());
    let forged_digest = "sha256:".to_owned() + &"f".repeat(64);
    let original_digest = "sha256:".to_owned() + &"c".repeat(64);
    assert!(document.contains(&original_digest));
    let tampered = document.replace(&original_digest, &forged_digest);
    assert_ne!(tampered, document);
    assert_eq!(
        recover_journal(&tampered, &id),
        Err(RecoveryError::ChainMismatch)
    );
}

#[test]
fn recovery_rejects_reordered_entries_even_though_each_entry_individually_decodes() {
    let id = identity();
    let mut reordered = sample_journal();
    reordered.reverse();
    // Build the document by hand with reordered entries but a `seq` that
    // still matches each entry's new position, so `journal::decode`'s shape
    // check (which only verifies `seq` is sequential, not that causal order
    // holds) accepts it; only the chain link this module adds catches the
    // reorder before the causally-invalid journal ever reaches the kernel.
    let rendered = journal::render(&reordered);
    let document = format!(
        "{{\"schema\":{},\"invocation\":{},\"generation\":1,\"chain\":{},\"entries\":{}}}\n",
        quote_json(PERSISTED_JOURNAL_SCHEMA),
        quote_json(id.digest()),
        // Deliberately keep the ORIGINAL (pre-reorder) chain so the
        // document's own claimed chain no longer matches its entries.
        quote_json(&journal::chain(&sample_journal())),
        rendered.trim_end(),
    );
    assert_eq!(
        recover_journal(&document, &id),
        Err(RecoveryError::ChainMismatch)
    );
}

#[test]
fn checkpoint_journal_sink_commits_an_incrementing_generation_each_call() {
    let mut store = RecordingStore::default();
    let id = identity();
    let mut sink = CheckpointJournalSink::new(&mut store, id.digest());
    sink.persist(&sample_journal()[..1]).expect("first commit");
    assert_eq!(sink.generation(), 1);
    sink.persist(&sample_journal()).expect("second commit");
    assert_eq!(sink.generation(), 2);
    assert_eq!(store.documents.len(), 2);
    let recovered = recover_journal(store.last(), &id).expect("well-formed");
    assert_eq!(recovered.generation, 2);
    assert_eq!(recovered.entries, sample_journal());
}

#[test]
fn checkpoint_journal_sink_resumes_from_a_recovered_generation() {
    let mut store = RecordingStore::default();
    let id = identity();
    {
        let mut sink = CheckpointJournalSink::new(&mut store, id.digest());
        sink.persist(&sample_journal()).expect("commit");
    }
    let recovered = recover_journal(store.last(), &id).expect("well-formed");
    let mut sink = CheckpointJournalSink::resume(&mut store, id.digest(), recovered.generation);
    sink.persist(&sample_journal()).expect("second commit");
    assert_eq!(sink.generation(), recovered.generation + 1);
}

// --- Kernel integration: the sink is called before dispatch, and a store
// --- failure at each of the two shapes named in the module docs produces
// --- exactly the behavior the docs promise. ---

fn cfg<'a>(identity: &'a LiveInvocationId) -> LiveInvocationConfig<'a> {
    LiveInvocationConfig {
        identity,
        task: b"task",
        deployment_binding: "sha256:deploy-fixture",
        interaction_schema_digest: SCHEMA_DIGEST,
        max_turns: 5,
        max_response_bytes: 4096,
        requested_budget_per_turn: 10,
    }
}

/// A store that fails on its Nth `commit` call is a fault-injection
/// technique fine-grained enough to hit exactly one of this module's named
/// persist call sites without needing a `CrashPoint`-shaped enum threaded
/// through the shared kernel.
#[test]
fn a_store_failure_before_the_first_dispatch_makes_zero_model_calls() {
    let id = identity();
    let capability = ModelInvokeCapability::grant("test");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    // Call 1 is `TurnOpened`; call 2 is `RequestIntent`, still before any
    // dispatch. Failing there must make zero calls to the handler, which
    // would panic if invoked at all.
    let mut store = RecordingStore::failing_from(2);
    let mut sink = CheckpointJournalSink::new(&mut store, id.digest());
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: Some(&mut sink),
    };
    let result = run_live_invocation(
        &cfg(&id),
        Vec::new(),
        &mut handlers,
        &AgentCancellation::new(),
    );
    assert_eq!(
        result.err(),
        Some(LiveKernelError::PersistenceFailed { dispatched: 0 }),
        "a write that fails before dispatch is an ordinary refusal with zero dispatches"
    );
    assert_eq!(handler.calls, 0);
}

#[test]
fn a_store_failure_immediately_after_the_response_still_reports_the_real_dispatch_count() {
    let id = identity();
    let capability = ModelInvokeCapability::grant("test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "a"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    // Calls: 1 = TurnOpened, 2 = RequestIntent (before dispatch), then the
    // handler is actually invoked, 3 = ResponseRecorded (after dispatch).
    // Failing on call 3 proves the dispatch already happened.
    let mut store = RecordingStore::failing_from(3);
    let mut sink = CheckpointJournalSink::new(&mut store, id.digest());
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: Some(&mut sink),
    };
    let result = run_live_invocation(
        &cfg(&id),
        Vec::new(),
        &mut handlers,
        &AgentCancellation::new(),
    );
    assert_eq!(
        result.err(),
        Some(LiveKernelError::PersistenceFailed { dispatched: 1 }),
        "the model call already happened before this write failed"
    );
    assert_eq!(
        handler.calls, 1,
        "the dispatch is real and cannot be undone"
    );
    // What the store actually kept durable is exactly the RequestIntent
    // write from before dispatch — the response never made it to storage,
    // matching a real crash at the same point.
    let recovered = recover_journal(store.last(), &id).expect("well-formed");
    assert!(matches!(
        recovered.entries.last(),
        Some(JournalEntry::RequestIntent { .. })
    ));
}

#[test]
fn a_fully_persisted_completed_run_recovers_and_replays_with_zero_dispatches() {
    let id = identity();
    let capability = ModelInvokeCapability::grant("test");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "a"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate = FixtureAuthorizationGate::new(10);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = FixturePolicy { total_turns: 1 };
    let mut store = RecordingStore::default();
    let final_document = {
        let mut sink = CheckpointJournalSink::new(&mut store, id.digest());
        let mut handlers = LiveInvocationHandlers {
            capability: &capability,
            handler: &mut handler,
            decoder: &mut decoder,
            gate: &mut gate,
            budget: &mut budget,
            observer: &mut observer,
            policy: &mut policy,
            effect: None,
            sink: Some(&mut sink),
        };
        let run = run_live_invocation(
            &cfg(&id),
            Vec::new(),
            &mut handlers,
            &AgentCancellation::new(),
        )
        .expect("completes");
        assert_eq!(run.dispatched, 1);
        store.last().to_owned()
    };

    // Simulate a fresh process: decode/verify the persisted document, then
    // replay it through a *fresh* kernel call whose handler panics if
    // touched at all.
    let recovered = recover_journal(&final_document, &id).expect("well-formed");
    let mut handler2 = FixtureModelHandler::must_not_be_called();
    let mut decoder2 = FixtureProposalDecoder::new(SCHEMA_DIGEST);
    let mut gate2 = FixtureAuthorizationGate::new(10);
    let mut budget2 = FixtureBudgetHook::new(10);
    let mut observer2 = FixtureObserver;
    let mut policy2 = FixturePolicy { total_turns: 1 };
    let mut handlers2 = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler2,
        decoder: &mut decoder2,
        gate: &mut gate2,
        budget: &mut budget2,
        observer: &mut observer2,
        policy: &mut policy2,
        effect: None,
        sink: None,
    };
    let replay = run_live_invocation(
        &cfg(&id),
        recovered.entries,
        &mut handlers2,
        &AgentCancellation::new(),
    )
    .expect("replays a terminal journal");
    assert_eq!(replay.dispatched, 0);
    assert_eq!(handler2.calls, 0);
}
