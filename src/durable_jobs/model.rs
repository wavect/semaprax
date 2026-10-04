//! Closed data model for the durable-job generation store.
//!
//! Every field here is either an explicit caller-supplied logical value (a
//! `u64` tick, never a wall clock) or a value this module derives
//! deterministically from prior state (a monotonically assigned
//! [`JobId`]). Nothing here reads `std::time::SystemTime`, an environment
//! variable, or a random source, because [`super::codec`] serializes this
//! model byte-for-byte into the durable record, and AGENTS.md requires that
//! record to be deterministic.

use std::collections::BTreeMap;

/// A durably assigned job identity. Identities are handed out in strictly
/// increasing order by [`super::store::GenerationJobStore::enqueue`], never
/// derived from a pointer, a thread id, or a timestamp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

/// Ten-ish-state lifecycle, deliberately a smaller closed set than
/// `std.jobs`'s ten codes: this module is the persistence seam ADR 0005
/// calls for, not a second implementation of the full `std.jobs` state
/// machine that `src/job_fixture.rs` and `src/job_runtime.rs` already own.
/// `Scheduled` and `Uncertain` are out of this module's scope; see
/// `docs/decisions/0005-durable-job-storage-medium.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    Pending,
    Leased,
    Running,
    Succeeded,
    RetryableFailure,
    PermanentFailure,
    Cancelled,
    DeadLettered,
}

impl JobState {
    /// A terminal state never transitions again. Mirrors `std.jobs.state
    /// .is_terminal`'s closed set, minus the two states this module does not
    /// model.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Succeeded
                | JobState::PermanentFailure
                | JobState::Cancelled
                | JobState::DeadLettered
        )
    }

    pub(crate) fn code(self) -> u8 {
        match self {
            JobState::Pending => 0,
            JobState::Leased => 1,
            JobState::Running => 2,
            JobState::Succeeded => 3,
            JobState::RetryableFailure => 4,
            JobState::PermanentFailure => 5,
            JobState::Cancelled => 6,
            JobState::DeadLettered => 7,
        }
    }

    pub(crate) fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => JobState::Pending,
            1 => JobState::Leased,
            2 => JobState::Running,
            3 => JobState::Succeeded,
            4 => JobState::RetryableFailure,
            5 => JobState::PermanentFailure,
            6 => JobState::Cancelled,
            7 => JobState::DeadLettered,
            _ => return None,
        })
    }
}

/// A bounded, pure retry policy. `next_state_after_failure` in
/// [`super::retry`] is the only place attempt counting and the ceiling are
/// consulted, so this record carries the policy, not the decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_backoff_ticks: u64,
    pub max_backoff_ticks: u64,
}

impl RetryPolicy {
    pub const fn new(max_attempts: u32, base_backoff_ticks: u64, max_backoff_ticks: u64) -> Self {
        RetryPolicy {
            max_attempts,
            base_backoff_ticks,
            max_backoff_ticks,
        }
    }
}

/// An affine lease grant. `generation` fences a reclaimed lease the same way
/// `std.jobs.lease` does: a lease minted by an earlier `claim` for this job
/// can never satisfy a legality check once a later `claim` has reclaimed it,
/// even if the two calls happen to name the same `worker_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    pub worker_id: u64,
    pub generation: u64,
    pub deadline_tick: u64,
}

/// An opaque token a caller must present back to [`super::store::JobStore
/// ::complete`]. Constructing one outside this module proves nothing: the
/// store always re-checks the live record's own lease before honoring a
/// completion, so a forged or stale token is refused, never trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseToken {
    pub job_id: JobId,
    pub worker_id: u64,
    pub generation: u64,
}

/// The reported result of one attempt. `Uncertain` delivery is explicitly
/// out of this module's scope (see the `JobState` doc comment); a handler
/// that cannot observe its own outcome is `src/job_runtime.rs`'s concern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptOutcome {
    Success,
    Retryable,
    Permanent,
}

/// One durable job record. Field order here is the field order
/// [`super::codec`] writes, so changing it changes the wire format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobRecord {
    pub id: JobId,
    pub idempotency_key: Vec<u8>,
    pub payload: Vec<u8>,
    pub state: JobState,
    pub attempt: u32,
    /// Incremented once per `claim`, never reset by a completion or an
    /// expiry-driven reclaim, so no two lease grants this job ever makes
    /// can share a `Lease::generation` — the exact fencing gap
    /// `docs/DURABLE-JOBS-V1.md` records `src/job_fixture.rs` having to fix
    /// (`JobRecord::lease_epoch` there; mirrored here independently because
    /// this module owns its own on-disk record).
    pub lease_epoch: u64,
    pub retry_policy: RetryPolicy,
    pub lease: Option<Lease>,
    pub created_at_tick: u64,
    pub last_error: Vec<u8>,
}

/// What `enqueue` requests. `idempotency_key` and `payload` are both opaque
/// bounded byte strings, the same closed-domain-descriptor idiom
/// `docs/DURABLE-JOBS-V1.md` uses for `std.jobs.idempotency`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnqueueRequest {
    pub idempotency_key: Vec<u8>,
    pub payload: Vec<u8>,
    pub retry_policy: RetryPolicy,
    pub now_tick: u64,
}

/// The three-way outcome `docs/DURABLE-JOBS-V1.md`'s idempotency table
/// specifies: fresh, duplicate (an idempotent no-op returning the existing
/// job), or conflict (closed refusal, never a silent merge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Created(JobId),
    Duplicate(JobId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStoreError {
    /// The idempotency key is already bound to a job with a different
    /// payload; enqueue refuses to guess which one is authoritative.
    IdempotencyConflict(JobId),
    /// No job is currently available to claim.
    NothingToClaim,
    /// The referenced job does not exist in this store.
    UnknownJob,
    /// The presented lease token does not match the job's current lease
    /// (wrong worker, superseded generation, an expired deadline, or the
    /// job never held one), or the job is not in the state this operation
    /// requires (e.g. completing a job that has not yet begun execution).
    /// Either way, no execution-adjacent call ever succeeds without a
    /// currently valid lease held by the caller.
    LeaseNotCurrent,
    /// The job is not in a state this operation is legal from.
    IllegalTransition,
    /// A durable write definitely failed before its destination was
    /// published. The live handle remains usable and callers may retry.
    Io,
    /// Another live `GenerationJobStore` holds this root's OS-backed
    /// exclusive writer lock. The caller must wait for that owner to close or
    /// terminate, then open a fresh handle instead of writing a stale table.
    WriterBusy,
    /// A durable generation or `ACTIVE` pointer may already be visible even
    /// though its acknowledgement failed. This handle is poisoned and must
    /// be dropped; recovery requires opening a fresh writer handle.
    PublicationUncertain,
    /// The persisted generation bytes were corrupt, truncated, or exceeded
    /// a bound. Refused rather than partially trusted.
    CorruptGeneration,
    /// The candidate table could not be encoded within `codec`'s bounds
    /// (too many jobs, or a field over its byte cap).
    RecordTooLarge,
    /// A tick arithmetic step (e.g. `now_tick + lease_ticks`) would
    /// overflow `u64`. Refused rather than wrapping into a bogus deadline.
    TickOverflow,
}

/// The full in-memory table one generation snapshot carries: every job
/// record plus the caller's auxiliary side-table, keyed and ordered by
/// `BTreeMap` so iteration order — and therefore the encoded bytes — never
/// depends on insertion order or hashing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobTable {
    pub next_job_id: u64,
    pub jobs: BTreeMap<JobId, JobRecord>,
    /// Arbitrary caller-declared application state committed atomically
    /// alongside job mutations. This is the "database transaction
    /// integration" seam: see
    /// `docs/decisions/0005-durable-job-storage-medium.md` and
    /// [`super::store::GenerationJobStore::enqueue_with_side_record`].
    pub side_records: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl Default for JobTable {
    /// Job ids are handed out starting at `1`, never `0`, so `JobId(0)` can
    /// be used elsewhere as an unambiguous "no job" sentinel if a caller
    /// ever wants one.
    fn default() -> Self {
        JobTable {
            next_job_id: 1,
            jobs: BTreeMap::new(),
            side_records: BTreeMap::new(),
        }
    }
}
