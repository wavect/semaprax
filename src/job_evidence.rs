//! A deterministic, replayable evidence log for one durable job's lifecycle.
//!
//! Issue #192 asks for "job evidence [that] is replayable but grants no
//! execution authority." `docs/DURABLE-JOBS-V1.md` and `src/job_fixture.rs`
//! already deliver the pure decision procedures and a Rust-only store that
//! obeys them; this module is the missing evidence format: an append-only,
//! domain-separated hash-chained record of the transitions a runner drove a
//! job through, plus a [`JobEvidenceLog::replay`] that **independently
//! recomputes** the job's final lifecycle state from those recorded inputs
//! alone, using the exact same pure functions
//! (`crate::job_fixture::decisions`, mirroring `std.jobs`) the live
//! [`crate::job_fixture::JobStore`] already obeys.
//!
//! # What this is not
//!
//! Appending an entry, computing a root digest, or a successful replay is
//! **inert data**, never an effect and never a permission. This module opens
//! no socket, spawns no thread, and cannot claim a lease, execute a handler,
//! retry an attempt, or publish anything — "a settlement or concurrency model
//! is proof data, not permission to perform a physical finalizer, spawn
//! runtime work, or publish an artifact" (`AGENTS.md`) applies exactly here.
//! A [`JobEvidenceLog`] is not copied from `src/agent_lifecycle/durable`'s
//! checkpoint machinery: an agent checkpoint retains a settled read
//! observation and a resumable program counter across a durable *run*; a job
//! evidence log instead proves one job's already-finished lifecycle replays
//! to the same state a verifier can recompute independently, which is a
//! narrower and simpler claim. Reusing that module's primitives blindly was
//! exactly what `docs/DURABLE-JOBS-V1.md` warned against; this is the
//! separate, jobs-scoped schema and authority boundary instead.
//!
//! # The `is_due` and lease-timing boundary
//!
//! [`JobEvidenceEntry::Claimed`] carries `is_due` as an **asserted** fact
//! rather than a tick and a stored `next_run_tick` the evidence log could
//! check itself: this format does not carry a clock. Replay only checks that
//! the *asserted* facts admit a legal transition per `std.jobs`'s state
//! machine, exactly the boundary an agent checkpoint's single registered
//! observation already accepts — a runner that asserts a false `is_due` (or
//! a false outcome kind) produces a log that still replays internally
//! consistently. What replay *does* catch is a claimed final state that
//! disagrees with what the recorded entries recompute to (see
//! [`JobEvidenceLog::replay`]'s `FinalStateMismatch`), and any entry that is
//! illegal given the state the prior entries already established — a
//! terminal state (`SUCCEEDED`, `PERMANENT_FAILURE`, `CANCELLED`,
//! `DEAD_LETTERED`) can never be reopened by a later entry, matching this
//! repository's sticky-failure-selection invariant.

use sha2::{Digest, Sha256};

use crate::digest_hex::LowerHex;
use crate::job_fixture::decisions;

const EVIDENCE_DOMAIN: &[u8] = b"semaprax.job-evidence.entry.v1\0";
const GENESIS_DIGEST: &str = "genesis";

/// One typed lifecycle event a runner recorded about a job it drove through
/// `JobStore`. Each variant carries exactly the inputs the paired
/// `std.jobs` decision procedure needs to recompute the resulting state;
/// none of it is a capability, a lease, or a credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobEvidenceEntry {
    /// The first entry in every log. `scheduled` selects `PENDING` (`false`)
    /// or `SCHEDULED` (`true`), mirroring `JobStore::enqueue`.
    Enqueued { scheduled: bool },
    /// Mirrors `JobStore::claim`; see the module boundary note above for why
    /// `is_due` is asserted rather than independently checked.
    Claimed { is_due: bool },
    /// Mirrors `JobStore::begin_execution`.
    BegunExecution,
    /// Mirrors `JobStore::complete`. `outcome_kind`: 0 success, 1 retryable
    /// failure, 2 permanent failure, 3 uncertain. `attempt_before` is the
    /// attempt counter immediately before this completion was recorded.
    Completed {
        outcome_kind: usize,
        attempt_before: u8,
        max_attempts: u8,
    },
    /// Mirrors `JobStore::record_connection_uncertain`.
    ConnectionUncertain,
    /// Mirrors `JobStore::reconcile_uncertain`. `decision`: 0 confirmed
    /// succeeded, 1 confirmed failed, 2 retry.
    ReconciledUncertain {
        decision: usize,
        is_idempotent_handler: bool,
        attempt: u8,
        max_attempts: u8,
    },
    /// Mirrors `JobStore::cancel`.
    Cancelled,
}

impl JobEvidenceEntry {
    fn discriminant(self) -> u8 {
        match self {
            Self::Enqueued { .. } => 0,
            Self::Claimed { .. } => 1,
            Self::BegunExecution => 2,
            Self::Completed { .. } => 3,
            Self::ConnectionUncertain => 4,
            Self::ReconciledUncertain { .. } => 5,
            Self::Cancelled => 6,
        }
    }

    /// A deterministic, fixed-layout encoding of this entry's own fields.
    /// Two entries that differ in any field encode to different bytes, so a
    /// tampered field is never absorbed into an unchanged digest (see
    /// `job_evidence::tests::tampering_a_recorded_outcome_changes_both_the_root_and_the_recomputed_state`).
    fn canonical_bytes(self) -> Vec<u8> {
        let mut bytes = vec![self.discriminant()];
        match self {
            Self::Enqueued { scheduled } => bytes.push(u8::from(scheduled)),
            Self::Claimed { is_due } => bytes.push(u8::from(is_due)),
            Self::BegunExecution => {}
            Self::Completed {
                outcome_kind,
                attempt_before,
                max_attempts,
            } => {
                bytes.extend_from_slice(&(outcome_kind as u64).to_le_bytes());
                bytes.push(attempt_before);
                bytes.push(max_attempts);
            }
            Self::ConnectionUncertain => {}
            Self::ReconciledUncertain {
                decision,
                is_idempotent_handler,
                attempt,
                max_attempts,
            } => {
                bytes.extend_from_slice(&(decision as u64).to_le_bytes());
                bytes.push(u8::from(is_idempotent_handler));
                bytes.push(attempt);
                bytes.push(max_attempts);
            }
            Self::Cancelled => {}
        }
        bytes
    }
}

/// Why [`JobEvidenceLog::replay`] refused a log. Every variant is a fail
/// -closed refusal; replay never guesses a state it did not recompute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobEvidenceError {
    /// The log has no entries at all.
    EmptyLog,
    /// `entries[entry_index]` is not a legal transition from the state the
    /// prior entries established (including a first entry that is not
    /// `Enqueued`, and any entry after a terminal state).
    IllegalTransition { entry_index: usize },
    /// Every entry replayed legally, but the state that produced disagrees
    /// with the state the log claims as final — the tamper case: some
    /// recorded input was changed without correspondingly updating the
    /// claimed outcome.
    FinalStateMismatch { recomputed: usize, claimed: usize },
}

/// An append-only, hash-chained record of one job's lifecycle, plus the
/// state its author claims that lifecycle ends at. See the module docs for
/// exactly what appending, the root digest, and a successful replay do and do
/// not prove.
#[derive(Clone, Debug, Default)]
pub struct JobEvidenceLog {
    entries: Vec<JobEvidenceEntry>,
    digests: Vec<String>,
    claimed_final_state: usize,
}

impl JobEvidenceLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            digests: Vec::new(),
            claimed_final_state: 0,
        }
    }

    /// Appends one recorded transition and the state its author observed
    /// the job land in immediately afterward (`resulting_state`, a
    /// `std.jobs.state.*` code). The new entry's digest chains from the
    /// prior one (or a fixed genesis value for the first entry), so the
    /// root after the last `append` call commits to every entry in order.
    pub fn append(&mut self, entry: JobEvidenceEntry, resulting_state: usize) {
        let previous = self
            .digests
            .last()
            .map_or(GENESIS_DIGEST, String::as_str);
        let mut hasher = Sha256::new();
        hasher.update(EVIDENCE_DOMAIN);
        hasher.update(previous.as_bytes());
        hasher.update(entry.canonical_bytes());
        let digest = format!("sha256:{:x}", LowerHex(hasher.finalize()));
        self.entries.push(entry);
        self.digests.push(digest);
        self.claimed_final_state = resulting_state;
    }

    /// The chained digest after the last appended entry — the "evidence
    /// root." `None` for an empty log.
    pub fn root(&self) -> Option<&str> {
        self.digests.last().map(String::as_str)
    }

    /// The final state the log's author claims, independent of whether
    /// [`Self::replay`] agrees.
    pub fn claimed_final_state(&self) -> usize {
        self.claimed_final_state
    }

    /// Independently recomputes the job's final lifecycle state from the
    /// recorded entries alone. Never mutates `self`, never touches a clock,
    /// a file, or a socket, and never claims, retries, or executes anything
    /// — it only checks whether the recorded history is internally legal
    /// and agrees with its own claimed outcome.
    pub fn replay(&self) -> Result<usize, JobEvidenceError> {
        let mut state: Option<usize> = None;
        for (entry_index, entry) in self.entries.iter().enumerate() {
            state = Some(
                Self::apply(state, *entry)
                    .ok_or(JobEvidenceError::IllegalTransition { entry_index })?,
            );
        }
        let recomputed = state.ok_or(JobEvidenceError::EmptyLog)?;
        if recomputed == self.claimed_final_state {
            Ok(recomputed)
        } else {
            Err(JobEvidenceError::FinalStateMismatch {
                recomputed,
                claimed: self.claimed_final_state,
            })
        }
    }

    /// One step of the replay state machine. `state` is the job's code
    /// before `entry`; `None` means "no job yet" (only `Enqueued` is legal
    /// there). Returns `None` for any transition `std.jobs`'s decision
    /// procedures do not admit, including every entry after a terminal
    /// state — terminal states are never reopened, matching this
    /// repository's sticky-failure-selection invariant.
    fn apply(state: Option<usize>, entry: JobEvidenceEntry) -> Option<usize> {
        match (entry, state) {
            (JobEvidenceEntry::Enqueued { scheduled }, None) => Some(usize::from(scheduled)),
            (JobEvidenceEntry::Claimed { is_due }, Some(current))
                if decisions::claim_is_legal(current, is_due) =>
            {
                Some(2)
            }
            (JobEvidenceEntry::BegunExecution, Some(2)) => Some(3),
            (
                JobEvidenceEntry::Completed {
                    outcome_kind,
                    attempt_before,
                    max_attempts,
                },
                Some(3),
            ) => {
                let attempt_after = if outcome_kind == 0 {
                    attempt_before
                } else {
                    attempt_before.saturating_add(1)
                };
                let next =
                    decisions::retry_next_state_after_outcome(outcome_kind, attempt_after, max_attempts);
                // `JobStore::complete` folds a fresh `RetryableFailure` (5)
                // back into `Scheduled` (1) once backoff is applied; replay
                // must agree with the live store exactly, not with the
                // decision procedure's own pre-fold output.
                Some(if next == 5 { 1 } else { next })
            }
            (JobEvidenceEntry::ConnectionUncertain, Some(2 | 3)) => Some(8),
            (
                JobEvidenceEntry::ReconciledUncertain {
                    decision,
                    is_idempotent_handler,
                    attempt,
                    max_attempts,
                },
                Some(8),
            ) => Some(match decision {
                0 => 4,
                1 => 6,
                2 if decisions::uncertain_retry_is_permitted(is_idempotent_handler, attempt, max_attempts) => {
                    0
                }
                _ => 8,
            }),
            (JobEvidenceEntry::Cancelled, Some(current)) if decisions::cancel_is_legal(current) => {
                Some(7)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database_fixture::DatabaseFixture;
    use crate::job_fixture::{EnqueueOutcome, JobStore, OutcomeKind};

    #[test]
    fn replay_agrees_with_the_live_job_store_across_a_successful_lifecycle() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"evidence-a".to_vec(), vec![1, 2, 3], None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let mut log = JobEvidenceLog::new();
        log.append(
            JobEvidenceEntry::Enqueued { scheduled: false },
            store.state_of(id).unwrap().code(),
        );
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        log.append(
            JobEvidenceEntry::Claimed { is_due: true },
            store.state_of(id).unwrap().code(),
        );
        store.begin_execution(id, lease_generation, 1).unwrap();
        log.append(JobEvidenceEntry::BegunExecution, store.state_of(id).unwrap().code());
        store
            .complete(id, lease_generation, 2, OutcomeKind::Success, 1, 100)
            .unwrap();
        log.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 0,
                attempt_before: 0,
                max_attempts: 3,
            },
            store.state_of(id).unwrap().code(),
        );

        assert_eq!(store.state_of(id).unwrap().code(), 4, "store should have reached SUCCEEDED");
        assert_eq!(log.replay(), Ok(4));
        assert_eq!(log.replay().unwrap(), store.state_of(id).unwrap().code());
    }

    #[test]
    fn replay_agrees_with_the_live_store_through_an_idempotent_uncertain_retry() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"evidence-b".to_vec(), vec![4, 5, 6], None, 3, true)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let mut log = JobEvidenceLog::new();
        log.append(
            JobEvidenceEntry::Enqueued { scheduled: false },
            store.state_of(id).unwrap().code(),
        );
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        log.append(
            JobEvidenceEntry::Claimed { is_due: true },
            store.state_of(id).unwrap().code(),
        );
        store.begin_execution(id, lease_generation, 1).unwrap();
        log.append(JobEvidenceEntry::BegunExecution, store.state_of(id).unwrap().code());
        store.record_connection_uncertain(id).unwrap();
        log.append(JobEvidenceEntry::ConnectionUncertain, store.state_of(id).unwrap().code());
        store.reconcile_uncertain(id, 2).unwrap();
        log.append(
            JobEvidenceEntry::ReconciledUncertain {
                decision: 2,
                is_idempotent_handler: true,
                attempt: 0,
                max_attempts: 3,
            },
            store.state_of(id).unwrap().code(),
        );

        assert_eq!(store.state_of(id).unwrap().code(), 0, "an idempotent handler's uncertain job retries back to PENDING");
        assert_eq!(log.replay(), Ok(0));
    }

    #[test]
    fn tampering_a_recorded_outcome_changes_both_the_root_and_the_recomputed_state() {
        let mut honest = JobEvidenceLog::new();
        honest.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        honest.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        honest.append(JobEvidenceEntry::BegunExecution, 3);
        honest.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 0,
                attempt_before: 0,
                max_attempts: 3,
            },
            4,
        );
        assert_eq!(honest.replay(), Ok(4));

        // A runner (or an attacker) claims the same SUCCEEDED outcome, but
        // the recorded completion input says the attempt was a permanent
        // failure instead. The two logs' entries are not byte-identical —
        // the whole point of this test — so their roots must diverge, and
        // replay must recompute the outcome the tampered input actually
        // implies rather than trusting the untouched `claimed_final_state`.
        let mut tampered = JobEvidenceLog::new();
        tampered.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        tampered.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        tampered.append(JobEvidenceEntry::BegunExecution, 3);
        tampered.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 2,
                attempt_before: 0,
                max_attempts: 3,
            },
            4,
        );

        assert_ne!(
            honest.root(),
            tampered.root(),
            "a changed recorded field must change the evidence root"
        );
        assert_eq!(
            tampered.replay(),
            Err(JobEvidenceError::FinalStateMismatch {
                recomputed: 6,
                claimed: 4
            })
        );
    }

    #[test]
    fn two_logs_built_from_identical_entries_produce_the_identical_root() {
        let mut first = JobEvidenceLog::new();
        let mut second = JobEvidenceLog::new();
        for log in [&mut first, &mut second] {
            log.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
            log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
            log.append(JobEvidenceEntry::BegunExecution, 3);
            log.append(
                JobEvidenceEntry::Completed {
                    outcome_kind: 0,
                    attempt_before: 0,
                    max_attempts: 3,
                },
                4,
            );
        }
        assert_eq!(first.root(), second.root());
        assert_eq!(first.replay(), second.replay());
    }

    #[test]
    fn a_log_that_does_not_start_with_enqueued_is_rejected() {
        let mut log = JobEvidenceLog::new();
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        assert_eq!(
            log.replay(),
            Err(JobEvidenceError::IllegalTransition { entry_index: 0 })
        );
    }

    #[test]
    fn an_empty_log_is_rejected_rather_than_defaulting_to_a_state() {
        let log = JobEvidenceLog::new();
        assert_eq!(log.replay(), Err(JobEvidenceError::EmptyLog));
    }

    #[test]
    fn a_terminal_state_cannot_be_reopened_by_a_later_entry() {
        let mut log = JobEvidenceLog::new();
        log.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        log.append(JobEvidenceEntry::BegunExecution, 3);
        log.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 2,
                attempt_before: 0,
                max_attempts: 3,
            },
            6,
        );
        // The job is now PERMANENT_FAILURE (6), terminal. A later attempt to
        // claim it again must be refused, not silently reopen the job.
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 6);
        assert_eq!(
            log.replay(),
            Err(JobEvidenceError::IllegalTransition { entry_index: 4 })
        );
    }

    #[test]
    fn retryable_completion_below_the_ceiling_folds_to_scheduled_like_the_live_store() {
        let mut log = JobEvidenceLog::new();
        log.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        log.append(JobEvidenceEntry::BegunExecution, 3);
        log.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 1,
                attempt_before: 0,
                max_attempts: 3,
            },
            1,
        );
        assert_eq!(log.replay(), Ok(1));
    }

    #[test]
    fn retryable_completion_at_the_ceiling_dead_letters_without_folding() {
        let mut log = JobEvidenceLog::new();
        log.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        log.append(JobEvidenceEntry::BegunExecution, 3);
        log.append(
            JobEvidenceEntry::Completed {
                outcome_kind: 1,
                attempt_before: 2,
                max_attempts: 3,
            },
            9,
        );
        assert_eq!(log.replay(), Ok(9));
    }

    #[test]
    fn uncertain_reconcile_refuses_an_automatic_retry_for_a_non_idempotent_handler() {
        let mut log = JobEvidenceLog::new();
        log.append(JobEvidenceEntry::Enqueued { scheduled: false }, 0);
        log.append(JobEvidenceEntry::Claimed { is_due: true }, 2);
        log.append(JobEvidenceEntry::BegunExecution, 3);
        log.append(JobEvidenceEntry::ConnectionUncertain, 8);
        // A retry decision on a non-idempotent handler stays UNCERTAIN.
        log.append(
            JobEvidenceEntry::ReconciledUncertain {
                decision: 2,
                is_idempotent_handler: false,
                attempt: 0,
                max_attempts: 3,
            },
            8,
        );
        assert_eq!(log.replay(), Ok(8));
    }
}
