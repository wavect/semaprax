//! Append-only step journal in the cache directory. A restart with the same
//! lineage reads it to decide what may run again: read-only steps may; a
//! side-effecting step that began is never replayed, and an unfinished
//! publication is reported as uncertain, never retried.
//!
//! One writer per lineage: `open` takes an exclusive advisory `flock` on the
//! sibling `<lineage>.journal.flock` file and holds it until the `Journal` is
//! dropped (or the process exits, which releases it). Every record is read
//! after the lock is held, so the decision (`may_run`) and the durable `begin`
//! are made against the file's true tail. A second writer gets
//! `SPX-HPD070` (busy). The lock file is never deleted: a lock is a kernel
//! object on an open file, so an old-looking file proves nothing.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Injected persistence fault for tests: given the state about to be appended,
/// returns the fault to simulate (`None` = write normally).
pub type FaultHook = Arc<dyn Fn(&str) -> Option<Fault> + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The append fails before any byte is written.
    Write,
    /// The record is written, then the sync fails: its durability is unknown.
    Sync,
    /// Half of the line is written (no newline), then the append fails.
    Torn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub step: String,
    /// `begin`, `done`, `refused`, `uncertain`, `cancelled` (non-execution proved).
    pub state: String,
    pub detail: Value,
}

pub struct Journal {
    path: PathBuf,
    records: Vec<Record>,
    /// Holds the writer lock for the journal's lifetime.
    _lock: Option<std::fs::File>,
    fault: Option<FaultHook>,
}

fn is_busy(d: &HarnessDiagnostic) -> bool {
    d.code == "SPX-HPD070" && d.message.contains("is busy")
}

fn io(path: &Path, e: impl std::fmt::Display) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPD070", format!("journal {}: {e}", path.display()))
}

#[cfg(unix)]
fn try_lock(dir: &Path, lineage: &str) -> HarnessResult<Option<std::fs::File>> {
    use rustix::fs::{flock, FlockOperation};
    let lock_path = dir.join(format!("{lineage}.journal.flock"));
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| io(&lock_path, e))?;
    match flock(&f, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(f)),
        Err(e) if e == rustix::io::Errno::WOULDBLOCK => Err(HarnessDiagnostic::new(
            "SPX-HPD070",
            format!(
                "journal {} is busy: another writer holds the lineage lock",
                lock_path.display()
            ),
        )),
        Err(e) => Err(io(&lock_path, e)),
    }
}

#[cfg(not(unix))]
fn try_lock(_: &Path, _: &str) -> HarnessResult<Option<std::fs::File>> {
    Ok(None)
}

impl Journal {
    /// Open the lineage journal as its sole writer, or fail `SPX-HPD070` when
    /// another writer holds it.
    pub fn open(dir: &Path, lineage: &str) -> HarnessResult<Journal> {
        std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        let lock = try_lock(dir, lineage)?;
        let path = dir.join(format!("{lineage}.journal.jsonl"));
        let mut records = Vec::new();
        if path.exists() {
            let text = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
            for (i, line) in text.lines().enumerate() {
                let v: Value = serde_json::from_str(line)
                    .map_err(|e| io(&path, format!("line {}: {e}", i + 1)))?;
                records.push(Record {
                    seq: v["seq"].as_u64().unwrap_or(0),
                    step: v["step"].as_str().unwrap_or("").into(),
                    state: v["state"].as_str().unwrap_or("").into(),
                    detail: v["detail"].clone(),
                });
            }
        }
        Ok(Journal {
            path,
            records,
            _lock: lock,
            fault: None,
        })
    }

    /// Like `open`, but a busy lineage is retried until `wait` elapses. Never
    /// breaks or deletes the lock; the holder's exit releases it.
    pub fn open_wait(dir: &Path, lineage: &str, wait: Duration) -> HarnessResult<Journal> {
        let end = Instant::now() + wait;
        loop {
            match Journal::open(dir, lineage) {
                Err(d) if is_busy(&d) && Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                other => return other,
            }
        }
    }

    /// Install a persistence fault injector (tests).
    pub fn set_fault(&mut self, hook: Option<FaultHook>) {
        self.fault = hook;
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Append one record and flush it to disk before returning.
    pub fn append(&mut self, step: &str, state: &str, detail: Value) -> HarnessResult<()> {
        let seq = self.records.len() as u64 + 1;
        let line = crate::json::canonical(
            &json!({"seq": seq, "step": step, "state": state, "detail": detail}),
        );
        let fault = self.fault.as_ref().and_then(|h| h(state));
        if fault == Some(Fault::Write) {
            return Err(io(&self.path, "injected write fault"));
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| io(&self.path, e))?;
        if fault == Some(Fault::Torn) {
            let _ = f.write_all(&line.as_bytes()[..line.len() / 2]);
            return Err(io(&self.path, "injected torn write"));
        }
        f.write_all(line.as_bytes())
            .and_then(|_| f.write_all(b"\n"))
            .map_err(|e| io(&self.path, e))?;
        if fault == Some(Fault::Sync) {
            return Err(io(&self.path, "injected sync fault"));
        }
        f.sync_all().map_err(|e| io(&self.path, e))?;
        self.records.push(Record {
            seq,
            step: step.into(),
            state: state.into(),
            detail,
        });
        Ok(())
    }

    /// Latest state recorded for `step`.
    pub fn state(&self, step: &str) -> Option<&Record> {
        self.records.iter().rev().find(|r| r.step == step)
    }

    /// True when `step` has a `begin` with no later terminal record.
    pub fn unfinished(&self, step: &str) -> bool {
        matches!(self.state(step), Some(r) if r.state == "begin")
    }

    /// Record that a cancelled side-effecting step may have had an external
    /// effect (billed, published or generated). Never replayed; `cause` is data.
    pub fn record_uncertain(&mut self, step: &str, cause: &str) -> HarnessResult<()> {
        self.append(step, "uncertain", json!({"cause": cause}))
    }

    /// Whether a side-effecting `step` may start again. Only a step with no
    /// record, a refusal, or a cancellation that provably never ran may; a
    /// begun, completed or uncertain step is a duplicate attempt.
    pub fn may_run(&self, step: &str) -> bool {
        match self.state(step) {
            None => true,
            Some(r) => matches!(r.state.as_str(), "refused" | "cancelled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncertain_and_begun_steps_are_never_replayed() {
        let dir = std::env::temp_dir().join(format!("hp-hn18-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut j = Journal::open(&dir, "l").unwrap();
        assert!(j.may_run("gen"));
        j.append("gen", "begin", json!({})).unwrap();
        assert!(!j.may_run("gen"));
        j.record_uncertain("gen", "cancel").unwrap();
        assert!(!j.may_run("gen"));
        // Survives a restart: the record is on disk. The writer lock is held
        // until the first journal is dropped.
        assert!(is_busy(&Journal::open(&dir, "l").err().unwrap()));
        drop(j);
        let j2 = Journal::open(&dir, "l").unwrap();
        assert!(!j2.may_run("gen"));
        let mut j = j2;
        j.append("pre", "cancelled", json!({})).unwrap();
        assert!(j.may_run("pre"));
        j.append("done", "done", json!({})).unwrap();
        assert!(!j.may_run("done"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_writer_wins_a_barrier_race_for_the_same_step() {
        let dir = std::env::temp_dir().join(format!("hp-ma03-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let claims: Vec<bool> = (0..8)
            .map(|_| {
                let (dir, barrier) = (dir.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let mut j = Journal::open_wait(&dir, "l", Duration::from_secs(20)).unwrap();
                    if !j.may_run("s") {
                        return false;
                    }
                    j.append("s", "begin", json!({})).unwrap();
                    true
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();
        assert_eq!(claims.iter().filter(|c| **c).count(), 1, "{claims:?}");
        let j = Journal::open(&dir, "l").unwrap();
        assert_eq!(j.records().len(), 1);
        // Another lineage is independent of a held lock.
        let _held = j;
        assert!(Journal::open(&dir, "other").is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn injected_faults_leave_exactly_what_reached_storage() {
        let dir = std::env::temp_dir().join(format!("hp-ma09-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut j = Journal::open(&dir, "l").unwrap();
        j.append("s", "begin", json!({})).unwrap();
        j.set_fault(Some(Arc::new(|st| (st == "done").then_some(Fault::Write))));
        assert!(j.append("s", "done", json!({})).is_err());
        assert!(
            !j.may_run("s"),
            "failed done must not make the step retryable"
        );
        drop(j);
        let j = Journal::open(&dir, "l").unwrap();
        assert_eq!(j.records().len(), 1);
        assert!(j.unfinished("s"));
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Child half of the two-process tests (selected by env). Parent half is
    /// `two_real_processes_claim_one_step_and_exit_releases_ownership`.
    fn child(mode: &str) {
        let dir = PathBuf::from(std::env::var("JMA_DIR").unwrap());
        let id = std::env::var("JMA_ID").unwrap();
        std::fs::write(dir.join(format!("ready-{id}")), "").unwrap();
        while !dir.join("go").exists() {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut j = Journal::open_wait(&dir.join("j"), "l", Duration::from_secs(30)).unwrap();
        if !j.may_run("s") {
            println!("JMA:LOST");
            return;
        }
        j.append("s", "begin", json!({"by": id})).unwrap();
        println!("JMA:CLAIMED");
        if mode == "exit" {
            // Exit while holding the lock: no drop, no cleanup.
            std::process::exit(0);
        }
    }

    #[test]
    fn two_real_processes_claim_one_step_and_exit_releases_ownership() {
        if let Ok(mode) = std::env::var("JMA_CHILD") {
            return child(&mode);
        }
        let dir = std::env::temp_dir().join(format!("hp-ma03-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name = "workflow::journal::tests::two_real_processes_claim_one_step_and_exit_releases_ownership";
        let spawn = |mode: &str, id: &str| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name, "--nocapture", "--test-threads=1"])
                .env("JMA_CHILD", mode)
                .env("JMA_DIR", &dir)
                .env("JMA_ID", id)
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        };
        // Two racing claimants; the one that wins exits holding the lock.
        let kids = [spawn("exit", "a"), spawn("exit", "b")];
        while !(dir.join("ready-a").exists() && dir.join("ready-b").exists()) {
            std::thread::sleep(Duration::from_millis(2));
        }
        std::fs::write(dir.join("go"), "").unwrap();
        let outs: Vec<String> = kids
            .into_iter()
            .map(|k| String::from_utf8(k.wait_with_output().unwrap().stdout).unwrap())
            .collect();
        let claimed = outs.iter().filter(|o| o.contains("JMA:CLAIMED")).count();
        let lost = outs.iter().filter(|o| o.contains("JMA:LOST")).count();
        assert_eq!((claimed, lost), (1, 1), "{outs:?}");
        // The claimant died without dropping: ownership is released, the
        // durable begin remains, and the step is not replayable.
        let j = Journal::open(&dir.join("j"), "l").unwrap();
        assert_eq!(j.records().len(), 1);
        assert!(j.unfinished("s") && !j.may_run("s"));
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
