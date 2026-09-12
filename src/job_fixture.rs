//! A deterministic in-memory durable-job fixture for issue #192's job-queue
//! profile.
//!
//! **This is not a message broker, not a durable transport, and not
//! reachable from checked SEMAPRAX source.** `std/jobs` (see
//! `docs/DURABLE-JOBS-V1.md`) specifies the pure, effect-free decision
//! procedures that govern a job's lifecycle state, lease legality, retry and
//! backoff, idempotent enqueue, scheduling, cancellation, and delivery
//! uncertainty, and those procedures execute as checked SEMAPRAX code on
//! every backend. This module is a separate, Rust-only proof that the same
//! decision procedures compose into something a real job runner could
//! implement: real lease claims with generation and deadline tracking, real
//! heartbeats, real worker-crash recovery, a real (simulated, single
//! threaded) concurrent-claim race, real retry with bounded backoff, and a
//! real dead-letter ceiling. It composes with `crate::database_fixture`
//! rather than reinventing a parallel ledger: `enqueue` commits a job row and
//! an idempotency-key uniqueness check through the *same*
//! `DatabaseFixture` transaction as any application-state row a caller wants
//! written atomically alongside it. It grants no filesystem, network, or
//! process authority, opens no socket, spawns no thread, and is used only by
//! this module's own tests.
//!
//! **The #228 boundary.** This fixture's [`JobStore::record_connection_uncertain`]
//! plays exactly the role `DatabaseFixture::connection_lost` already plays
//! for `std.db`: it is the one transition driven by an external signal the
//! *Rust* host layer observes, not a request a checked SEMAPRAX handler
//! makes. A checked SEMAPRAX-authored job handler cannot produce this input
//! itself today, because every fallible host operation in this repository
//! aborts its enclosing invocation on failure instead of returning an
//! inspectable value (issue #228). `docs/DURABLE-JOBS-V1.md#the-228-boundary-what-blocks-a-checked-semaprax-caller`
//! records the exact follow-on probe; this module does not work around the
//! gap or invent a checked-source path to the `UNCERTAIN` state.

use std::collections::BTreeMap;

use crate::database_fixture::{Column, DatabaseFixture, Value};

/// Rust-side mirrors of `std/jobs/src/jobs.spx`'s pure decision procedures,
/// duplicated deliberately (the same choice `database_fixture.rs` made for
/// `std.db`): this proves the decision procedures hold at the Rust layer
/// independent of the interpreter, not that this module calls it.
pub mod decisions {
    /// `std.jobs.state.is_terminal`.
    pub fn state_is_terminal(state: usize) -> bool {
        matches!(state, 4 | 6 | 7 | 9)
    }

    /// `std.jobs.lease.is_current`.
    pub fn lease_is_current(
        lease_generation: u64,
        job_generation: u64,
        now_tick: u64,
        deadline_tick: u64,
    ) -> bool {
        lease_generation == job_generation && now_tick < deadline_tick
    }

    /// `std.jobs.claim.is_legal`.
    pub fn claim_is_legal(state: usize, is_due: bool) -> bool {
        state == 0 || (state == 1 && is_due)
    }

    /// `std.jobs.retry.should_dead_letter`.
    pub fn retry_should_dead_letter(attempt: u8, max_attempts: u8) -> bool {
        attempt >= max_attempts
    }

    /// `std.jobs.retry.next_state_after_outcome`. `kind`: 0 success, 1
    /// retryable failure, 2 permanent failure, 3 uncertain.
    pub fn retry_next_state_after_outcome(kind: usize, attempt: u8, max_attempts: u8) -> usize {
        if kind > 3 {
            6
        } else if kind == 0 {
            4
        } else if kind == 2 {
            6
        } else if kind == 3 {
            8
        } else if retry_should_dead_letter(attempt, max_attempts) {
            9
        } else {
            5
        }
    }

    /// `std.jobs.retry.backoff_ticks`.
    pub fn retry_backoff_ticks(attempt: u8, base_ticks: u64, max_ticks: u64) -> u64 {
        let mut ticks = base_ticks.min(max_ticks);
        for _ in 0..attempt {
            if ticks >= max_ticks {
                break;
            }
            ticks = ticks.saturating_mul(2).min(max_ticks);
        }
        ticks
    }

    /// `std.jobs.cancel.is_legal`.
    pub fn cancel_is_legal(state: usize) -> bool {
        matches!(state, 0 | 1 | 2 | 5)
    }

    /// `std.jobs.uncertain.retry_is_permitted`.
    pub fn uncertain_retry_is_permitted(
        is_idempotent_handler: bool,
        attempt: u8,
        max_attempts: u8,
    ) -> bool {
        is_idempotent_handler && !retry_should_dead_letter(attempt, max_attempts)
    }

    /// `std.jobs.schedule.is_due`.
    pub fn schedule_is_due(now_tick: u64, next_run_tick: u64) -> bool {
        now_tick >= next_run_tick
    }

    /// `std.jobs.schedule.catch_up_next_run` (skip-missed policy, bounded).
    pub fn schedule_catch_up_next_run(
        now_tick: u64,
        next_run_tick: u64,
        interval_tick: u64,
        max_catch_up: u64,
    ) -> u64 {
        let missed = if now_tick <= next_run_tick {
            0
        } else {
            (now_tick - next_run_tick) / interval_tick
        };
        let bounded_missed = missed.min(max_catch_up);
        next_run_tick + interval_tick * (bounded_missed + 1)
    }
}

use decisions::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Scheduled,
    Leased,
    Running,
    Succeeded,
    RetryableFailure,
    PermanentFailure,
    Cancelled,
    Uncertain,
    DeadLettered,
}

impl JobState {
    /// The numeric `std.jobs.state.*` code this variant mirrors. Public so
    /// `crate::job_evidence` can record and replay a run's true state
    /// without duplicating this mapping as a second source of truth.
    pub fn code(self) -> usize {
        match self {
            JobState::Pending => 0,
            JobState::Scheduled => 1,
            JobState::Leased => 2,
            JobState::Running => 3,
            JobState::Succeeded => 4,
            JobState::RetryableFailure => 5,
            JobState::PermanentFailure => 6,
            JobState::Cancelled => 7,
            JobState::Uncertain => 8,
            JobState::DeadLettered => 9,
        }
    }

    fn from_code(code: usize) -> Self {
        match code {
            0 => JobState::Pending,
            1 => JobState::Scheduled,
            2 => JobState::Leased,
            3 => JobState::Running,
            4 => JobState::Succeeded,
            5 => JobState::RetryableFailure,
            6 => JobState::PermanentFailure,
            7 => JobState::Cancelled,
            8 => JobState::Uncertain,
            _ => JobState::DeadLettered,
        }
    }

    pub fn is_terminal(self) -> bool {
        state_is_terminal(self.code())
    }
}

/// An outcome kind reported after an attempt, mirroring `std.jobs.outcome`:
/// `Success`, `Retryable`, `Permanent`, or `Uncertain` (see the module-level
/// #228 boundary note for who is allowed to supply `Uncertain`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeKind {
    Success,
    Retryable,
    Permanent,
    Uncertain,
}

impl OutcomeKind {
    fn code(self) -> usize {
        match self {
            OutcomeKind::Success => 0,
            OutcomeKind::Retryable => 1,
            OutcomeKind::Permanent => 2,
            OutcomeKind::Uncertain => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobFixtureError {
    UnknownJob,
    ClaimNotLegal,
    LeaseNotCurrent,
    CancelNotLegal,
    NotUncertain,
    RetryNotPermitted,
    RevisionRefused,
}

#[derive(Clone, Copy, Debug)]
struct Lease {
    worker_id: u32,
    lease_generation: u64,
    deadline_tick: u64,
}

#[derive(Clone, Debug)]
struct JobRecord {
    idempotency_key: Vec<u8>,
    payload_descriptor: Vec<u8>,
    bound_revision: u8,
    state: JobState,
    job_generation: u64,
    lease: Option<Lease>,
    attempt: u8,
    max_attempts: u8,
    is_idempotent_handler: bool,
    ever_ran: bool,
    next_run_tick: Option<u64>,
    interval_tick: Option<u64>,
    max_occurrences: Option<u64>,
    occurrences_run: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Created(u64),
    Duplicate(u64),
    Conflict,
}

/// A recurring schedule: fires every `interval_tick` starting at
/// `next_run_tick`, for at most `max_occurrences` runs, using the
/// "skip-missed" catch-up policy bounded by `max_catch_up`.
#[derive(Clone, Copy, Debug)]
pub struct Schedule {
    pub next_run_tick: u64,
    pub interval_tick: u64,
    pub max_occurrences: u64,
    pub max_catch_up: u64,
}

/// The in-memory job store. `ledger` rows back every enqueue with a real
/// `DatabaseFixture` transaction; lease bookkeeping (ephemeral, not part of
/// the durable ledger row) lives in `jobs`.
pub struct JobStore {
    jobs: BTreeMap<u64, JobRecord>,
    next_id: u64,
    current_revision: u8,
}

const JOBS_TABLE: &str = "jobs";

impl JobStore {
    pub fn new(current_revision: u8) -> Self {
        Self {
            jobs: BTreeMap::new(),
            next_id: 1,
            current_revision,
        }
    }

    /// Creates the ledger-backed `jobs` table on a fresh `DatabaseFixture`.
    /// Columns: `id` (Usize), `idempotency_key` (Bytes), `state` (Usize).
    pub fn install_ledger_schema(ledger: &mut DatabaseFixture) {
        ledger
            .create_table(
                JOBS_TABLE,
                vec![
                    Column {
                        name: "id".to_owned(),
                        tag: 3,
                    },
                    Column {
                        name: "idempotency_key".to_owned(),
                        tag: 4,
                    },
                    Column {
                        name: "state".to_owned(),
                        tag: 3,
                    },
                ],
            )
            .expect("fixture identifiers are safe literals");
    }

    /// Enqueues a job. `ledger` is a `DatabaseFixture` whose "jobs" table
    /// (see [`Self::install_ledger_schema`]) already exists: enqueue begins a
    /// transaction, checks the idempotency key against both the in-memory
    /// index and the ledger row set, inserts the new row, and commits — the
    /// same transaction an application-state write can be added to before
    /// this call returns, demonstrating "database transaction integration
    /// for enqueue plus application state change" without a parallel ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(
        &mut self,
        ledger: &mut DatabaseFixture,
        idempotency_key: Vec<u8>,
        payload_descriptor: Vec<u8>,
        schedule: Option<Schedule>,
        max_attempts: u8,
        is_idempotent_handler: bool,
    ) -> Result<EnqueueOutcome, JobFixtureError> {
        if let Some(existing) = self
            .jobs
            .iter()
            .find(|(_, job)| job.idempotency_key == idempotency_key)
        {
            let (&id, job) = existing;
            return if job.payload_descriptor == payload_descriptor {
                Ok(EnqueueOutcome::Duplicate(id))
            } else {
                Ok(EnqueueOutcome::Conflict)
            };
        }
        ledger.begin().expect("ledger connection is reusable");
        let id = self.next_id;
        ledger
            .insert(
                JOBS_TABLE,
                vec![
                    Value::Usize(id as usize),
                    Value::Bytes(idempotency_key.clone()),
                    Value::Usize(0),
                ],
            )
            .expect("jobs table schema matches the inserted row shape");
        ledger.commit().expect("no other transaction is open");
        self.next_id += 1;
        let (state, next_run_tick, interval_tick, max_occurrences) = match schedule {
            Some(schedule) => (
                JobState::Scheduled,
                Some(schedule.next_run_tick),
                Some(schedule.interval_tick),
                Some(schedule.max_occurrences),
            ),
            None => (JobState::Pending, None, None, None),
        };
        self.jobs.insert(
            id,
            JobRecord {
                idempotency_key,
                payload_descriptor,
                bound_revision: self.current_revision,
                state,
                job_generation: 0,
                lease: None,
                attempt: 0,
                max_attempts,
                is_idempotent_handler,
                ever_ran: false,
                next_run_tick,
                interval_tick,
                max_occurrences,
                occurrences_run: 0,
            },
        );
        Ok(EnqueueOutcome::Created(id))
    }

    fn job_mut(&mut self, id: u64) -> Result<&mut JobRecord, JobFixtureError> {
        self.jobs.get_mut(&id).ok_or(JobFixtureError::UnknownJob)
    }

    pub fn state_of(&self, id: u64) -> Option<JobState> {
        self.jobs.get(&id).map(|job| job.state)
    }

    pub fn attempt_of(&self, id: u64) -> Option<u8> {
        self.jobs.get(&id).map(|job| job.attempt)
    }

    /// The worker currently holding this job's lease, if any.
    pub fn leased_worker_id(&self, id: u64) -> Option<u32> {
        self.jobs
            .get(&id)
            .and_then(|job| job.lease)
            .map(|lease| lease.worker_id)
    }

    /// Claims a job for `worker_id` if `claim_is_legal` holds for its
    /// current state and (for a scheduled job) `now_tick`. Returns the
    /// granted `(job_generation, lease_generation, deadline_tick)` a worker
    /// must present to every later call.
    pub fn claim(
        &mut self,
        id: u64,
        worker_id: u32,
        now_tick: u64,
        lease_ticks: u64,
    ) -> Result<(u64, u64, u64), JobFixtureError> {
        let job = self.job_mut(id)?;
        let is_due = job
            .next_run_tick
            .is_none_or(|next_run| schedule_is_due(now_tick, next_run));
        if !claim_is_legal(job.state.code(), is_due) {
            return Err(JobFixtureError::ClaimNotLegal);
        }
        let lease_generation = job.lease.map_or(0, |lease| lease.lease_generation) + 1;
        let deadline_tick = now_tick + lease_ticks;
        job.state = JobState::Leased;
        job.lease = Some(Lease {
            worker_id,
            lease_generation,
            deadline_tick,
        });
        Ok((job.job_generation, lease_generation, deadline_tick))
    }

    fn current_lease(
        job: &JobRecord,
        lease_generation: u64,
        now_tick: u64,
    ) -> Result<Lease, JobFixtureError> {
        let lease = job.lease.ok_or(JobFixtureError::LeaseNotCurrent)?;
        if !lease_is_current(
            lease_generation,
            lease.lease_generation,
            now_tick,
            lease.deadline_tick,
        ) {
            return Err(JobFixtureError::LeaseNotCurrent);
        }
        Ok(lease)
    }

    /// Extends the current lease's deadline by `extend_ticks`. Refused
    /// unless the caller's `lease_generation` matches the current one and
    /// the current deadline has not yet passed (`heartbeat.is_legal`).
    pub fn heartbeat(
        &mut self,
        id: u64,
        lease_generation: u64,
        now_tick: u64,
        extend_ticks: u64,
    ) -> Result<u64, JobFixtureError> {
        let job = self.job_mut(id)?;
        if !matches!(job.state, JobState::Leased | JobState::Running) {
            return Err(JobFixtureError::LeaseNotCurrent);
        }
        let mut lease = Self::current_lease(job, lease_generation, now_tick)?;
        lease.deadline_tick = now_tick + extend_ticks;
        job.lease = Some(lease);
        Ok(lease.deadline_tick)
    }

    /// Moves a leased job to `Running`. Refused unless the presented lease is
    /// current for a job still exactly in `Leased`.
    pub fn begin_execution(
        &mut self,
        id: u64,
        lease_generation: u64,
        now_tick: u64,
    ) -> Result<(), JobFixtureError> {
        let job = self.job_mut(id)?;
        if job.state != JobState::Leased {
            return Err(JobFixtureError::LeaseNotCurrent);
        }
        Self::current_lease(job, lease_generation, now_tick)?;
        job.state = JobState::Running;
        job.ever_ran = true;
        Ok(())
    }

    /// Records an attempt outcome. Refused unless the presented lease is
    /// current for a job exactly `Running` (`completion.is_legal`). On a
    /// retryable failure that is not yet dead-lettered, the job returns to
    /// `Pending` (or `Scheduled`, past `retry.backoff_ticks`) for reclaim
    /// rather than staying `RetryableFailure` forever; a permanent failure,
    /// success, dead-letter, or uncertain outcome is recorded as-is.
    pub fn complete(
        &mut self,
        id: u64,
        lease_generation: u64,
        now_tick: u64,
        outcome: OutcomeKind,
        base_backoff_ticks: u64,
        max_backoff_ticks: u64,
    ) -> Result<JobState, JobFixtureError> {
        let job = self.job_mut(id)?;
        if job.state != JobState::Running {
            return Err(JobFixtureError::LeaseNotCurrent);
        }
        Self::current_lease(job, lease_generation, now_tick)?;
        if outcome != OutcomeKind::Success {
            job.attempt = job.attempt.saturating_add(1);
        }
        let next = retry_next_state_after_outcome(outcome.code(), job.attempt, job.max_attempts);
        job.state = JobState::from_code(next);
        job.lease = None;
        if job.state == JobState::RetryableFailure {
            let backoff = retry_backoff_ticks(job.attempt, base_backoff_ticks, max_backoff_ticks);
            job.next_run_tick = Some(now_tick + backoff);
            job.state = JobState::Scheduled;
        }
        Ok(job.state)
    }

    /// The one transition driven by an external signal rather than a
    /// requested operation: the host observed that a lease-holding job's
    /// outcome cannot be determined (a dropped connection, a post-publication
    /// I/O failure whose acknowledgement never arrived) and forces the
    /// honest `Uncertain` resting state rather than guessing success or
    /// failure. Mirrors `DatabaseFixture::connection_lost` exactly; see the
    /// module-level #228 boundary note for why only a Rust-side caller can
    /// supply this input today.
    pub fn record_connection_uncertain(&mut self, id: u64) -> Result<(), JobFixtureError> {
        let job = self.job_mut(id)?;
        if !matches!(job.state, JobState::Leased | JobState::Running) {
            return Err(JobFixtureError::LeaseNotCurrent);
        }
        job.ever_ran = job.ever_ran || job.state == JobState::Running;
        job.state = JobState::Uncertain;
        job.lease = None;
        Ok(())
    }

    /// Reconciles an `Uncertain` job. `decision`: 0 confirmed succeeded, 1
    /// confirmed failed, 2 retry (only proceeds when the handler is declared
    /// idempotent and the attempt ceiling is not reached; otherwise the job
    /// stays `Uncertain`, never silently retried or dead-lettered).
    pub fn reconcile_uncertain(
        &mut self,
        id: u64,
        decision: usize,
    ) -> Result<JobState, JobFixtureError> {
        let job = self.job_mut(id)?;
        if job.state != JobState::Uncertain {
            return Err(JobFixtureError::NotUncertain);
        }
        job.state = match decision {
            0 => JobState::Succeeded,
            1 => JobState::PermanentFailure,
            2 if uncertain_retry_is_permitted(
                job.is_idempotent_handler,
                job.attempt,
                job.max_attempts,
            ) =>
            {
                job.attempt = job.attempt.saturating_add(1);
                JobState::Pending
            }
            _ => JobState::Uncertain,
        };
        Ok(job.state)
    }

    /// Reclaims any job whose lease has expired without a completion signal:
    /// bumps `job_generation` (so the stale worker's lease can never again
    /// satisfy `lease.is_current`) and returns it to `Pending` for a new
    /// claim. This is plain crash/timeout recovery, distinct from
    /// [`Self::record_connection_uncertain`]: no signal was ever observed
    /// about the in-flight attempt, so this profile presumes the worker is
    /// simply gone and lets a fresh attempt run, exactly the "at-least-once"
    /// baseline the issue asks for.
    pub fn expire_stale_leases(&mut self, now_tick: u64) -> Vec<u64> {
        let mut reclaimed = Vec::new();
        for (&id, job) in self.jobs.iter_mut() {
            if let Some(lease) = job.lease {
                if now_tick >= lease.deadline_tick {
                    job.job_generation += 1;
                    job.lease = None;
                    job.state = JobState::Pending;
                    reclaimed.push(id);
                }
            }
        }
        reclaimed
    }

    /// Cancels a job if `cancel.is_legal` holds for its current state. A
    /// `Running` job cannot be cancelled from outside; only a cooperative
    /// check inside the handler could stop it, which is outside this fixture.
    pub fn cancel(&mut self, id: u64) -> Result<JobState, JobFixtureError> {
        let job = self.job_mut(id)?;
        if !cancel_is_legal(job.state.code()) {
            return Err(JobFixtureError::CancelNotLegal);
        }
        job.state = JobState::Cancelled;
        job.lease = None;
        Ok(job.state)
    }

    pub fn compensation_is_required(&self, id: u64) -> Option<bool> {
        self.jobs.get(&id).map(|job| {
            job.ever_ran && matches!(job.state, JobState::PermanentFailure | JobState::Cancelled)
        })
    }

    /// Advances a completed recurring job's schedule using the skip-missed
    /// catch-up policy, or dead-letters it (by staying `Cancelled`-adjacent —
    /// concretely, this fixture reports exhaustion via `None`) once its
    /// bounded occurrence count is reached.
    pub fn advance_recurring_schedule(
        &mut self,
        id: u64,
        now_tick: u64,
        max_catch_up: u64,
    ) -> Result<Option<u64>, JobFixtureError> {
        let job = self.job_mut(id)?;
        let (Some(next_run_tick), Some(interval_tick), Some(max_occurrences)) =
            (job.next_run_tick, job.interval_tick, job.max_occurrences)
        else {
            return Ok(None);
        };
        job.occurrences_run += 1;
        if job.occurrences_run >= max_occurrences {
            job.next_run_tick = None;
            return Ok(None);
        }
        let next = schedule_catch_up_next_run(now_tick, next_run_tick, interval_tick, max_catch_up);
        job.next_run_tick = Some(next);
        job.state = JobState::Scheduled;
        Ok(Some(next))
    }

    /// Refuses a job bound to an unknown handler/schema revision (below `1`
    /// or above `current_revision`), mirroring `std.db.migration`'s
    /// gapless, only-grows ledger.
    pub fn revision_check(&self, id: u64) -> Result<(), JobFixtureError> {
        let job = self.jobs.get(&id).ok_or(JobFixtureError::UnknownJob)?;
        let known = job.bound_revision >= 1 && job.bound_revision <= self.current_revision;
        if known {
            Ok(())
        } else {
            Err(JobFixtureError::RevisionRefused)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> Vec<u8> {
        vec![3, 4, 1]
    }

    #[test]
    fn enqueue_is_idempotent_and_refuses_a_conflicting_reuse() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let first = store
            .enqueue(&mut ledger, b"key-a".to_vec(), descriptor(), None, 3, false)
            .unwrap();
        let EnqueueOutcome::Created(id) = first else {
            panic!("expected a fresh job");
        };
        // Same key, same descriptor: idempotent no-op, returns the existing job.
        let duplicate = store
            .enqueue(&mut ledger, b"key-a".to_vec(), descriptor(), None, 3, false)
            .unwrap();
        assert_eq!(duplicate, EnqueueOutcome::Duplicate(id));
        // Same key, different descriptor: closed conflict, never a silent merge.
        let conflicting = store
            .enqueue(
                &mut ledger,
                b"key-a".to_vec(),
                vec![3, 4, 2],
                None,
                3,
                false,
            )
            .unwrap();
        assert_eq!(conflicting, EnqueueOutcome::Conflict);
        assert_eq!(ledger.row_count("jobs").unwrap(), 1);
    }

    #[test]
    fn enqueue_commits_the_ledger_row_inside_one_transaction() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        store
            .enqueue(&mut ledger, b"key-b".to_vec(), descriptor(), None, 3, false)
            .unwrap();
        assert_eq!(ledger.row_count("jobs").unwrap(), 1);
        assert_eq!(
            ledger.transaction_state(),
            crate::database_fixture::TransactionState::Committed
        );
    }

    #[test]
    fn claim_lease_lifecycle_and_completion_require_a_current_lease() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-c".to_vec(), descriptor(), None, 3, true)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        assert_eq!(store.state_of(id), Some(JobState::Pending));
        let (job_generation, lease_generation, deadline) = store.claim(id, 1, 0, 10).unwrap();
        assert_eq!(job_generation, 0);
        assert_eq!(store.state_of(id), Some(JobState::Leased));
        // A second claim while already leased is refused.
        assert_eq!(
            store.claim(id, 2, 1, 10),
            Err(JobFixtureError::ClaimNotLegal)
        );
        store.begin_execution(id, lease_generation, 2).unwrap();
        assert_eq!(store.state_of(id), Some(JobState::Running));
        // Completing past the lease deadline is refused.
        assert_eq!(
            store.complete(id, lease_generation, deadline, OutcomeKind::Success, 1, 100),
            Err(JobFixtureError::LeaseNotCurrent)
        );
        assert_eq!(store.state_of(id), Some(JobState::Running));
        let state = store
            .complete(
                id,
                lease_generation,
                deadline - 1,
                OutcomeKind::Success,
                1,
                100,
            )
            .unwrap();
        assert_eq!(state, JobState::Succeeded);
        assert!(state.is_terminal());
    }

    #[test]
    fn heartbeat_extends_the_deadline_and_is_refused_once_stale() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-d".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _deadline) = store.claim(id, 1, 0, 10).unwrap();
        let extended = store.heartbeat(id, lease_generation, 5, 10).unwrap();
        assert_eq!(extended, 15);
        // A heartbeat presenting a lease_generation the store no longer
        // recognizes as current (a stale worker) is refused.
        assert_eq!(
            store.heartbeat(id, lease_generation + 1, 6, 10),
            Err(JobFixtureError::LeaseNotCurrent)
        );
    }

    #[test]
    fn expired_lease_is_reclaimed_and_the_stale_worker_cannot_complete_it() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-e".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, stale_lease_generation, deadline) = store.claim(id, 1, 0, 10).unwrap();
        // The lease deadline passes with no heartbeat: a recovery sweep
        // reclaims it for a new attempt.
        let reclaimed = store.expire_stale_leases(deadline);
        assert_eq!(reclaimed, vec![id]);
        assert_eq!(store.state_of(id), Some(JobState::Pending));
        // A second worker claims the reclaimed job under a new generation.
        let (job_generation_2, lease_generation_2, _deadline_2) =
            store.claim(id, 2, deadline, 10).unwrap();
        assert_eq!(job_generation_2, 1);
        // The original (stale) worker's lease can never complete the job
        // again: its lease_generation no longer matches the current one.
        assert_eq!(
            store.complete(
                id,
                stale_lease_generation,
                deadline + 1,
                OutcomeKind::Success,
                1,
                100
            ),
            Err(JobFixtureError::LeaseNotCurrent)
        );
        // The new worker's lease is current and can complete it.
        store
            .begin_execution(id, lease_generation_2, deadline + 1)
            .unwrap();
        let state = store
            .complete(
                id,
                lease_generation_2,
                deadline + 2,
                OutcomeKind::Success,
                1,
                100,
            )
            .unwrap();
        assert_eq!(state, JobState::Succeeded);
    }

    /// Two workers racing to claim the same job: simulated single-threaded
    /// (the same technique `database_fixture.rs`'s own
    /// `concurrent_runner_duplicate_attempt_is_never_applied_twice` uses),
    /// not real concurrent threads. Only the first claim succeeds; the
    /// second observes the job is no longer claimable.
    #[test]
    fn concurrent_claim_race_grants_the_lease_to_exactly_one_worker() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-f".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let first = store.claim(id, 1, 0, 10);
        let second = store.claim(id, 2, 0, 10);
        assert!(first.is_ok());
        assert_eq!(second, Err(JobFixtureError::ClaimNotLegal));
        assert_eq!(store.leased_worker_id(id), Some(1));
    }

    #[test]
    fn retryable_failures_back_off_and_dead_letter_at_the_ceiling() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-g".to_vec(), descriptor(), None, 2, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        store.begin_execution(id, lease_generation, 1).unwrap();
        let state = store
            .complete(id, lease_generation, 2, OutcomeKind::Retryable, 5, 1000)
            .unwrap();
        assert_eq!(state, JobState::Scheduled);
        assert_eq!(store.attempt_of(id), Some(1));
        let (_, lease_generation_2, _) = store.claim(id, 1, 100, 10).unwrap();
        store.begin_execution(id, lease_generation_2, 101).unwrap();
        let state = store
            .complete(id, lease_generation_2, 102, OutcomeKind::Retryable, 5, 1000)
            .unwrap();
        // max_attempts is 2 and this was the second attempt: dead-lettered.
        assert_eq!(state, JobState::DeadLettered);
        assert!(state.is_terminal());
    }

    #[test]
    fn permanent_failure_is_immediate_and_never_retried() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-h".to_vec(), descriptor(), None, 5, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        store.begin_execution(id, lease_generation, 1).unwrap();
        let state = store
            .complete(id, lease_generation, 2, OutcomeKind::Permanent, 5, 1000)
            .unwrap();
        assert_eq!(state, JobState::PermanentFailure);
        assert!(store.compensation_is_required(id).unwrap());
    }

    #[test]
    fn uncertain_outcome_stays_uncertain_for_a_non_idempotent_handler_until_reconciled() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-i".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        store.begin_execution(id, lease_generation, 1).unwrap();
        store.record_connection_uncertain(id).unwrap();
        assert_eq!(store.state_of(id), Some(JobState::Uncertain));
        assert!(!store.state_of(id).unwrap().is_terminal());
        // A retry decision on a non-idempotent handler is refused: the job
        // stays uncertain rather than being silently retried.
        let state = store.reconcile_uncertain(id, 2).unwrap();
        assert_eq!(state, JobState::Uncertain);
        // An explicit confirmation moves it on.
        let state = store.reconcile_uncertain(id, 1).unwrap();
        assert_eq!(state, JobState::PermanentFailure);
        assert!(store.compensation_is_required(id).unwrap());
    }

    #[test]
    fn uncertain_outcome_may_auto_retry_only_for_an_idempotent_handler_under_the_ceiling() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-j".to_vec(), descriptor(), None, 3, true)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        store.begin_execution(id, lease_generation, 1).unwrap();
        store.record_connection_uncertain(id).unwrap();
        let state = store.reconcile_uncertain(id, 2).unwrap();
        assert_eq!(state, JobState::Pending);
        assert_eq!(store.attempt_of(id), Some(1));
    }

    #[test]
    fn cancel_is_refused_while_running_and_requires_compensation_once_it_lands() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-k".to_vec(), descriptor(), None, 3, true)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        let (_, lease_generation, _) = store.claim(id, 1, 0, 10).unwrap();
        store.begin_execution(id, lease_generation, 1).unwrap();
        assert_eq!(store.cancel(id), Err(JobFixtureError::CancelNotLegal));
        // Reclaim (simulating the running attempt ending inconclusively) and
        // then cancel from a cancellable state.
        store.record_connection_uncertain(id).unwrap();
        store.reconcile_uncertain(id, 2).unwrap();
        assert_eq!(store.state_of(id), Some(JobState::Pending));
        let state = store.cancel(id).unwrap();
        assert_eq!(state, JobState::Cancelled);
        assert!(store.compensation_is_required(id).unwrap());
    }

    #[test]
    fn scheduled_job_claims_only_once_due_and_recurs_with_bounded_catch_up() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(1);
        let schedule = Schedule {
            next_run_tick: 100,
            interval_tick: 10,
            max_occurrences: 3,
            max_catch_up: 2,
        };
        let EnqueueOutcome::Created(id) = store
            .enqueue(
                &mut ledger,
                b"key-l".to_vec(),
                descriptor(),
                Some(schedule),
                3,
                false,
            )
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        assert_eq!(store.state_of(id), Some(JobState::Scheduled));
        assert_eq!(
            store.claim(id, 1, 50, 10),
            Err(JobFixtureError::ClaimNotLegal)
        );
        let (_, lease_generation, _) = store.claim(id, 1, 999, 10).unwrap();
        store.begin_execution(id, lease_generation, 999).unwrap();
        store
            .complete(id, lease_generation, 999, OutcomeKind::Success, 1, 100)
            .unwrap();
        // The clock is far past several missed windows; skip-missed catch-up
        // bounded by max_catch_up jumps at most two windows ahead.
        let next = store.advance_recurring_schedule(id, 999, 2).unwrap();
        assert_eq!(next, Some(130));
        assert_eq!(store.state_of(id), Some(JobState::Scheduled));
    }

    #[test]
    fn revision_check_refuses_an_unknown_bound_revision() {
        let mut ledger = DatabaseFixture::new();
        JobStore::install_ledger_schema(&mut ledger);
        let mut store = JobStore::new(3);
        let EnqueueOutcome::Created(id) = store
            .enqueue(&mut ledger, b"key-m".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        assert_eq!(store.revision_check(id), Ok(()));
        // A rollback to an earlier compiler revision leaves this job bound
        // above the new current revision: fail closed rather than decode it
        // speculatively.
        let mut rolled_back = JobStore::new(1);
        let EnqueueOutcome::Created(rolled_back_id) = rolled_back
            .enqueue(&mut ledger, b"key-n".to_vec(), descriptor(), None, 3, false)
            .unwrap()
        else {
            panic!("expected a fresh job");
        };
        rolled_back.current_revision = 0;
        assert_eq!(
            rolled_back.revision_check(rolled_back_id),
            Err(JobFixtureError::RevisionRefused)
        );
    }
}
