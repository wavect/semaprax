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
use std::collections::HashMap;
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
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
    /// Every record (plain `open` only; empty for an indexed journal).
    records: Vec<Record>,
    /// Retained validated state (indexed open only), lent by `cache`.
    index: Option<Index>,
    cache: Option<Arc<JournalIndex>>,
    /// Holds the writer lock for the journal's lifetime.
    _lock: Option<std::fs::File>,
    fault: Option<FaultHook>,
}

/// Longest journal line accepted; a longer one is a malformed journal.
const MAX_LINE: u64 = 1 << 20;

/// Validated state of one journal file, retained across operations (MF-02):
/// the latest record per step, the validated end offset, the file identity and
/// the last validated line (to detect an in-place rewrite of the prefix).
struct Index {
    id: Option<(u64, u64)>,
    offset: u64,
    count: u64,
    tail: Vec<u8>,
    latest: HashMap<String, Record>,
}

impl Index {
    fn empty() -> Index {
        Index {
            id: None,
            offset: 0,
            count: 0,
            tail: Vec::new(),
            latest: HashMap::new(),
        }
    }
}

/// Process-local retained journal state, one entry per journal file. The entry
/// is lent to the `Journal` that holds the file's writer lock and handed back
/// when it drops, so access is serialised by that same lock. It carries no
/// authority: every decision is still made under the lock after catching up
/// with the file.
#[derive(Default)]
pub struct JournalIndex {
    entries: Mutex<HashMap<PathBuf, Index>>,
    decoded: std::sync::atomic::AtomicU64,
}

impl JournalIndex {
    /// Records decoded from disk through this cache (own appends are applied
    /// without decoding). Test instrumentation.
    #[doc(hidden)]
    pub fn decoded(&self) -> u64 {
        self.decoded.load(std::sync::atomic::Ordering::SeqCst)
    }
}

fn file_id(m: &std::fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        None
    }
}

fn decode(path: &Path, line_no: u64, line: &[u8]) -> HarnessResult<Record> {
    let v: Value =
        serde_json::from_slice(line).map_err(|e| io(path, format!("line {line_no}: {e}")))?;
    Ok(Record {
        seq: v["seq"].as_u64().unwrap_or(0),
        step: v["step"].as_str().unwrap_or("").into(),
        state: v["state"].as_str().unwrap_or("").into(),
        detail: v["detail"].clone(),
    })
}

/// Stream the file from `start`, one bounded line at a time. `on` receives each
/// decoded record, the offset just past its line and the raw line (with its
/// newline). Returns whether the final line was newline-terminated.
fn scan(
    f: &mut std::fs::File,
    path: &Path,
    start: u64,
    first_line: u64,
    mut on: impl FnMut(Record, u64, &[u8]),
) -> HarnessResult<bool> {
    f.seek(SeekFrom::Start(start)).map_err(|e| io(path, e))?;
    let mut r = std::io::BufReader::new(&mut *f);
    let (mut pos, mut n) = (start, first_line);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let got = (&mut r)
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut buf)
            .map_err(|e| io(path, e))?;
        if got == 0 {
            return Ok(true);
        }
        if got as u64 > MAX_LINE {
            return Err(io(path, format!("line {n}: longer than {MAX_LINE} bytes")));
        }
        let terminated = buf.last() == Some(&b'\n');
        let body = if terminated {
            &buf[..got - 1]
        } else {
            &buf[..]
        };
        let rec = decode(path, n, body)?;
        pos += got as u64;
        n += 1;
        on(rec, pos, &buf);
        if !terminated {
            return Ok(false);
        }
    }
}

fn closed(path: &Path, why: &str) -> HarnessDiagnostic {
    io(
        path,
        format!("{why} since it was last validated; failing closed, no step is admitted"),
    )
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

impl Drop for Journal {
    /// Return retained state to the cache before the writer lock is released
    /// (fields drop after this body), so the next lock holder finds it.
    fn drop(&mut self) {
        if let (Some(i), Some(c)) = (self.index.take(), self.cache.as_ref()) {
            let mut g = c.entries.lock().unwrap_or_else(|p| p.into_inner());
            g.insert(self.path.clone(), i);
        }
    }
}

impl Journal {
    /// Open the lineage journal as its sole writer, or fail `SPX-HPD070` when
    /// another writer holds it.
    pub fn open(dir: &Path, lineage: &str) -> HarnessResult<Journal> {
        std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        let lock = try_lock(dir, lineage)?;
        let path = dir.join(format!("{lineage}.journal.jsonl"));
        let mut records = Vec::new();
        match std::fs::File::open(&path) {
            Ok(mut f) => {
                scan(&mut f, &path, 0, 1, |r, _, _| records.push(r))?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(&path, e)),
        }
        Ok(Journal {
            path,
            records,
            index: None,
            cache: None,
            _lock: lock,
            fault: None,
        })
    }

    /// Like `open_wait`, but retaining validated state in `cache` across
    /// operations: the first open decodes the file once, later opens verify the
    /// file identity and validated tail, then decode only records appended
    /// since (by this or another process), all under the writer lock. A
    /// replaced, truncated or rewritten file, or a malformed tail, fails closed
    /// (`SPX-HPD070`) and keeps the retained state; nothing is forgotten.
    pub fn open_wait_indexed(
        dir: &Path,
        lineage: &str,
        wait: Duration,
        cache: &Arc<JournalIndex>,
    ) -> HarnessResult<Journal> {
        let end = Instant::now() + wait;
        let (lock, path) = loop {
            std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
            match try_lock(dir, lineage) {
                Err(d) if is_busy(&d) && Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(d) => return Err(d),
                Ok(l) => break (l, dir.join(format!("{lineage}.journal.jsonl"))),
            }
        };
        let mut index = {
            let mut g = cache.entries.lock().unwrap_or_else(|p| p.into_inner());
            g.remove(&path).unwrap_or_else(Index::empty)
        };
        let res = Journal::catch_up(&path, &mut index, cache);
        // Hand the state back even on failure so it keeps failing closed.
        let journal = Journal {
            path,
            records: Vec::new(),
            index: Some(index),
            cache: Some(cache.clone()),
            _lock: lock,
            fault: None,
        };
        res.map(|()| journal)
    }

    fn catch_up(path: &Path, idx: &mut Index, cache: &JournalIndex) -> HarnessResult<()> {
        let mut f = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return if idx.count == 0 && idx.offset == 0 {
                    Ok(())
                } else {
                    Err(closed(path, "the journal file disappeared"))
                };
            }
            Err(e) => return Err(io(path, e)),
        };
        let meta = f.metadata().map_err(|e| io(path, e))?;
        let id = file_id(&meta);
        if idx.id.is_some() && idx.id != id {
            return Err(closed(path, "the journal file was replaced"));
        }
        if meta.len() < idx.offset {
            return Err(closed(path, "the journal file was truncated"));
        }
        if !idx.tail.is_empty() {
            let mut seen = vec![0u8; idx.tail.len()];
            f.seek(SeekFrom::Start(idx.offset - idx.tail.len() as u64))
                .and_then(|_| f.read_exact(&mut seen))
                .map_err(|e| io(path, e))?;
            if seen != idx.tail {
                return Err(closed(path, "the validated journal tail was rewritten"));
            }
        }
        idx.id = id;
        let start = idx.offset;
        let terminated = scan(&mut f, path, start, idx.count + 1, |r, end, line| {
            cache
                .decoded
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if line.last() != Some(&b'\n') {
                return;
            }
            idx.count += 1;
            idx.offset = end;
            idx.tail = line.to_vec();
            idx.latest.insert(r.step.clone(), r);
        })?;
        if terminated {
            Ok(())
        } else {
            // An appended line would glue onto the unterminated one: refuse.
            Err(closed(path, "an unterminated record was found at the tail"))
        }
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

    /// Every record, for a plain `open`. An indexed journal keeps only the
    /// latest record per step and returns an empty slice.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Append one record and flush it to disk before returning.
    pub fn append(&mut self, step: &str, state: &str, detail: Value) -> HarnessResult<()> {
        let seq = match &self.index {
            Some(i) => i.count,
            None => self.records.len() as u64,
        } + 1;
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
        if let Some(i) = &self.index {
            let len = f.metadata().map_err(|e| io(&self.path, e))?.len();
            if len != i.offset {
                return Err(closed(
                    &self.path,
                    "the journal file changed under the lock",
                ));
            }
        }
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
        let rec = Record {
            seq,
            step: step.into(),
            state: state.into(),
            detail,
        };
        match &mut self.index {
            Some(i) => {
                i.count += 1;
                i.offset += line.len() as u64 + 1;
                i.tail = [line.as_bytes(), b"\n"].concat();
                i.id = i.id.or_else(|| file_id(&f.metadata().ok()?));
                i.latest.insert(rec.step.clone(), rec);
            }
            None => self.records.push(rec),
        }
        Ok(())
    }

    /// Latest state recorded for `step`.
    pub fn state(&self, step: &str) -> Option<&Record> {
        match &self.index {
            Some(i) => i.latest.get(step),
            None => self.records.iter().rev().find(|r| r.step == step),
        }
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
        while mode != "indexed" && !dir.join("go").exists() {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut j = if mode == "indexed" {
            // Primed before the race: the cache already holds the (empty)
            // journal, so the winner's record must arrive by catch-up.
            let cache = Arc::new(JournalIndex::default());
            drop(
                Journal::open_wait_indexed(&dir.join("j"), "l", Duration::from_secs(30), &cache)
                    .unwrap(),
            );
            std::fs::write(dir.join(format!("primed-{id}")), "").unwrap();
            while !dir.join("go").exists() {
                std::thread::sleep(Duration::from_millis(2));
            }
            Journal::open_wait_indexed(&dir.join("j"), "l", Duration::from_secs(30), &cache)
                .unwrap()
        } else {
            Journal::open_wait(&dir.join("j"), "l", Duration::from_secs(30)).unwrap()
        };
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

    #[test]
    fn two_real_processes_with_primed_indexes_claim_one_step() {
        if let Ok(mode) = std::env::var("JMA_CHILD") {
            return child(&mode);
        }
        let dir = std::env::temp_dir().join(format!("hp-mf02-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let name =
            "workflow::journal::tests::two_real_processes_with_primed_indexes_claim_one_step";
        let kids: Vec<_> = ["a", "b"]
            .iter()
            .map(|id| {
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", name, "--nocapture", "--test-threads=1"])
                    .env("JMA_CHILD", "indexed")
                    .env("JMA_DIR", &dir)
                    .env("JMA_ID", id)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        while !(dir.join("primed-a").exists() && dir.join("primed-b").exists()) {
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
        let j = Journal::open(&dir.join("j"), "l").unwrap();
        assert_eq!(j.records().len(), 1);
        drop(j);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hp-mf02-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn idx_open(d: &Path, c: &Arc<JournalIndex>) -> HarnessResult<Journal> {
        Journal::open_wait_indexed(d, "l", Duration::from_secs(5), c)
    }

    #[test]
    fn sequential_fixture_decodes_nothing_and_seeded_history_exactly_once() {
        let d = tmp("seq");
        // Pre-seed 7 historical records through the plain writer.
        let mut j = Journal::open(&d, "l").unwrap();
        for i in 0..7 {
            j.append(&format!("old{i}"), "done", json!({})).unwrap();
        }
        drop(j);
        let cache = Arc::new(JournalIndex::default());
        let n = 25;
        for i in 0..n {
            let step = format!("s{i}");
            let mut j = idx_open(&d, &cache).unwrap(); // claim
            assert!(j.may_run(&step));
            j.append(&step, "begin", json!({})).unwrap();
            drop(j);
            let mut j = idx_open(&d, &cache).unwrap(); // settle
            assert!(!j.may_run(&step));
            j.append(&step, "done", json!({})).unwrap();
        }
        // History parsed once (7); 2n opens and 2n own appends decoded nothing.
        assert_eq!(cache.decoded(), 7);
        let j = Journal::open(&d, "l").unwrap();
        assert_eq!(j.records().len(), 7 + 2 * n);
        let seqs: Vec<u64> = j.records().iter().map(|r| r.seq).collect();
        assert_eq!(seqs, (1..=(7 + 2 * n as u64)).collect::<Vec<_>>());
        drop(j);
        // Empty-start fixture: zero decodes for the same sequence.
        let d2 = tmp("seq-empty");
        let cache = Arc::new(JournalIndex::default());
        for i in 0..n {
            let mut j = idx_open(&d2, &cache).unwrap();
            j.append(&format!("s{i}"), "begin", json!({})).unwrap();
            drop(j);
            let mut j = idx_open(&d2, &cache).unwrap();
            j.append(&format!("s{i}"), "done", json!({})).unwrap();
        }
        assert_eq!(cache.decoded(), 0);
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&d2);
    }

    #[test]
    fn interleaved_writers_see_each_others_records_before_admission() {
        let d = tmp("inter");
        let (ca, cb) = (
            Arc::new(JournalIndex::default()),
            Arc::new(JournalIndex::default()),
        );
        let mut a = idx_open(&d, &ca).unwrap();
        a.append("x", "begin", json!({})).unwrap();
        drop(a);
        let b = idx_open(&d, &cb).unwrap();
        assert!(!b.may_run("x"), "b sees a's begin");
        drop(b);
        let mut b = idx_open(&d, &cb).unwrap();
        b.append("y", "begin", json!({})).unwrap();
        b.append("x", "uncertain", json!({})).unwrap();
        drop(b);
        let a = idx_open(&d, &ca).unwrap();
        assert!(!a.may_run("y") && !a.may_run("x"));
        // a decoded only b's two records; b decoded a's one.
        assert_eq!((ca.decoded(), cb.decoded()), (2, 1));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn truncation_replacement_rewrite_and_torn_tail_fail_closed_never_forget() {
        let d = tmp("hostile");
        let path = d.join("l.journal.jsonl");
        let cache = Arc::new(JournalIndex::default());
        let mut j = idx_open(&d, &cache).unwrap();
        j.append("s", "begin", json!({})).unwrap();
        j.append("s", "uncertain", json!({"cause": "x"})).unwrap();
        drop(j);
        let good = std::fs::read(&path).unwrap();
        let fails = |c: &Arc<JournalIndex>| idx_open(&d, c).err().map(|e| e.code);
        // Truncation to empty: no admission, and not "forgotten".
        std::fs::write(&path, b"").unwrap();
        assert_eq!(fails(&cache), Some("SPX-HPD070"));
        // Create the replacement while the original inode is still live:
        // unlinking first permits Linux to reuse that inode immediately.
        let replacement = d.join("replacement.journal.jsonl");
        std::fs::write(&replacement, &good).unwrap();
        #[cfg(unix)]
        assert_ne!(
            file_id(&std::fs::metadata(&path).unwrap()),
            file_id(&std::fs::metadata(&replacement).unwrap()),
            "replacement fixture must have a distinct file identity"
        );
        std::fs::rename(&replacement, &path).unwrap();
        assert_eq!(fails(&cache), Some("SPX-HPD070"), "new inode");
        // Missing file.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(fails(&cache), Some("SPX-HPD070"));
        // Restoring the original identity is impossible; a fresh cache (a
        // restart) reads whatever file exists, here the restored one.
        std::fs::write(&path, &good).unwrap();
        let fresh = Arc::new(JournalIndex::default());
        assert!(!idx_open(&d, &fresh).unwrap().may_run("s"));
        // In-place rewrite of the validated tail with the same length.
        let mut forged = good.clone();
        let at = forged.len() - 5;
        forged[at] = b'Z';
        std::fs::write(&path, &forged).unwrap();
        let cache2 = Arc::new(JournalIndex::default());
        // Prime from the true content first.
        std::fs::write(&path, &good).unwrap();
        drop(idx_open(&d, &cache2).unwrap());
        {
            use std::io::{Seek, SeekFrom};
            let mut f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            f.seek(SeekFrom::Start(at as u64)).unwrap();
            f.write_all(b"Z").unwrap();
        }
        assert_eq!(fails(&cache2), Some("SPX-HPD070"));
        // Torn tail appended by an unlocked writer.
        std::fs::write(&path, &good).unwrap();
        let cache3 = Arc::new(JournalIndex::default());
        drop(idx_open(&d, &cache3).unwrap());
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(b"{\"seq\":3,\"ste").unwrap();
        assert_eq!(fails(&cache3), Some("SPX-HPD070"));
        assert_eq!(
            fails(&Arc::new(JournalIndex::default())),
            Some("SPX-HPD070")
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn old_format_lines_stay_readable_through_the_index() {
        let d = tmp("old");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("l.journal.jsonl"),
            "{\"seq\":1,\"state\":\"begin\",\"step\":\"a\"}\n{\"seq\":2,\"state\":\"refused\",\"step\":\"a\"}\n{\"seq\":3,\"state\":\"done\",\"step\":\"b\"}\n",
        )
        .unwrap();
        let cache = Arc::new(JournalIndex::default());
        let mut j = idx_open(&d, &cache).unwrap();
        assert!(j.may_run("a") && !j.may_run("b") && j.may_run("c"));
        j.append("c", "begin", json!({})).unwrap();
        drop(j);
        let j = Journal::open(&d, "l").unwrap();
        assert_eq!(j.records().last().unwrap().seq, 4);
        assert_eq!(cache.decoded(), 3);
        let _ = std::fs::remove_dir_all(&d);
    }
}
