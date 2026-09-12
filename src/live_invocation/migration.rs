//! Migrating a suspended live invocation onto a new ProgramRoot, schema and
//! deployment policy (issue #115), while keeping its causal-journal history
//! and its cumulative budget intact.
//!
//! # What this module reuses, and what it cannot reach
//!
//! `execution_revision::typed_migration` (issues #109/#110's downstream
//! prerequisite) already proves the shape a real checked migration takes:
//! validate a genuine `Suspend`, bind old/new state schemas, evaluate a pure
//! migration function *twice* and reject any answer that differs between
//! the two calls, and charge every prior reservation forward rather than
//! resetting it. This module's file lease is `src/live_invocation/**`
//! only — `execution_revision`, `hir` and `interpreter` are frozen and out
//! of reach here, so [`LiveStateMigration`] is this module's own injected
//! checked-pure-function seam, in exactly the same spirit
//! `model_invoke::ModelHandler` already uses for `model.invoke`: a real
//! deployment binds it to a compiler-checked pure migration function over
//! retained HIR (the real mechanism `execution_revision::typed_migration`
//! already implements); this module ships only deterministic fixtures
//! ([`super::fixture::FixtureStateMigration`] and friends) and enforces
//! "checked pure" the same way `execution_revision::typed_migration::
//! evaluate_migration` does — by calling the bound function twice and
//! refusing to proceed if the two answers differ
//! (`LiveMigrationError::NonDeterministicMigration`).
//!
//! # What "history and budgets intact" means at this layer
//!
//! `kernel::run_live_invocation` treats an Agent's actual state as entirely
//! opaque: it never holds it, only threads bytes through the caller's own
//! `TurnObserver`/`TurnPolicy`. So "migrate state" here cannot mean
//! "rewrite the kernel's journal in place" — a live invocation's identity
//! and journal are permanently bound together (`journal::validate` rejects
//! any entry naming a different invocation), and rewriting either would
//! defeat the whole causal-journal contract. Instead:
//!
//! - **History is intact** because migration never touches the
//!   predecessor's journal at all. [`migrate_live_invocation`] takes it by
//!   shared reference, reads it, and returns; the predecessor journal
//!   remains exactly as durable and exactly as replayable afterward as
//!   before — `tests::the_predecessors_journal_still_replays_with_zero_dispatches_after_migration`
//!   proves this by replaying it through `kernel::run_live_invocation`
//!   *after* migration with a handler wired to panic if touched.
//! - **Budgets are intact** because the predecessor's total committed
//!   spend (`budget::committed_from_journal`, the same fold
//!   `CumulativeBudgetLedger::resume` already uses) is carried into the
//!   handoff and the destination's ledger is built with
//!   `CumulativeBudgetLedger::migrated`/`resume_migrated`, which start
//!   `committed` at that carried total rather than at zero. A destination
//!   ceiling is the *new* deployment's own independent policy decision — it
//!   may be smaller or larger than the predecessor's — but it can never be
//!   used to pretend the carried spend did not happen.
//!
//! # Bounded scope
//!
//! One bounded live-to-live migration, usable in an A→B→C chain by calling
//! it again with B's outputs as A's were used. No distributed multi-writer
//! transaction, no automatic cross-store reconciliation: like
//! `execution_revision::typed_migration`, this is a pure function over
//! caller-supplied, already-validated inputs. Only a journal whose terminal
//! outcome is an actual `Suspend` may be migrated
//! (`LiveMigrationError::NotSuspended`); any uncertain, mid-turn, complete
//! or failed journal is refused before the migration function is ever
//! called (`LiveMigrationError::NotTerminal`/`NotSuspended`) — in-flight or
//! uncertain work must first reach the reviewed suspend/reconciliation
//! state, exactly as issue #115 scopes it.
//!
//! # Rich schema: an unknown or future revision is refused, not adopted
//!
//! [`LiveStateMigration::known_schema_transitions`] lets a real migration
//! function declare exactly which `(previous_schema, destination_schema)`
//! pairs it is compiler-checked to interpret. When it declares a set,
//! [`migrate_live_invocation`] refuses (`LiveMigrationError::
//! UnknownSchemaRevision`) any previous/destination
//! `interaction_schema_digest` pair outside it, before `migrate` is ever
//! called — the same fail-closed rule this module already applies to a
//! stale destination or a non-suspended predecessor. This is what makes
//! "schema interpretation remains revision-specific" true here: a migration
//! bound to `(SCHEMA_A, SCHEMA_B)` never silently reinterprets state under
//! `SCHEMA_C`, no matter how similar the bytes look.

use serde_json::Value;

use crate::diagnostic::quote_json;

use super::budget::committed_from_journal;
use super::identity::{digest, LiveInvocationId, LiveInvocationSeed};
use super::journal::{self, JournalEntry};

pub mod checkpoint;

pub use checkpoint::{
    persist_migration_handoff, recover_migration_handoff, run_migrated_destination,
    MigrationCheckpointError, MigrationDestinationError, MigrationDestinationRun,
    RecoveredMigrationHandoff, PERSISTED_MIGRATION_HANDOFF_SCHEMA,
};

#[cfg(test)]
mod tests;

const HANDOFF_DOMAIN: &[u8] = b"semaprax.live-invocation.migration-handoff.v1\0";
const HANDOFF_SCHEMA: &str = "semaprax.live-invocation.migration-handoff.v1";

/// The maximum size, in bytes, of either the previous or the migrated state
/// this module will pass through a migration. Bounds allocation the same
/// way every other byte-shaped seam in this crate is capped (e.g.
/// `execution_revision::typed_migration`'s own 262 144-byte state cap,
/// which this mirrors rather than invents a new number for).
pub const MAX_MIGRATED_STATE_BYTES: usize = 262_144;

/// The checked, injected pure migration seam: given the predecessor's exact
/// state bytes, produce the destination's state bytes.
///
/// A real deployment binds this to a compiler-checked pure function over
/// retained source, the same mechanism
/// `execution_revision::typed_migration::evaluate_migration` already
/// implements against HIR (reject effects, an illegal ownership mode, an
/// incompatible result shape, or an unbound source identity, *before* ever
/// producing a value) — this module cannot call into that machinery
/// directly (it is outside this file lease), so it re-states the one
/// property it verifiably *can* enforce at this Rust-trait boundary without
/// HIR: [`migrate_live_invocation`] calls this method twice on the exact
/// same input and rejects the migration
/// (`LiveMigrationError::NonDeterministicMigration`) if the two answers
/// differ, exactly mirroring `evaluate_migration`'s own `first != second`
/// check. This module ships only deterministic fixtures
/// (`super::fixture::FixtureStateMigration` and friends); binding a real
/// compiler-checked migration function is downstream integration work
/// against this trait, the same declared boundary
/// `docs/LIVE-INVOCATION-CONTRACT-V1.md` already draws around
/// `ModelHandler`/`ProposalDecoder`/`AuthorizationGate`.
pub trait LiveStateMigration {
    fn migrate(&mut self, previous_state: &[u8]) -> Result<Vec<u8>, String>;

    /// The exact `(previous_schema_digest, destination_schema_digest)` pairs
    /// this migration function is checked to interpret — the rich-schema
    /// extension issue #115 asks for: a real compiler-checked migration
    /// function is bound against one specific source schema and one
    /// specific destination schema (the same "old/new nominal state schema"
    /// binding `execution_revision::typed_migration` already requires), not
    /// against arbitrary bytes. Returning `None` (the default) declares no
    /// restriction and is used only by fixtures that do not exercise this
    /// check; a real deployment always returns `Some` naming the schema
    /// pairs its compiled migration function actually covers, so
    /// [`migrate_live_invocation`] can refuse an unknown or future schema
    /// revision *before* ever calling [`LiveStateMigration::migrate`] rather
    /// than silently reinterpreting state it was never checked against.
    fn known_schema_transitions(&self) -> Option<&[(String, String)]> {
        None
    }
}

/// A refusal [`migrate_live_invocation`] produces before ever touching a
/// bound [`LiveStateMigration`], a `JournalSink`, or a `ModelHandler`. Every
/// variant here is checked, in the order the variants are listed, before
/// any host dispatch or store effect is attempted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveMigrationError {
    /// `previous_seed` does not derive to `previous_identity` — the caller
    /// supplied an unbound source identity (a "wrong original execution
    /// association").
    PreviousIdentityMismatch,
    /// `destination_seed` does not derive to `destination_identity` — a
    /// stale or reminted destination generation.
    StaleDestination,
    /// The predecessor and destination seeds name the same `program_root`:
    /// this is not a version change, and migrating in place would silently
    /// approve what is really the same deployment re-running `initialize`.
    UnchangedProgramRoot,
    /// The previous journal does not causally validate against
    /// `previous_identity` at all (omission, reorder, cross-invocation
    /// entry, etc. — see `journal::JournalError`).
    InvalidPreviousJournal(journal::JournalError),
    /// The previous journal is not terminal: it is uncertain (ends right
    /// after a `RequestIntent` with no response), stuck mid-turn (an
    /// unresolved decode/authorize/effect prefix — including an "uncertain
    /// effect" ending right after `EffectIntent` with no
    /// `EffectObserved`/`EffectFailed`), or otherwise still in flight. Only
    /// a journal that has already reached a recorded terminal outcome may
    /// be migrated.
    NotTerminal,
    /// The previous journal is terminal, but its case is not `suspend`. A
    /// completed or failed invocation has nothing left to migrate into a
    /// new generation; only suspended work — the reviewed
    /// suspend/reconciliation state issue #115 requires — is eligible.
    NotSuspended,
    /// `previous_state` (or the migrated result) exceeds
    /// [`MAX_MIGRATED_STATE_BYTES`].
    StateCapacity,
    /// The bound [`LiveStateMigration`] itself refused (its own closed
    /// reason text, never reinterpreted).
    MigrationRefused(String),
    /// The bound [`LiveStateMigration`] answered differently on its two
    /// calls with the exact same input — an impure or effectful migration,
    /// rejected rather than trusted.
    NonDeterministicMigration,
    /// The bound [`LiveStateMigration`] declared a restricted set of known
    /// `(previous_schema, destination_schema)` transitions
    /// ([`LiveStateMigration::known_schema_transitions`]), and the exact
    /// pair named by `previous.seed.interaction_schema_digest` and
    /// `destination.seed.interaction_schema_digest` is not one of them — an
    /// unknown or future schema revision this migration function was never
    /// checked to interpret. Refused before [`LiveStateMigration::migrate`]
    /// is ever called, the same fail-closed rule every other refusal here
    /// follows.
    UnknownSchemaRevision,
}

/// The durable, replayable record of one live-invocation migration: every
/// input that was bound and checked, and the destination's carried-forward
/// totals. Produced only by [`migrate_live_invocation`]; every field is a
/// pure function of that call's inputs, so calling it twice with identical
/// inputs produces a [`Self::digest`]-identical handoff — this is what
/// makes a repeated recovery idempotent rather than a duplicate active
/// migration continuation (see `tests::migrating_twice_with_identical_
/// inputs_produces_a_byte_identical_handoff`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveMigrationHandoff {
    previous_identity: String,
    previous_journal_chain: String,
    previous_committed_budget: i64,
    previous_turns: usize,
    previous_model_calls: usize,
    previous_model_failures: usize,
    previous_effect_calls: usize,
    destination_identity: String,
    migration_function: String,
    migrated_state_digest: String,
    previous_schema_digest: String,
    destination_schema_digest: String,
}

impl LiveMigrationHandoff {
    #[must_use]
    pub fn previous_identity(&self) -> &str {
        &self.previous_identity
    }
    #[must_use]
    pub fn previous_journal_chain(&self) -> &str {
        &self.previous_journal_chain
    }
    /// The predecessor's total nonrefundably committed budget at the moment
    /// of migration — the exact amount a destination ledger must be seeded
    /// with (`CumulativeBudgetLedger::migrated`/`resume_migrated`) so this
    /// migration cannot refund it.
    #[must_use]
    pub fn previous_committed_budget(&self) -> i64 {
        self.previous_committed_budget
    }
    #[must_use]
    pub fn previous_turns(&self) -> usize {
        self.previous_turns
    }
    #[must_use]
    pub fn previous_model_calls(&self) -> usize {
        self.previous_model_calls
    }
    #[must_use]
    pub fn previous_model_failures(&self) -> usize {
        self.previous_model_failures
    }
    #[must_use]
    pub fn previous_effect_calls(&self) -> usize {
        self.previous_effect_calls
    }
    #[must_use]
    pub fn destination_identity(&self) -> &str {
        &self.destination_identity
    }
    #[must_use]
    pub fn migration_function(&self) -> &str {
        &self.migration_function
    }
    #[must_use]
    pub fn migrated_state_digest(&self) -> &str {
        &self.migrated_state_digest
    }
    /// The predecessor's interaction schema digest at the moment of
    /// migration. Historical model responses recorded under this schema in
    /// the predecessor's journal are never reinterpreted against the
    /// destination schema — this field records exactly which schema they
    /// stay bound to.
    #[must_use]
    pub fn previous_schema_digest(&self) -> &str {
        &self.previous_schema_digest
    }
    /// The destination's interaction schema digest — the only schema a
    /// fresh request against the migrated invocation is checked against.
    #[must_use]
    pub fn destination_schema_digest(&self) -> &str {
        &self.destination_schema_digest
    }

    /// The canonical digest of this exact handoff. Two handoffs produced
    /// from byte-identical inputs are digest-identical; any differing
    /// field (including which migration function was named, or which
    /// schema pair it was bound against) changes it — the same "reminted
    /// handoff" a stale destination binding must be detectable against.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(HANDOFF_DOMAIN, self.canonical().as_bytes())
    }

    pub(super) fn canonical(&self) -> String {
        format!(
            "{{\"schema\":{},\"previous_identity\":{},\"previous_journal_chain\":{},\"previous_committed_budget\":{},\"previous_turns\":{},\"previous_model_calls\":{},\"previous_model_failures\":{},\"previous_effect_calls\":{},\"destination_identity\":{},\"migration_function\":{},\"migrated_state_digest\":{},\"previous_schema_digest\":{},\"destination_schema_digest\":{}}}",
            quote_json(HANDOFF_SCHEMA),
            quote_json(&self.previous_identity),
            quote_json(&self.previous_journal_chain),
            self.previous_committed_budget,
            self.previous_turns,
            self.previous_model_calls,
            self.previous_model_failures,
            self.previous_effect_calls,
            quote_json(&self.destination_identity),
            quote_json(&self.migration_function),
            quote_json(&self.migrated_state_digest),
            quote_json(&self.previous_schema_digest),
            quote_json(&self.destination_schema_digest),
        );
    }

    pub(super) fn decode(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        const KEYS: [&str; 13] = [
            "schema",
            "previous_identity",
            "previous_journal_chain",
            "previous_committed_budget",
            "previous_turns",
            "previous_model_calls",
            "previous_model_failures",
            "previous_effect_calls",
            "destination_identity",
            "migration_function",
            "migrated_state_digest",
            "previous_schema_digest",
            "destination_schema_digest",
        ];
        if object.len() != KEYS.len()
            || !KEYS.iter().all(|key| object.contains_key(*key))
            || object["schema"].as_str() != Some(HANDOFF_SCHEMA)
        {
            return None;
        }
        Some(Self {
            previous_identity: object["previous_identity"].as_str()?.to_owned(),
            previous_journal_chain: object["previous_journal_chain"].as_str()?.to_owned(),
            previous_committed_budget: object["previous_committed_budget"].as_i64()?,
            previous_turns: usize::try_from(object["previous_turns"].as_u64()?).ok()?,
            previous_model_calls: usize::try_from(object["previous_model_calls"].as_u64()?).ok()?,
            previous_model_failures: usize::try_from(object["previous_model_failures"].as_u64()?)
                .ok()?,
            previous_effect_calls: usize::try_from(object["previous_effect_calls"].as_u64()?)
                .ok()?,
            destination_identity: object["destination_identity"].as_str()?.to_owned(),
            migration_function: object["migration_function"].as_str()?.to_owned(),
            migrated_state_digest: object["migrated_state_digest"].as_str()?.to_owned(),
            previous_schema_digest: object["previous_schema_digest"].as_str()?.to_owned(),
            destination_schema_digest: object["destination_schema_digest"].as_str()?.to_owned(),
        })
    }
}

/// The result of one successful migration: the handoff record, and the
/// destination's migrated state bytes (the value a caller feeds into the
/// destination's own `TurnObserver`/`TurnPolicy` for its fresh turn 0 — this
/// module does not itself hold or thread that state, matching
/// `kernel::run_live_invocation`'s existing "state is entirely the caller's"
/// design).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigratedLiveInvocation {
    pub handoff: LiveMigrationHandoff,
    pub migrated_state: Vec<u8>,
}

/// Every input naming the predecessor side of one migration, grouped for
/// the same reason `kernel::LiveInvocationConfig` groups its own many
/// independent fields: [`migrate_live_invocation`] has several unrelated
/// responsibilities (identity binding, journal validation, budget folding),
/// each clearer as one named field than as one more positional argument.
pub struct LiveMigrationSource<'a> {
    pub identity: &'a LiveInvocationId,
    pub seed: &'a LiveInvocationSeed,
    pub journal: &'a [JournalEntry],
    pub state: &'a [u8],
}

/// Every input naming the destination side of one migration. See
/// [`LiveMigrationSource`] for why this is a struct rather than two more
/// positional arguments.
pub struct LiveMigrationDestination<'a> {
    pub identity: &'a LiveInvocationId,
    pub seed: &'a LiveInvocationSeed,
}

/// Migrates a suspended live invocation onto a new identity through a
/// checked pure migration, preserving journal history (by never touching
/// it) and cumulative budget (by folding the predecessor's committed total
/// into the returned handoff).
///
/// Every refusal in [`LiveMigrationError`] is checked, in this order,
/// before [`LiveStateMigration::migrate`] is ever called — so a stale
/// destination, a wrong source association, or an uncertain/non-suspended
/// predecessor fails before any host dispatch or store effect, never after
/// one. `migration.migrate` is then called exactly twice (never once, never
/// conditionally) to check determinism; a real deployment's compiler-
/// checked pure function always agrees with itself, so this never rejects
/// an honestly pure migration — only one that is not.
pub fn migrate_live_invocation(
    previous: &LiveMigrationSource<'_>,
    destination: &LiveMigrationDestination<'_>,
    migration_function: &str,
    migration: &mut dyn LiveStateMigration,
) -> Result<MigratedLiveInvocation, LiveMigrationError> {
    if LiveInvocationId::derive(previous.seed) != *previous.identity {
        return Err(LiveMigrationError::PreviousIdentityMismatch);
    }
    if LiveInvocationId::derive(destination.seed) != *destination.identity {
        return Err(LiveMigrationError::StaleDestination);
    }
    if previous.seed.program_root == destination.seed.program_root {
        return Err(LiveMigrationError::UnchangedProgramRoot);
    }
    if previous.state.len() > MAX_MIGRATED_STATE_BYTES {
        return Err(LiveMigrationError::StateCapacity);
    }

    let validated = journal::validate(previous.journal, previous.identity.digest())
        .map_err(LiveMigrationError::InvalidPreviousJournal)?;
    if !validated.terminal {
        return Err(LiveMigrationError::NotTerminal);
    }
    let receipt = journal::receipt_projection(&validated);
    if receipt.terminal_case.as_deref() != Some("suspend") {
        return Err(LiveMigrationError::NotSuspended);
    }

    // Rich-schema check: if the bound migration function declared a
    // restricted set of schema pairs it is checked to interpret, the exact
    // previous/destination schema pair named by each seed's
    // `interaction_schema_digest` must be one of them. An unknown or future
    // schema revision is refused here, before `migrate` is ever called —
    // failing closed rather than silently reinterpreting state the
    // migration function was never checked against.
    if let Some(known) = migration.known_schema_transitions() {
        let previous_schema = previous.seed.interaction_schema_digest.as_str();
        let destination_schema = destination.seed.interaction_schema_digest.as_str();
        let bound = known
            .iter()
            .any(|(p, d)| p == previous_schema && d == destination_schema);
        if !bound {
            return Err(LiveMigrationError::UnknownSchemaRevision);
        }
    }

    // Checked-pure invocation: call twice on the identical input and refuse
    // to proceed if the bound migration disagrees with itself — mirroring
    // `execution_revision::typed_migration::evaluate_migration`'s own
    // `first != second` rejection, the one property this trait boundary
    // can enforce without HIR in hand.
    let first = migration
        .migrate(previous.state)
        .map_err(LiveMigrationError::MigrationRefused)?;
    let second = migration
        .migrate(previous.state)
        .map_err(LiveMigrationError::MigrationRefused)?;
    if first != second {
        return Err(LiveMigrationError::NonDeterministicMigration);
    }
    if first.len() > MAX_MIGRATED_STATE_BYTES {
        return Err(LiveMigrationError::StateCapacity);
    }

    let handoff = LiveMigrationHandoff {
        previous_identity: previous.identity.digest().to_owned(),
        previous_journal_chain: journal::chain(previous.journal),
        previous_committed_budget: committed_from_journal(previous.journal),
        previous_turns: receipt.turns,
        previous_model_calls: receipt.model_calls,
        previous_model_failures: receipt.model_failures,
        previous_effect_calls: receipt.effect_calls,
        destination_identity: destination.identity.digest().to_owned(),
        migration_function: migration_function.to_owned(),
        migrated_state_digest: digest(HANDOFF_DOMAIN, &first),
        previous_schema_digest: previous.seed.interaction_schema_digest.clone(),
        destination_schema_digest: destination.seed.interaction_schema_digest.clone(),
    };
    Ok(MigratedLiveInvocation {
        handoff,
        migrated_state: first,
    })
}

/// Checks that a recovered [`LiveMigrationHandoff`] is actually bound to
/// `destination_identity`, before a recovering caller does anything else
/// with it (reconstruct a budget ledger, open the destination's first
/// turn, or perform any store/host effect). A handoff loaded for the wrong
/// destination — a stale generation, or one reminted for a different
/// identity — is rejected here, at zero cost, rather than only failing
/// later when its carried totals turn out to be nonsense for this
/// destination.
pub fn verify_destination_binding(
    handoff: &LiveMigrationHandoff,
    destination_identity: &LiveInvocationId,
) -> Result<(), LiveMigrationError> {
    if handoff.destination_identity != destination_identity.digest() {
        return Err(LiveMigrationError::StaleDestination);
    }
    Ok(())
}
