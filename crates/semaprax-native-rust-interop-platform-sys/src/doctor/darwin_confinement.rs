//! macOS Seatbelt-based doctor confinement primitive and settlement contract.
//!
//! This is the primitive [`DOCTOR-PRODUCTION-PROVISIONER-MACOS-V1`][doc]
//! defines: a per-invocation Seatbelt profile applied through
//! `/usr/bin/sandbox-exec`, one fixed process handoff into it, and a
//! settlement state machine that never collapses `Completed`, `Failed`,
//! `Cancelled`, and `Uncertain` into each other. It is a standalone
//! confinement primitive tested only in isolation here; it is not the
//! ordinary `--version` probe in `doctor::unix::launch::darwin`, and it is
//! not wired into any ordinary CLI route or into `provisioned_doctor_*`.
//!
//! [doc]: https://github.com/wavect/semaprax/blob/main/docs/DOCTOR-PRODUCTION-PROVISIONER-MACOS-V1.md
use std::ffi::OsStr;
use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// The one confinement entry point this module ships. Direct `sandbox_init(3)`
/// linkage is a private, unheadered interface with no stable symbol contract
/// across releases; `sandbox-exec` is Apple's own shipped, documented command
/// surface over the identical enforcement (`man sandbox-exec`, `man sandbox`).
/// It is marked deprecated upstream; this contract's nonclaims record that.
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Poll interval while waiting for the confined process to settle. Coarse
/// enough to avoid busy-spinning, fine enough to keep test deadlines short.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConfinementError {
    /// The fixed confinement launcher is absent or not a regular file. This
    /// is fail-closed: the caller never falls back to an unconfined spawn.
    Unsupported,
    Invalid,
    Spawn,
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureReason {
    ExitCode(i32),
    Signal(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UncertainReason {
    /// `waitpid`-equivalent observation itself failed or errored.
    WaitFailed,
    /// The confined process group still has a member after settlement was
    /// otherwise decided. This is the macOS analog of the Linux contract's
    /// `cgroup.events` `populated 0` proof: the direct child alone settling
    /// is not sufficient, the whole confined group must be gone.
    GroupStillPresent,
    /// The post-timeout kill could not be distinguished from "already gone".
    KillAmbiguous,
}

/// The four settlement outcomes this contract requires stay distinct: a
/// completed operation, a failed one, a supervisor-cancelled one (the
/// deadline fired), and an uncertain one (the operation may or may not have
/// taken effect). See [`StickySettlement`] for why they cannot collapse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Settlement {
    Completed,
    Failed(FailureReason),
    Cancelled,
    Uncertain(UncertainReason),
}

/// Sticky failure/cancellation/uncertainty selection. Once a non-`Completed`
/// status is selected, no later call can move it: "cleanup can never replace
/// the selected status" (`AGENTS.md`). A pending `Completed` is the only
/// status a later call may still override, because it was never a final
/// proof of settlement by itself -- the group-emptiness check that runs
/// after it is part of proving settlement, not cleanup after it.
#[derive(Debug, Default)]
pub(crate) struct StickySettlement(Option<Settlement>);

impl StickySettlement {
    pub(crate) fn select(&mut self, candidate: Settlement) {
        match self.0 {
            None => self.0 = Some(candidate),
            Some(Settlement::Completed) if !matches!(candidate, Settlement::Completed) => {
                self.0 = Some(candidate);
            }
            Some(_) => {}
        }
    }

    pub(crate) fn resolve(self) -> Option<Settlement> {
        self.0
    }
}

struct ScratchProfile {
    path: PathBuf,
}

impl ScratchProfile {
    fn write(scratch_root: &Path) -> Result<Self, ConfinementError> {
        let escaped = escape_seatbelt_string(scratch_root)?;
        let profile = format!(
            "(version 1)\n\
             (deny default)\n\
             (allow process-exec)\n\
             (allow process-fork)\n\
             (allow file-read*)\n\
             (allow sysctl-read)\n\
             (allow mach-lookup)\n\
             (allow file-write*\n  (subpath \"{escaped}\"))\n"
        );
        let path = unique_temp_path("semaprax-doctor-confinement-profile", "sb")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| ConfinementError::Io)?;
        file.write_all(profile.as_bytes())
            .map_err(|_| ConfinementError::Io)?;
        file.flush().map_err(|_| ConfinementError::Io)?;
        Ok(Self { path })
    }
}

impl Drop for ScratchProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn unique_temp_path(prefix: &str, extension: &str) -> Result<PathBuf, ConfinementError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ConfinementError::Io)?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "{prefix}-{}-{nanos}.{extension}",
        std::process::id()
    )))
}

/// Escape a path for embedding in a Seatbelt `(subpath "...")` string literal.
/// Without this, a scratch root containing `"` could close the literal early
/// and splice attacker-controlled profile text into the compiled sandbox.
fn escape_seatbelt_string(path: &Path) -> Result<String, ConfinementError> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.contains(&0) {
        return Err(ConfinementError::Invalid);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ConfinementError::Invalid)?;
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\\' || ch == '"' {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    Ok(escaped)
}

pub(crate) struct ConfinedChild {
    child: Child,
    pgid: libc::pid_t,
    _profile: ScratchProfile,
}

/// Spawn `exe` under a fresh Seatbelt profile that denies everything except
/// reading, process exec/fork, and writing under `scratch_root`. `exe` and
/// `scratch_root` must be absolute; there is no `PATH` lookup anywhere in
/// this path.
pub(crate) fn confined_spawn(
    exe: &Path,
    args: &[&OsStr],
    scratch_root: &Path,
) -> Result<ConfinedChild, ConfinementError> {
    confined_spawn_via(Path::new(SANDBOX_EXEC), exe, args, scratch_root)
}

fn confined_spawn_via(
    sandbox_exec: &Path,
    exe: &Path,
    args: &[&OsStr],
    scratch_root: &Path,
) -> Result<ConfinedChild, ConfinementError> {
    if !matches!(sandbox_exec.metadata(), Ok(metadata) if metadata.is_file()) {
        return Err(ConfinementError::Unsupported);
    }
    if !exe.is_absolute() || !scratch_root.is_absolute() {
        return Err(ConfinementError::Invalid);
    }
    // Seatbelt matches `subpath` against the symlink-resolved path the
    // kernel actually opens, not the literal string handed to this profile.
    // `std::env::temp_dir()` is commonly a symlink (`/var` -> `/private/var`
    // on macOS): embedding the unresolved path here would make even an
    // in-scratch write fail closed as a false-positive denial.
    let canonical_scratch = scratch_root
        .canonicalize()
        .map_err(|_| ConfinementError::Invalid)?;
    let profile = ScratchProfile::write(&canonical_scratch)?;
    let mut command = Command::new(sandbox_exec);
    command
        .arg("-f")
        .arg(&profile.path)
        .arg(exe)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: the closure only calls the async-signal-safe `setpgid(0, 0)`
    // between fork and exec, and returns a plain `io::Result` to `Command`.
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|_| ConfinementError::Spawn)?;
    let pgid = child.id() as libc::pid_t;
    Ok(ConfinedChild {
        child,
        pgid,
        _profile: profile,
    })
}

pub(crate) struct Settled {
    pub(crate) status: Settlement,
    pub(crate) stdout: Vec<u8>,
}

/// Wait for the confined process, enforcing `deadline`, then prove the whole
/// confined process group is gone before returning. A timeout selects
/// `Cancelled` (the supervisor's own decision), never `Failed`: the operation
/// did not fail on its own, it was stopped. An observation failure at any
/// step selects `Uncertain` and is sticky over every later step, including a
/// clean-looking exit status this function still goes on to inspect for its
/// own diagnostics but must not use to overwrite that selection.
pub(crate) fn settle(mut confined: ConfinedChild, deadline: Duration) -> Settled {
    let mut state = StickySettlement::default();
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_status = None;
    loop {
        match confined.child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                if start.elapsed() >= deadline {
                    timed_out = true;
                    break;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => {
                state.select(Settlement::Uncertain(UncertainReason::WaitFailed));
                break;
            }
        }
    }

    if timed_out {
        // SAFETY: signalling the process group by pid alone; no data race.
        let killed = unsafe { libc::kill(-confined.pgid, libc::SIGKILL) };
        if killed != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            state.select(Settlement::Uncertain(UncertainReason::KillAmbiguous));
        }
        match confined.child.wait() {
            Ok(_) => state.select(Settlement::Cancelled),
            Err(_) => state.select(Settlement::Uncertain(UncertainReason::WaitFailed)),
        }
    } else if let Some(status) = exit_status {
        match status.code() {
            Some(0) => state.select(Settlement::Completed),
            Some(code) => state.select(Settlement::Failed(FailureReason::ExitCode(code))),
            None => state.select(Settlement::Failed(FailureReason::Signal(
                status.signal().unwrap_or(0),
            ))),
        }
    }

    // The macOS analog of the Linux contract's empty-cgroup proof: `kill(2)`
    // against the negated pgid reports ESRCH only when no process remains in
    // the confined group. A live member, or any other observation, forbids
    // treating the invocation as settled.
    // SAFETY: signal 0 performs no action beyond the existence check.
    let probe = unsafe { libc::kill(-confined.pgid, 0) };
    if probe == 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
        state.select(Settlement::Uncertain(UncertainReason::GroupStillPresent));
    }

    let mut stdout = Vec::new();
    if let Some(mut pipe) = confined.child.stdout.take() {
        let _ = pipe.read_to_end(&mut stdout);
    }

    Settled {
        status: state
            .resolve()
            .unwrap_or(Settlement::Uncertain(UncertainReason::WaitFailed)),
        stdout,
    }
}
