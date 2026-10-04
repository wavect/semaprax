//! The one process execution: cleared environment, explicit executable and
//! cwd, own process group, deadline and cancellation that kill the group, and
//! bounded capture that digests every byte and spills to a retention file.

use crate::host::CancelToken;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Termination {
    Exited(i32),
    Signaled(i32),
    TimedOut,
    Cancelled,
}

impl Termination {
    /// Exit and signal are observed facts; timeout and cancellation killed the
    /// command mid-run, so what it did before dying is unknown.
    pub fn certain(&self) -> bool {
        matches!(self, Self::Exited(_) | Self::Signaled(_))
    }
    pub fn label(&self) -> String {
        match self {
            Self::Exited(c) => format!("exit:{c}"),
            Self::Signaled(s) => format!("signal:{s}"),
            Self::TimedOut => "timeout".into(),
            Self::Cancelled => "cancelled".into(),
        }
    }
    /// Process exit code the CLI reports.
    pub fn code(&self) -> i32 {
        match self {
            Self::Exited(c) => *c,
            Self::Signaled(s) => 128 + s,
            Self::TimedOut => 124,
            Self::Cancelled => 130,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stream {
    pub total: u64,
    pub digest: String,
    /// First `mem_cap` bytes.
    pub head: Vec<u8>,
    /// Bytes written to the spill file.
    pub stored: u64,
    pub spilled: bool,
    /// The stream ended (EOF) before the result was taken.
    pub settled: bool,
}

impl Stream {
    /// Every byte is available in memory.
    pub fn in_memory(&self) -> bool {
        self.head.len() as u64 == self.total && self.settled
    }
    /// Every byte is in the spill file.
    pub fn fully_stored(&self) -> bool {
        self.spilled && self.stored == self.total && self.settled
    }
}

pub struct Spill {
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub max_stream: u64,
}

pub struct ExecSpec<'a> {
    pub argv: &'a [String],
    pub executable: &'a Path,
    pub cwd: &'a Path,
    pub env: &'a BTreeMap<String, String>,
    pub timeout: Duration,
    pub mem_cap: usize,
    pub spill: Option<Spill>,
    pub cancel: &'a CancelToken,
}

pub struct Captured {
    pub termination: Termination,
    pub stdout: Stream,
    pub stderr: Stream,
    pub elapsed_ms: u64,
}

struct Acc {
    hasher: Sha256,
    total: u64,
    head: Vec<u8>,
    file: Option<File>,
    stored: u64,
    done: bool,
}

type Shared = Arc<Mutex<Acc>>;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn lock(a: &Shared) -> std::sync::MutexGuard<'_, Acc> {
    a.lock().unwrap_or_else(|p| p.into_inner())
}

fn open_spill(path: &Option<PathBuf>) -> std::io::Result<Option<File>> {
    use std::os::unix::fs::OpenOptionsExt;
    path.as_ref()
        .map(|p| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(p)
        })
        .transpose()
}

fn drain<R: Read + Send + 'static>(
    mut r: R,
    mem_cap: usize,
    file: Option<File>,
    max_stream: u64,
) -> Shared {
    let acc: Shared = Arc::new(Mutex::new(Acc {
        hasher: Sha256::new(),
        total: 0,
        head: Vec::new(),
        file,
        stored: 0,
        done: false,
    }));
    let shared = acc.clone();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let mut a = lock(&shared);
            a.hasher.update(&buf[..n]);
            a.total += n as u64;
            let room = mem_cap.saturating_sub(a.head.len());
            a.head.extend_from_slice(&buf[..n.min(room)]);
            let stored = a.stored;
            if let Some(f) = a.file.as_mut() {
                let take = (max_stream.saturating_sub(stored) as usize).min(n);
                if take > 0 && f.write_all(&buf[..take]).is_ok() {
                    a.stored += take as u64;
                }
            }
        }
        let mut a = lock(&shared);
        if let Some(f) = a.file.as_mut() {
            let _ = f.flush();
        }
        a.file = None;
        a.done = true;
    });
    acc
}

fn snapshot(a: &Shared, spilled: bool) -> Stream {
    let a = lock(a);
    Stream {
        total: a.total,
        digest: format!("sha256:{}", hex(&a.hasher.clone().finalize())),
        head: a.head.clone(),
        stored: a.stored,
        spilled,
        settled: a.done,
    }
}

fn kill_group(pid: u32) {
    if let Some(p) = rustix::process::Pid::from_raw(pid as i32) {
        let _ = rustix::process::kill_process_group(p, rustix::process::Signal::KILL);
    }
}

fn wait_done(acc: [&Shared; 2], until: Instant) {
    while Instant::now() < until && !acc.iter().all(|a| lock(a).done) {
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Spawn failure means the command never ran.
pub fn run(spec: &ExecSpec) -> std::io::Result<Captured> {
    let (so, se) = match &spec.spill {
        Some(s) => (Some(s.stdout.clone()), Some(s.stderr.clone())),
        None => (None, None),
    };
    let (fo, fe) = (open_spill(&so)?, open_spill(&se)?);
    let mut cmd = Command::new(spec.executable);
    cmd.arg0(&spec.argv[0])
        .args(&spec.argv[1..])
        .env_clear()
        .envs(spec.env)
        .current_dir(spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let start = Instant::now();
    let mut child = cmd.spawn()?;
    let max = spec.spill.as_ref().map_or(0, |s| s.max_stream);
    let out = drain(child.stdout.take().expect("piped"), spec.mem_cap, fo, max);
    let err = drain(child.stderr.take().expect("piped"), spec.mem_cap, fe, max);
    let pid = child.id();
    let deadline = start + spec.timeout;
    let termination = loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                break match (st.code(), st.signal()) {
                    (Some(c), _) => Termination::Exited(c),
                    (None, Some(s)) => Termination::Signaled(s),
                    _ => Termination::Exited(-1),
                };
            }
            Ok(None) => {}
            Err(_) => break Termination::Exited(-1),
        }
        if spec.cancel.is_cancelled() {
            kill_group(pid);
            let _ = child.wait();
            break Termination::Cancelled;
        }
        if Instant::now() >= deadline {
            kill_group(pid);
            let _ = child.wait();
            break Termination::TimedOut;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    // Streams normally close with the command; a background descendant that
    // keeps a pipe open is killed with the group after a short grace.
    wait_done(
        [&out, &err],
        Instant::now() + Duration::from_millis(if termination.certain() { 1500 } else { 500 }),
    );
    if !(lock(&out).done && lock(&err).done) {
        kill_group(pid);
        wait_done([&out, &err], Instant::now() + Duration::from_millis(1000));
    }
    let spilled = spec.spill.is_some();
    Ok(Captured {
        termination,
        stdout: snapshot(&out, spilled),
        stderr: snapshot(&err, spilled),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}
