//! [`GenerationJobStore`]: the sealed `JobStore` seam, backed by this
//! repository's own atomic-generation substrate pattern rather than a SQL
//! driver — see `docs/decisions/0005-durable-job-storage-medium.md`.
//!
//! Every mutating call here follows the same shape: clone the live table,
//! apply the requested change to the clone, encode the *whole* clone
//! ([`super::codec::encode`]), and durably commit it as one new generation
//! ([`super::durable_fs::commit_bytes`], twice — once for the generation
//! file, once for the `ACTIVE` pointer that selects it) before mutating
//! `self` or returning success to the caller. A call that fails at any
//! point in that sequence leaves both the in-memory table and the on-disk
//! state exactly as they were before the call: there is no path that
//! reports success without a durable commit, and no path that partially
//! mutates the live table on failure.
//!
//! This is deliberately simple rather than fast: a whole-table rewrite per
//! mutation does not scale to a high-throughput queue, and does not try to.
//! It is the seam ADR 0005 asks for — one that a later, higher-throughput
//! medium could sit behind without changing a caller — not a production
//! scheduler.

#[cfg(test)]
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::codec;
use super::durable_fs;
use super::model::{
    AttemptOutcome, EnqueueOutcome, EnqueueRequest, JobId, JobRecord, JobState, JobStoreError,
    JobTable, Lease, LeaseToken,
};
use super::retry;

mod sealed {
    pub trait Sealed {}
}

/// The durable-job persistence seam. Sealed: only [`GenerationJobStore`] in
/// this module implements it, so a caller can be generic over "some durable
/// job store" without this crate exposing a point where an unvetted, e.g.
/// network-backed, implementation could be substituted in from outside.
pub trait JobStore: sealed::Sealed {
    fn enqueue(&mut self, request: EnqueueRequest) -> Result<EnqueueOutcome, JobStoreError>;

    /// Same as `enqueue`, but atomically commits one additional
    /// caller-declared `(side_key, side_value)` entry into the same
    /// generation. Both land together or neither does — see
    /// `store::tests::a_fault_during_the_joint_commit_leaves_neither_the_job_nor_the_side_record_visible`.
    /// This is the seam's answer to issue #192's "database transaction
    /// integration for enqueue plus application state change" criterion,
    /// read as ADR 0005 re-scopes it: atomicity against this durable store,
    /// not a SQL engine.
    fn enqueue_with_side_record(
        &mut self,
        request: EnqueueRequest,
        side_key: Vec<u8>,
        side_value: Vec<u8>,
    ) -> Result<EnqueueOutcome, JobStoreError>;

    fn claim(
        &mut self,
        worker_id: u64,
        now_tick: u64,
        lease_ticks: u64,
    ) -> Result<(JobId, LeaseToken), JobStoreError>;

    fn begin_execution(&mut self, token: LeaseToken, now_tick: u64) -> Result<(), JobStoreError>;

    fn complete(
        &mut self,
        token: LeaseToken,
        now_tick: u64,
        outcome: AttemptOutcome,
        error: Option<Vec<u8>>,
    ) -> Result<JobState, JobStoreError>;

    fn cancel(&mut self, job_id: JobId) -> Result<JobState, JobStoreError>;

    fn get(&self, job_id: JobId) -> Option<&JobRecord>;

    fn side_record(&self, key: &[u8]) -> Option<&Vec<u8>>;
}

/// A file-backed [`JobStore`]. One `GenerationJobStore` owns one directory
/// tree; nothing here reads or writes any path outside `root` (see
/// `tests::module_source_never_reaches_for_network_process_or_env_authority`
/// for an executable check of that claim across this whole module tree).
pub struct GenerationJobStore {
    generations_dir: PathBuf,
    active_path: PathBuf,
    current_generation: u64,
    table: JobTable,
    stage_seq: AtomicU64,
}

fn active_bytes_to_generation(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

impl GenerationJobStore {
    /// Open (or initialize) a store rooted at `root`. `root` is created if
    /// missing, along with a `generations` subdirectory; nothing above
    /// `root` is ever touched. If `root` already holds committed
    /// generations, the most recent one `ACTIVE` names is loaded and
    /// becomes this instance's live table — this is the recovery route a
    /// crash-durability test exercises by dropping one instance and opening
    /// a second at the same path.
    pub fn open(root: &Path) -> Result<Self, JobStoreError> {
        durable_fs::ensure_dir(root).map_err(|_| JobStoreError::Io)?;
        let generations_dir =
            durable_fs::ensure_dir(&root.join("generations")).map_err(|_| JobStoreError::Io)?;
        let active_path = root.join("ACTIVE");
        let active_bytes =
            durable_fs::read_optional(&active_path).map_err(|_| JobStoreError::Io)?;
        let (current_generation, table) = match active_bytes {
            None => (0, JobTable::default()),
            Some(bytes) => {
                let generation =
                    active_bytes_to_generation(&bytes).ok_or(JobStoreError::CorruptGeneration)?;
                let generation_bytes =
                    durable_fs::read_optional(&generations_dir.join(generation.to_string()))
                        .map_err(|_| JobStoreError::Io)?
                        .ok_or(JobStoreError::CorruptGeneration)?;
                let table =
                    codec::decode(&generation_bytes).ok_or(JobStoreError::CorruptGeneration)?;
                (generation, table)
            }
        };
        // A terminated process can leave a `create_new` stage behind.  Do
        // not remove or promote it: it may be live, foreign, or incomplete.
        // Start after every owned stage sequence observed in both directories
        // so this reopened handle cannot collide with the crashed attempt.
        let stage_seq = durable_fs::next_job_stage_sequence([root, &generations_dir])
            .map_err(|_| JobStoreError::Io)?;
        Ok(GenerationJobStore {
            generations_dir,
            active_path,
            current_generation,
            table,
            stage_seq: AtomicU64::new(stage_seq),
        })
    }

    fn next_stage_name(&self, label: &str) -> Result<String, JobStoreError> {
        let seq = self
            .stage_seq
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| JobStoreError::Io)?;
        Ok(format!(".stage-{label}-{seq}"))
    }

    /// Durably publish `candidate` as the next generation, then adopt it as
    /// the live table. On any error, `self` (both in-memory and on-disk)
    /// remains exactly as it was: either the generation file write failed
    /// (nothing new is reachable from `ACTIVE`), or it succeeded but the
    /// `ACTIVE` pointer write failed (a stray but harmless generation file
    /// exists; `ACTIVE` still names the previous one, which is what a later
    /// `open` will load).
    fn commit(&mut self, candidate: JobTable) -> Result<(), JobStoreError> {
        let bytes = codec::encode(&candidate).ok_or(JobStoreError::RecordTooLarge)?;
        let new_generation = self
            .current_generation
            .checked_add(1)
            .ok_or(JobStoreError::TickOverflow)?;
        let generation_path = self.generations_dir.join(new_generation.to_string());
        let generation_stage = self.next_stage_name("generation")?;
        durable_fs::commit_bytes(&generation_path, &generation_stage, &bytes)
            .map_err(|_| JobStoreError::Io)?;
        let pointer_stage = self.next_stage_name("active")?;
        durable_fs::commit_bytes(
            &self.active_path,
            &pointer_stage,
            &new_generation.to_le_bytes(),
        )
        .map_err(|_| JobStoreError::Io)?;
        self.table = candidate;
        self.current_generation = new_generation;
        Ok(())
    }

    /// Test-only hook into `commit`'s durable-write sequence, used to prove
    /// that a fault at any point leaves the store's *readable* state
    /// (`self.table` after a fresh `open`) unchanged.
    #[cfg(test)]
    fn commit_with_hook(
        &mut self,
        candidate: JobTable,
        hook: &mut Option<&mut durable_fs::Hook<'_>>,
    ) -> Result<(), io::Error> {
        let bytes = codec::encode(&candidate).ok_or_else(|| io::Error::other("too large"))?;
        let new_generation = self.current_generation + 1;
        let generation_path = self.generations_dir.join(new_generation.to_string());
        let generation_stage = self
            .next_stage_name("generation")
            .map_err(|_| io::Error::other("job stage sequence exhausted"))?;
        durable_fs::commit_bytes_with_hook(&generation_path, &generation_stage, &bytes, hook)?;
        let pointer_stage = self
            .next_stage_name("active")
            .map_err(|_| io::Error::other("job stage sequence exhausted"))?;
        durable_fs::commit_bytes_with_hook(
            &self.active_path,
            &pointer_stage,
            &new_generation.to_le_bytes(),
            hook,
        )?;
        self.table = candidate;
        self.current_generation = new_generation;
        Ok(())
    }

    fn find_by_idempotency_key(&self, key: &[u8]) -> Option<&JobRecord> {
        self.table.jobs.values().find(|r| r.idempotency_key == key)
    }

    fn enqueue_locked(
        &mut self,
        request: EnqueueRequest,
        side_entry: Option<(Vec<u8>, Vec<u8>)>,
    ) -> Result<EnqueueOutcome, JobStoreError> {
        if let Some(existing) = self.find_by_idempotency_key(&request.idempotency_key) {
            return if existing.payload == request.payload {
                Ok(EnqueueOutcome::Duplicate(existing.id))
            } else {
                Err(JobStoreError::IdempotencyConflict(existing.id))
            };
        }
        let mut candidate = self.table.clone();
        let id = JobId(candidate.next_job_id);
        candidate.next_job_id = candidate
            .next_job_id
            .checked_add(1)
            .ok_or(JobStoreError::TickOverflow)?;
        candidate.jobs.insert(
            id,
            JobRecord {
                id,
                idempotency_key: request.idempotency_key,
                payload: request.payload,
                state: JobState::Pending,
                attempt: 0,
                lease_epoch: 0,
                retry_policy: request.retry_policy,
                lease: None,
                created_at_tick: request.now_tick,
                last_error: Vec::new(),
            },
        );
        if let Some((key, value)) = side_entry {
            candidate.side_records.insert(key, value);
        }
        self.commit(candidate)?;
        Ok(EnqueueOutcome::Created(id))
    }
}

impl sealed::Sealed for GenerationJobStore {}

impl JobStore for GenerationJobStore {
    fn enqueue(&mut self, request: EnqueueRequest) -> Result<EnqueueOutcome, JobStoreError> {
        self.enqueue_locked(request, None)
    }

    fn enqueue_with_side_record(
        &mut self,
        request: EnqueueRequest,
        side_key: Vec<u8>,
        side_value: Vec<u8>,
    ) -> Result<EnqueueOutcome, JobStoreError> {
        self.enqueue_locked(request, Some((side_key, side_value)))
    }

    fn claim(
        &mut self,
        worker_id: u64,
        now_tick: u64,
        lease_ticks: u64,
    ) -> Result<(JobId, LeaseToken), JobStoreError> {
        let candidate_id = self
            .table
            .jobs
            .values()
            .find(|r| is_claimable(r, now_tick))
            .map(|r| r.id)
            .ok_or(JobStoreError::NothingToClaim)?;
        let deadline_tick = now_tick
            .checked_add(lease_ticks)
            .ok_or(JobStoreError::TickOverflow)?;
        let mut candidate = self.table.clone();
        let record = candidate
            .jobs
            .get_mut(&candidate_id)
            .ok_or(JobStoreError::UnknownJob)?;
        record.lease_epoch = record
            .lease_epoch
            .checked_add(1)
            .ok_or(JobStoreError::TickOverflow)?;
        let generation = record.lease_epoch;
        record.lease = Some(Lease {
            worker_id,
            generation,
            deadline_tick,
        });
        record.state = JobState::Leased;
        self.commit(candidate)?;
        Ok((
            candidate_id,
            LeaseToken {
                job_id: candidate_id,
                worker_id,
                generation,
            },
        ))
    }

    fn begin_execution(&mut self, token: LeaseToken, now_tick: u64) -> Result<(), JobStoreError> {
        let mut candidate = self.table.clone();
        let record = candidate
            .jobs
            .get_mut(&token.job_id)
            .ok_or(JobStoreError::UnknownJob)?;
        if record.state != JobState::Leased || !lease_is_current(record, &token, now_tick) {
            return Err(JobStoreError::LeaseNotCurrent);
        }
        record.state = JobState::Running;
        self.commit(candidate)
    }

    fn complete(
        &mut self,
        token: LeaseToken,
        now_tick: u64,
        outcome: AttemptOutcome,
        error: Option<Vec<u8>>,
    ) -> Result<JobState, JobStoreError> {
        let mut candidate = self.table.clone();
        let record = candidate
            .jobs
            .get_mut(&token.job_id)
            .ok_or(JobStoreError::UnknownJob)?;
        if record.state != JobState::Running || !lease_is_current(record, &token, now_tick) {
            return Err(JobStoreError::LeaseNotCurrent);
        }
        let attempt = record
            .attempt
            .checked_add(1)
            .ok_or(JobStoreError::TickOverflow)?;
        record.attempt = attempt;
        record.last_error = error.unwrap_or_default();
        let next_state = retry::next_state_after_attempt(outcome, attempt, record.retry_policy);
        record.state = next_state;
        if !next_state.is_terminal() {
            // A retryable failure below the ceiling returns to `Pending` so
            // a later `claim` picks it back up; the lease it just held is
            // cleared so a stale completion of the *old* lease can never
            // race a fresh claim (fenced by `lease_epoch` regardless).
            record.lease = None;
        }
        self.commit(candidate)?;
        Ok(next_state)
    }

    fn cancel(&mut self, job_id: JobId) -> Result<JobState, JobStoreError> {
        let mut candidate = self.table.clone();
        let record = candidate
            .jobs
            .get_mut(&job_id)
            .ok_or(JobStoreError::UnknownJob)?;
        let cancellable = matches!(
            record.state,
            JobState::Pending | JobState::Leased | JobState::RetryableFailure
        );
        if !cancellable {
            return Err(JobStoreError::IllegalTransition);
        }
        record.state = JobState::Cancelled;
        record.lease = None;
        self.commit(candidate)?;
        Ok(JobState::Cancelled)
    }

    fn get(&self, job_id: JobId) -> Option<&JobRecord> {
        self.table.jobs.get(&job_id)
    }

    fn side_record(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.table.side_records.get(key)
    }
}

/// A job is claimable when it has never been leased (`Pending`), when a
/// completed attempt left it awaiting retry (`RetryableFailure`), or when a
/// prior lease exists but has expired: `state` still reads `Leased` or
/// `Running` (nothing else ever reclaimed it), and `now_tick` has reached or
/// passed the recorded `deadline_tick`. Reclaiming mints a fresh
/// `lease_epoch` in `claim`, which is what fences the old, now-expired
/// holder's token out of `complete` even if that stale call arrives later
/// naming the same `worker_id`.
///
/// `RetryableFailure` is reclaimable immediately rather than gated behind
/// `retry::backoff_ticks`'s delay: wiring a due-time schedule onto this
/// state is the `std.jobs.schedule`-shaped concern `docs/DURABLE-JOBS-V1.md`
/// and `src/job_fixture.rs` already own, and is explicitly out of this
/// module's scope (see `model::JobState`'s doc comment).
fn is_claimable(record: &JobRecord, now_tick: u64) -> bool {
    match record.state {
        JobState::Pending | JobState::RetryableFailure => true,
        JobState::Leased | JobState::Running => record
            .lease
            .is_some_and(|lease| now_tick >= lease.deadline_tick),
        _ => false,
    }
}

fn lease_is_current(record: &JobRecord, token: &LeaseToken, now_tick: u64) -> bool {
    match record.lease {
        Some(lease) => {
            lease.worker_id == token.worker_id
                && lease.generation == token.generation
                && now_tick < lease.deadline_tick
        }
        None => false,
    }
}

#[cfg(test)]
mod tests;
