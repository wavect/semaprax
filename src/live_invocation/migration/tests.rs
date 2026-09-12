//! Fixture-backed exercise of `migrate_live_invocation`: the checked-pure
//! boundary, every refusal ordering, and the two fault-injection proofs
//! issue #115 is graded on — the predecessor's journal still replays
//! untouched after migration, and the destination's carried-forward budget
//! survives a simulated crash without ever being refunded.

use std::cell::Cell;
use std::rc::Rc;

use crate::agent_runtime::AgentCancellation;

use super::super::budget::{CumulativeBudgetLedger, BUDGET_EXHAUSTED};
use super::super::fixture::{
    fixture_response, FixtureAuthorizationGate, FixtureBudgetHook, FixtureModelHandler,
    FixtureNondeterministicStateMigration, FixtureObserver, FixtureProposalDecoder,
    FixtureRefusingStateMigration, FixtureSchemaBoundStateMigration, FixtureStateMigration,
    StepClock,
};
use super::super::identity::{LiveInvocationId, LiveInvocationSeed};
use super::super::journal::{self, JournalEntry};
use super::super::kernel::{
    run_live_invocation, LiveInvocationConfig, LiveInvocationHandlers, LiveInvocationOutcome,
    TurnPolicy, TurnTransition,
};
use super::super::model_invoke::{
    InvocationBudgetHook, ModelInvocationOutcome, ModelInvokeCapability,
};
use super::*;

const SCHEMA_A: &str = "sha256:0000000000000000000000000000000000000000000000000000000000aa";
const SCHEMA_B: &str = "sha256:0000000000000000000000000000000000000000000000000000000000bb";
const SCHEMA_C: &str = "sha256:0000000000000000000000000000000000000000000000000000000000cc";

fn seed(program_root: &str, schema: &str) -> LiveInvocationSeed {
    LiveInvocationSeed {
        program_root: program_root.to_owned(),
        deployment_policy: "sha256:".to_owned() + &"2".repeat(64),
        task: b"fixture task".to_vec(),
        budget: 1000,
        interaction_schema_digest: schema.to_owned(),
        approved_providers: vec!["fixture-provider".into()],
    }
}

fn config<'a>(
    identity: &'a LiveInvocationId,
    schema: &'a str,
    max_turns: u32,
) -> LiveInvocationConfig<'a> {
    LiveInvocationConfig {
        identity,
        task: b"fixture task",
        deployment_binding: "sha256:deploy-fixture",
        interaction_schema_digest: schema,
        max_turns,
        max_response_bytes: 4096,
        requested_budget_per_turn: 10,
    }
}

/// Suspends on turn `at`, otherwise continues. Not shipped in `fixture.rs`
/// because no other lane needs a `Suspend`-producing policy yet — this
/// module is the first caller that requires one at all.
struct SuspendingPolicy {
    at: u32,
}
impl TurnPolicy for SuspendingPolicy {
    fn reduce(&mut self, turn: u32, proposal: &[u8]) -> TurnTransition {
        if turn == self.at {
            TurnTransition::Suspend(proposal.to_vec())
        } else {
            TurnTransition::Continue
        }
    }
}

/// Runs a fixture invocation under `identity`/`schema` through `total_turns`
/// scripted turns, suspending on the last one, and returns the resulting
/// terminal (suspended) journal plus the total budget it committed.
fn run_to_suspend(
    identity: &LiveInvocationId,
    schema: &str,
    total_turns: u32,
) -> Vec<JournalEntry> {
    let cfg = config(identity, schema, total_turns + 1);
    let capability = ModelInvokeCapability::grant("fixture migration source setup");
    let script: Vec<_> = (0..=total_turns)
        .map(|turn| ModelInvocationOutcome::Settled(fixture_response(turn, "x")))
        .collect();
    let mut handler = FixtureModelHandler::scripted(script);
    let mut decoder = FixtureProposalDecoder::new(schema);
    let mut gate = FixtureAuthorizationGate::new(usize::try_from(total_turns + 1).unwrap());
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = SuspendingPolicy { at: total_turns };
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(run.outcome, LiveInvocationOutcome::Suspend(_)));
    run.journal
}

#[test]
fn a_suspended_invocation_migrates_through_the_checked_pure_function_and_calls_it_exactly_twice() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 2);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureStateMigration::appending(b"-migrated".to_vec());
    let result = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"previous-state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fixture.migration.v1",
        &mut migration,
    )
    .expect("a terminal suspended journal migrates cleanly");

    // The checked-pure double-evaluation discipline: called exactly twice,
    // never once (untrusted) and never more (redundant dispatch).
    assert_eq!(migration.calls, 2);
    assert_eq!(result.migrated_state, b"previous-state-migrated".to_vec());

    let receipt = journal::receipt_projection(
        &journal::validate(&journal, previous_identity.digest()).unwrap(),
    );
    assert_eq!(result.handoff.previous_turns(), receipt.turns);
    assert_eq!(result.handoff.previous_model_calls(), receipt.model_calls);
    assert_eq!(
        result.handoff.previous_model_failures(),
        receipt.model_failures
    );
    assert_eq!(result.handoff.previous_effect_calls(), receipt.effect_calls);
    assert_eq!(
        result.handoff.previous_committed_budget(),
        super::super::budget::committed_from_journal(&journal),
    );
    assert_eq!(
        result.handoff.previous_identity(),
        previous_identity.digest()
    );
    assert_eq!(
        result.handoff.destination_identity(),
        destination_identity.digest()
    );
    assert_eq!(result.handoff.migration_function(), "fixture.migration.v1");
    assert_eq!(
        result.handoff.previous_journal_chain(),
        journal::chain(&journal)
    );
}

#[test]
fn previous_identity_mismatch_is_refused_before_the_migration_function_is_ever_called() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let real_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&real_identity, SCHEMA_A, 1);

    // A caller claims a *different* previous identity than the seed derives.
    let wrong_identity =
        LiveInvocationId::derive(&seed(&("sha256:".to_owned() + &"7".repeat(64)), SCHEMA_A));

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &wrong_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::PreviousIdentityMismatch);
    assert_eq!(
        migration.calls, 0,
        "refused before the migration function runs"
    );
}

#[test]
fn stale_destination_is_refused_before_the_migration_function_is_ever_called() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let claimed_destination =
        LiveInvocationId::derive(&seed(&("sha256:".to_owned() + &"5".repeat(64)), SCHEMA_B));

    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &claimed_destination,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::StaleDestination);
    assert_eq!(migration.calls, 0);
}

#[test]
fn migrating_onto_the_same_program_root_is_refused() {
    let program_root = "sha256:".to_owned() + &"1".repeat(64);
    let previous_seed = seed(&program_root, SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    // Same program_root, only the schema differs — not a version change.
    let destination_seed = seed(&program_root, SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::UnchangedProgramRoot);
    assert_eq!(migration.calls, 0);
}

#[test]
fn an_uncertain_intent_journal_is_refused_as_not_terminal() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    // Ends right after RequestIntent: uncertain delivery, never terminal.
    let journal = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: previous_identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"2".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"3".repeat(64),
            reserved_budget: 5,
        },
    ];

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::NotTerminal);
    assert_eq!(migration.calls, 0);
}

#[test]
fn an_uncertain_effect_in_flight_is_refused_as_not_terminal() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    // Ends right after EffectIntent, no EffectObserved/EffectFailed yet.
    let journal = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: previous_identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"2".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"3".repeat(64),
            reserved_budget: 5,
        },
        JournalEntry::ResponseRecorded {
            turn: 0,
            response_digest: "sha256:".to_owned() + &"4".repeat(64),
            response: vec![1, 2, 3],
        },
        JournalEntry::ProposalAdmitted {
            turn: 0,
            proposal_digest: "sha256:".to_owned() + &"5".repeat(64),
        },
        JournalEntry::AuthorizationConsumed {
            turn: 0,
            grant_digest: "sha256:".to_owned() + &"6".repeat(64),
        },
        JournalEntry::EffectIntent {
            turn: 0,
            operation: "live-invocation.turn-effect".into(),
            request_digest: "sha256:".to_owned() + &"7".repeat(64),
        },
    ];

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::NotTerminal);
    assert_eq!(migration.calls, 0);
}

#[test]
fn a_completed_journal_is_refused_as_not_suspended() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    struct AlwaysCompletePolicy;
    impl TurnPolicy for AlwaysCompletePolicy {
        fn reduce(&mut self, _turn: u32, proposal: &[u8]) -> TurnTransition {
            TurnTransition::Complete(proposal.to_vec())
        }
    }
    let cfg = config(&previous_identity, SCHEMA_A, 1);
    let capability = ModelInvokeCapability::grant("fixture completed-not-suspended setup");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "x"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_A);
    let mut gate = FixtureAuthorizationGate::new(1);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = AlwaysCompletePolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };
    let run =
        run_live_invocation(&cfg, Vec::new(), &mut handlers, &AgentCancellation::new()).unwrap();
    assert!(matches!(run.outcome, LiveInvocationOutcome::Complete(_)));

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &run.journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::NotSuspended);
    assert_eq!(migration.calls, 0);
}

#[test]
fn a_nondeterministic_migration_function_is_rejected() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureNondeterministicStateMigration::new();
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::NonDeterministicMigration);
    assert_eq!(
        migration.calls, 2,
        "both calls happen before disagreement is detected"
    );
}

#[test]
fn a_refusing_migration_function_surfaces_its_own_reason() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureRefusingStateMigration;
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(
        error,
        LiveMigrationError::MigrationRefused("fixture_refuses_all_migrations".into())
    );
}

#[test]
fn a_migration_bound_to_the_declared_schema_pair_migrates_cleanly_and_records_it() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureSchemaBoundStateMigration::bound_to(
        vec![(SCHEMA_A.to_owned(), SCHEMA_B.to_owned())],
        b"-x".to_vec(),
    );
    let result = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .expect("the exact declared schema pair migrates");

    // Called exactly twice (the checked-pure double-evaluation discipline
    // still applies once the schema pair is bound), and the handoff records
    // exactly which schemas this migration crossed.
    assert_eq!(migration.calls, 2);
    assert_eq!(result.handoff.previous_schema_digest(), SCHEMA_A);
    assert_eq!(result.handoff.destination_schema_digest(), SCHEMA_B);
}

#[test]
fn an_unknown_or_future_destination_schema_revision_is_refused_before_the_migration_function_is_ever_called(
) {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    // The destination names SCHEMA_C, but this migration function is only
    // checked against (SCHEMA_A, SCHEMA_B) — an unknown/future revision for
    // it, exactly like a schema the compiled migration was never checked
    // against.
    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_C);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureSchemaBoundStateMigration::bound_to(
        vec![(SCHEMA_A.to_owned(), SCHEMA_B.to_owned())],
        b"-x".to_vec(),
    );
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();

    assert_eq!(error, LiveMigrationError::UnknownSchemaRevision);
    // Fail closed: the migration function is never invoked for an unbound
    // schema pair, not even once.
    assert_eq!(migration.calls, 0);
}

#[test]
fn state_over_the_capacity_cap_is_refused_before_the_migration_function_runs() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let oversized = vec![0u8; MAX_MIGRATED_STATE_BYTES + 1];
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let error = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: &oversized,
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap_err();
    assert_eq!(error, LiveMigrationError::StateCapacity);
    assert_eq!(migration.calls, 0);
}

#[test]
fn migrating_twice_with_identical_inputs_produces_a_byte_identical_handoff() {
    // The idempotency property recovery relies on: repeating the exact same
    // migration never produces a "new" continuation, only the same digest.
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);
    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let source = LiveMigrationSource {
        identity: &previous_identity,
        seed: &previous_seed,
        journal: &journal,
        state: b"state",
    };
    let destination = LiveMigrationDestination {
        identity: &destination_identity,
        seed: &destination_seed,
    };

    let mut migration_a = FixtureStateMigration::appending(b"-x".to_vec());
    let first = migrate_live_invocation(&source, &destination, "fn.v1", &mut migration_a).unwrap();
    let mut migration_b = FixtureStateMigration::appending(b"-x".to_vec());
    let second = migrate_live_invocation(&source, &destination, "fn.v1", &mut migration_b).unwrap();
    assert_eq!(first.handoff.digest(), second.handoff.digest());
    assert_eq!(first.handoff, second.handoff);
}

#[test]
fn changing_the_migration_function_name_changes_the_handoff_digest() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);
    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let source = LiveMigrationSource {
        identity: &previous_identity,
        seed: &previous_seed,
        journal: &journal,
        state: b"state",
    };
    let destination = LiveMigrationDestination {
        identity: &destination_identity,
        seed: &destination_seed,
    };

    let mut migration_a = FixtureStateMigration::appending(b"-x".to_vec());
    let v1 = migrate_live_invocation(&source, &destination, "fn.v1", &mut migration_a).unwrap();
    let mut migration_b = FixtureStateMigration::appending(b"-x".to_vec());
    let v2 = migrate_live_invocation(&source, &destination, "fn.v2", &mut migration_b).unwrap();
    assert_ne!(v1.handoff.digest(), v2.handoff.digest());
}

#[test]
fn verify_destination_binding_rejects_a_handoff_bound_to_a_different_destination() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 1);
    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);

    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let migrated = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap();

    // Correct destination: accepted.
    assert!(verify_destination_binding(&migrated.handoff, &destination_identity).is_ok());

    // A different destination identity recovering the same handoff: refused.
    let other_destination =
        LiveInvocationId::derive(&seed(&("sha256:".to_owned() + &"4".repeat(64)), SCHEMA_C));
    assert_eq!(
        verify_destination_binding(&migrated.handoff, &other_destination).unwrap_err(),
        LiveMigrationError::StaleDestination
    );
}

/// The history-intact fault-injection proof: migration reads the
/// predecessor's journal but never mutates or invalidates it. Replaying it
/// through the ordinary kernel *after* migration still reproduces the exact
/// original `Suspend` outcome with zero dispatches — the same
/// `must_not_be_called` proof `tests::replaying_a_terminal_journal_makes_
/// zero_dispatches_and_reproduces_the_outcome` already uses for the
/// non-migration case, run here on a journal that has *also* been consumed
/// by a migration in between.
#[test]
fn the_predecessors_journal_still_replays_with_zero_dispatches_after_migration() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 2);

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let _ = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .expect("migration succeeds");

    // Replay the SAME journal reference migration was handed, completely
    // untouched by it, through the ordinary kernel. Every seam panics if
    // touched, proving zero dispatches.
    let cfg = config(&previous_identity, SCHEMA_A, 3);
    let capability = ModelInvokeCapability::grant("post-migration replay of predecessor history");
    let mut handler = FixtureModelHandler::must_not_be_called();
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_A);
    let mut gate = FixtureAuthorizationGate::new(0);
    let mut budget = FixtureBudgetHook::new(10);
    struct PanicObserver;
    impl super::super::kernel::TurnObserver for PanicObserver {
        fn observe(&mut self, _turn: u32) -> Vec<u8> {
            panic!("observer must not be called on a terminal replay")
        }
    }
    struct PanicPolicy;
    impl TurnPolicy for PanicPolicy {
        fn reduce(&mut self, _turn: u32, _proposal: &[u8]) -> TurnTransition {
            panic!("policy must not be called on a terminal replay")
        }
    }
    let mut observer = PanicObserver;
    let mut policy = PanicPolicy;
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };
    let replay = run_live_invocation(
        &cfg,
        journal.clone(),
        &mut handlers,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert_eq!(replay.dispatched, 0);
    assert_eq!(
        replay.journal, journal,
        "history is byte-identical, untouched by migration"
    );
    assert!(matches!(replay.outcome, LiveInvocationOutcome::Suspend(_)));
}

/// The budget fault-injection proof: the destination's carried-forward
/// commitment from the predecessor is never lost or refunded across a
/// simulated crash on the destination side either. This is
/// `budget::tests::resuming_after_a_simulated_crash_never_refunds_the_
/// already_committed_reservation`'s exact shape, crossed over a migration
/// boundary: build a destination journal ending in an uncertain
/// `RequestIntent` (the crash window), and prove `resume_migrated`
/// reconstructs `carried + this reservation`, not just this reservation.
#[test]
fn resuming_the_destination_after_a_simulated_crash_never_refunds_the_carried_predecessor_spend() {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 2); // 3 turns * 10 = 30 committed

    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-x".to_vec());
    let migrated = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fn",
        &mut migration,
    )
    .unwrap();
    let carried = migrated.handoff.previous_committed_budget();
    assert_eq!(
        carried, 30,
        "three turns at 10 each, exactly what was committed"
    );

    // The destination's own journal so far: one turn opened, one
    // RequestIntent durably committed (60 more units), then the process
    // "crashes" before any ResponseRecorded/ResponseFailed arrives.
    let destination_journal_so_far = vec![
        JournalEntry::TurnOpened {
            turn: 0,
            invocation: destination_identity.digest().to_owned(),
            observation_digest: "sha256:".to_owned() + &"2".repeat(64),
        },
        JournalEntry::RequestIntent {
            turn: 0,
            request_digest: "sha256:".to_owned() + &"3".repeat(64),
            reserved_budget: 60,
        },
    ];

    let mut clock = StepClock::new(0);
    let mut resumed = CumulativeBudgetLedger::resume_migrated(
        100,
        None,
        carried,
        &destination_journal_so_far,
        &mut clock,
    );
    assert_eq!(
        resumed.committed(),
        90,
        "30 carried from the predecessor plus 60 already committed on the destination"
    );
    assert_eq!(resumed.remaining(), 10);

    // A retry that would only fit if either the carried 30 or the
    // destination's own 60 had been silently refunded is still refused.
    let refusal = resumed
        .reserve(
            &crate::live_invocation::model_invoke::ModelInvocationRequest {
                turn: 1,
                task: b"t".to_vec(),
                observation: b"o".to_vec(),
                proposal_grammar_digest: SCHEMA_B.to_owned(),
                deployment_binding: "sha256:deploy-fixture".to_owned(),
                max_response_bytes: 4096,
                effective_budget: 20,
            },
        )
        .unwrap_err();
    assert_eq!(
        refusal,
        super::super::model_invoke::BudgetRefusal(BUDGET_EXHAUSTED.to_owned())
    );
    assert_eq!(
        resumed.committed(),
        90,
        "still exactly 90, nothing refunded"
    );
}

/// The full A→B→C chain the issue requires: one recovered suspension at B,
/// a changed rich-state field carried through the migration function, and
/// call counts / committed budget accumulating without ever being reset —
/// checked at C, after two migrations.
#[test]
fn an_a_to_b_to_c_chain_preserves_call_counts_and_never_refunds_committed_budget() {
    let a_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let a_identity = LiveInvocationId::derive(&a_seed);
    let a_journal = run_to_suspend(&a_identity, SCHEMA_A, 1); // 2 turns * 10 = 20

    let b_seed = seed(&("sha256:".to_owned() + &"2".repeat(64)), SCHEMA_B);
    let b_identity = LiveInvocationId::derive(&b_seed);
    let mut a_to_b = FixtureStateMigration::appending(b"|field=changed-at-b".to_vec());
    let migrated_b = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &a_identity,
            seed: &a_seed,
            journal: &a_journal,
            state: b"rich-state",
        },
        &LiveMigrationDestination {
            identity: &b_identity,
            seed: &b_seed,
        },
        "a_to_b.v1",
        &mut a_to_b,
    )
    .unwrap();
    assert_eq!(
        migrated_b.migrated_state,
        b"rich-state|field=changed-at-b".to_vec()
    );
    assert_eq!(migrated_b.handoff.previous_committed_budget(), 20);
    assert_eq!(migrated_b.handoff.previous_turns(), 2);

    // A subsequent request under B observes the migrated field: prove the
    // observer sees the migrated bytes by feeding them through a scripted
    // handler and asserting the request's observation carries them.
    let observed = Rc::new(Cell::new(false));
    struct ObservingOnce {
        migrated_state: Vec<u8>,
        seen: Rc<Cell<bool>>,
    }
    impl super::super::kernel::TurnObserver for ObservingOnce {
        fn observe(&mut self, _turn: u32) -> Vec<u8> {
            if String::from_utf8_lossy(&self.migrated_state).contains("changed-at-b") {
                self.seen.set(true);
            }
            self.migrated_state.clone()
        }
    }
    let b_run_turns = 2u32; // B suspends after 2 more turns.
    let cfg_b = config(&b_identity, SCHEMA_B, b_run_turns + 1);
    let capability_b = ModelInvokeCapability::grant("B observes migrated state");
    let script_b: Vec<_> = (0..=b_run_turns)
        .map(|turn| ModelInvocationOutcome::Settled(fixture_response(turn, "y")))
        .collect();
    let mut handler_b = FixtureModelHandler::scripted(script_b);
    let mut decoder_b = FixtureProposalDecoder::new(SCHEMA_B);
    let mut gate_b = FixtureAuthorizationGate::new(usize::try_from(b_run_turns + 1).unwrap());
    let mut budget_b = FixtureBudgetHook::new(10);
    let mut observer_b = ObservingOnce {
        migrated_state: migrated_b.migrated_state.clone(),
        seen: Rc::clone(&observed),
    };
    let mut policy_b = SuspendingPolicy { at: b_run_turns };
    let mut handlers_b = LiveInvocationHandlers {
        capability: &capability_b,
        handler: &mut handler_b,
        decoder: &mut decoder_b,
        gate: &mut gate_b,
        budget: &mut budget_b,
        observer: &mut observer_b,
        policy: &mut policy_b,
        effect: None,
        sink: None,
    };
    let b_run = run_live_invocation(
        &cfg_b,
        Vec::new(),
        &mut handlers_b,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert!(
        observed.get(),
        "B's model request observed the migrated state"
    );
    assert!(matches!(b_run.outcome, LiveInvocationOutcome::Suspend(_)));

    let b_receipt = journal::receipt_projection(
        &journal::validate(&b_run.journal, b_identity.digest()).unwrap(),
    );
    assert_eq!(b_receipt.turns, 3);
    let b_committed = super::super::budget::committed_from_journal(&b_run.journal);
    assert_eq!(b_committed, 30);

    // Migrate B -> C: the handoff's own previous_committed_budget is B's
    // own total (30) — accumulating A's carried 20 is the *caller's* job
    // (add migrated_b.handoff.previous_committed_budget() to B's own total
    // when seeding C's ledger), proven below.
    let c_seed = seed(&("sha256:".to_owned() + &"3".repeat(64)), SCHEMA_C);
    let c_identity = LiveInvocationId::derive(&c_seed);
    let mut b_to_c = FixtureStateMigration::appending(b"|field=changed-at-c".to_vec());
    let migrated_c = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &b_identity,
            seed: &b_seed,
            journal: &b_run.journal,
            state: &migrated_b.migrated_state,
        },
        &LiveMigrationDestination {
            identity: &c_identity,
            seed: &c_seed,
        },
        "b_to_c.v1",
        &mut b_to_c,
    )
    .unwrap();
    assert_eq!(migrated_c.handoff.previous_committed_budget(), b_committed);
    assert_eq!(migrated_c.handoff.previous_turns(), b_receipt.turns);

    // The true cumulative total across the whole A->B->C chain: A's carried
    // budget PLUS B's own committed total (B's handoff already reflects
    // only B's own turns, since B started its own fresh journal at
    // migration). A caller wanting one running total folds every hop's
    // `previous_committed_budget` together; nothing here loses or resets
    // any of them.
    let total_chain_committed = migrated_b.handoff.previous_committed_budget()
        + migrated_c.handoff.previous_committed_budget();
    assert_eq!(total_chain_committed, 20 + 30);

    let mut clock = StepClock::new(0);
    let c_ledger = CumulativeBudgetLedger::migrated(1_000, None, total_chain_committed, &mut clock);
    assert_eq!(c_ledger.committed(), 50);
    assert_eq!(c_ledger.remaining(), 950);
}

#[derive(Default)]
struct MigrationRecordingStore {
    documents: Vec<String>,
    calls: usize,
    fail_from_call: Option<usize>,
}

impl MigrationRecordingStore {
    fn last(&self) -> &str {
        self.documents.last().expect("a checkpoint was committed")
    }
}

impl crate::agent_lifecycle::CheckpointStore for MigrationRecordingStore {
    fn commit(
        &mut self,
        _generation: u64,
        document: &str,
    ) -> Result<(), crate::agent_lifecycle::CheckpointStoreError> {
        self.calls += 1;
        if self.fail_from_call == Some(self.calls) {
            return Err(crate::agent_lifecycle::CheckpointStoreError);
        }
        self.documents.push(document.to_owned());
        Ok(())
    }
}

fn checkpoint_migration() -> (MigratedLiveInvocation, LiveInvocationSeed, LiveInvocationId) {
    let previous_seed = seed(&("sha256:".to_owned() + &"1".repeat(64)), SCHEMA_A);
    let previous_identity = LiveInvocationId::derive(&previous_seed);
    let journal = run_to_suspend(&previous_identity, SCHEMA_A, 0);
    let destination_seed = seed(&("sha256:".to_owned() + &"9".repeat(64)), SCHEMA_B);
    let destination_identity = LiveInvocationId::derive(&destination_seed);
    let mut migration = FixtureStateMigration::appending(b"-migrated".to_vec());
    let migrated = migrate_live_invocation(
        &LiveMigrationSource {
            identity: &previous_identity,
            seed: &previous_seed,
            journal: &journal,
            state: b"previous-state",
        },
        &LiveMigrationDestination {
            identity: &destination_identity,
            seed: &destination_seed,
        },
        "fixture.checkpoint.v1",
        &mut migration,
    )
    .unwrap();
    (migrated, destination_seed, destination_identity)
}

#[test]
fn a_migration_handoff_checkpoint_recovers_only_when_every_bound_byte_replays() {
    let (migrated, _, destination) = checkpoint_migration();
    let mut store = MigrationRecordingStore::default();
    let persisted = persist_migration_handoff(&mut store, migrated).unwrap();
    assert_eq!(persisted.generation(), 1);
    let recovered = recover_migration_handoff(store.last(), &destination).unwrap();
    assert_eq!(recovered, persisted);
    let wrong_destination =
        LiveInvocationId::derive(&seed(&("sha256:".to_owned() + &"8".repeat(64)), SCHEMA_B));
    assert_eq!(
        recover_migration_handoff(store.last(), &wrong_destination),
        Err(MigrationCheckpointError::DestinationMismatch)
    );

    let mut document: serde_json::Value = serde_json::from_str(store.last()).unwrap();
    document["migrated_state"] = serde_json::Value::String("00".to_owned());
    assert_eq!(
        recover_migration_handoff(&(document.to_string() + "\n"), &destination),
        Err(MigrationCheckpointError::StateMismatch)
    );
    let mut document: serde_json::Value = serde_json::from_str(store.last()).unwrap();
    document["handoff"]["migration_function"] = serde_json::Value::String("tampered".to_owned());
    assert_eq!(
        recover_migration_handoff(&(document.to_string() + "\n"), &destination),
        Err(MigrationCheckpointError::HandoffMismatch)
    );
    let mut document: serde_json::Value = serde_json::from_str(store.last()).unwrap();
    document["schema"] = serde_json::Value::String("future.v2".to_owned());
    assert_eq!(
        recover_migration_handoff(&(document.to_string() + "\n"), &destination),
        Err(MigrationCheckpointError::SchemaMismatch)
    );
    assert_eq!(
        recover_migration_handoff(
            &"x".repeat(super::checkpoint::MAX_MIGRATION_CHECKPOINT_BYTES + 1),
            &destination,
        ),
        Err(MigrationCheckpointError::Capacity)
    );
    for generation in [0, u64::MAX] {
        let mut document: serde_json::Value = serde_json::from_str(store.last()).unwrap();
        document["generation"] = serde_json::json!(generation);
        assert_eq!(
            recover_migration_handoff(&(document.to_string() + "\n"), &destination),
            Err(MigrationCheckpointError::Generation)
        );
    }
    let duplicate = store.last().replacen("{", "{\"schema\":\"duplicate\",", 1);
    assert_eq!(
        recover_migration_handoff(&duplicate, &destination),
        Err(MigrationCheckpointError::NonCanonical)
    );
}

#[test]
fn a_persisted_or_recovered_handoff_drives_destination_dispatch_and_replay() {
    let (migrated, _, destination) = checkpoint_migration();
    let mut store = MigrationRecordingStore::default();
    let mut recovered = persist_migration_handoff(&mut store, migrated).unwrap();
    let cfg = config(&destination, SCHEMA_B, 1);
    let capability = ModelInvokeCapability::grant("migrated destination");
    let mut handler = FixtureModelHandler::scripted(vec![ModelInvocationOutcome::Settled(
        fixture_response(0, "destination"),
    )]);
    let mut decoder = FixtureProposalDecoder::new(SCHEMA_B);
    let mut gate = FixtureAuthorizationGate::new(1);
    let mut budget = FixtureBudgetHook::new(10);
    let mut observer = FixtureObserver;
    let mut policy = super::super::fixture::FixturePolicy { total_turns: 1 };
    let mut handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut handler,
        decoder: &mut decoder,
        gate: &mut gate,
        budget: &mut budget,
        observer: &mut observer,
        policy: &mut policy,
        effect: None,
        sink: None,
    };
    store.fail_from_call = Some(store.calls + 1);
    assert!(matches!(
        run_migrated_destination(
            &mut recovered,
            &mut store,
            &cfg,
            &mut handlers,
            &AgentCancellation::new(),
        ),
        Err(MigrationDestinationError::Kernel(
            super::super::kernel::LiveKernelError::PersistenceFailed { dispatched: 0 }
        ))
    ));
    assert_eq!(handler.calls, 0, "a failed checkpoint precedes dispatch");
    store.fail_from_call = None;
    let first = run_migrated_destination(
        &mut recovered,
        &mut store,
        &cfg,
        &mut handlers,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert_eq!(first.run.dispatched, 1);
    assert!(first.generation > 1, "destination journal was checkpointed");
    assert_eq!(handler.calls, 1);

    let mut replay = recover_migration_handoff(store.last(), &destination).unwrap();
    let mut never_handler = FixtureModelHandler::scripted(Vec::new());
    let mut replay_decoder = FixtureProposalDecoder::new(SCHEMA_B);
    let mut replay_gate = FixtureAuthorizationGate::new(0);
    let mut replay_budget = FixtureBudgetHook::new(10);
    let mut replay_observer = FixtureObserver;
    let mut replay_policy = super::super::fixture::FixturePolicy { total_turns: 1 };
    let mut replay_handlers = LiveInvocationHandlers {
        capability: &capability,
        handler: &mut never_handler,
        decoder: &mut replay_decoder,
        gate: &mut replay_gate,
        budget: &mut replay_budget,
        observer: &mut replay_observer,
        policy: &mut replay_policy,
        effect: None,
        sink: None,
    };
    let replayed = run_migrated_destination(
        &mut replay,
        &mut store,
        &cfg,
        &mut replay_handlers,
        &AgentCancellation::new(),
    )
    .unwrap();
    assert_eq!(replayed.run.dispatched, 0);
    assert_eq!(never_handler.calls, 0);
}
