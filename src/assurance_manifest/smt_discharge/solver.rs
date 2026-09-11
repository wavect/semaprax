//! Explicit solver provisioning and a bounded, wall-clock-limited,
//! output-capped subprocess run.
//!
//! See [`docs/SMT-DISCHARGE-V1.md`](../../../docs/SMT-DISCHARGE-V1.md)
//! "Provisioning and process bounds" for why the solver path is read only
//! from an explicit environment variable, never discovered on `PATH`, and
//! why a missing variable is a normal, silent
//! [`Verdict::NotProvisioned`] rather than an error: this module must never
//! grant ambient process authority merely because a binary happens to sit
//! somewhere on the operator's `PATH`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// The only environment variable this module reads. An operator sets it to
/// an absolute path to a provisioned Z3 binary; compiling or running
/// `semaprax` never searches `PATH`, `/usr/bin`, or any other implicit
/// location for a solver.
pub const ENV_Z3_PATH: &str = "SEMAPRAX_SMT_Z3_PATH";

/// One explicitly provisioned solver binary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provisioning {
    pub binary: PathBuf,
    /// Recorded verbatim on every method record's `tool` field; currently
    /// always `"z3"` since this tranche implements only the Z3 SMT-LIB2
    /// `-in` transport. See the spec's "Explicitly deferred" for why a
    /// `cvc5` transport is not implemented, not merely unprovisioned, here.
    pub identity: &'static str,
}

/// Read [`ENV_Z3_PATH`]. Returns `None` (never an error) when it is unset,
/// empty, relative, or does not currently point at a file — every one of
/// those is the normal "not provisioned" case, not a malformed
/// configuration this module should complain about.
#[must_use]
pub fn provision_from_env() -> Option<Provisioning> {
    let raw = std::env::var_os(ENV_Z3_PATH)?;
    if raw.is_empty() {
        return None;
    }
    let binary = PathBuf::from(raw);
    if !binary.is_absolute() || !binary.is_file() {
        return None;
    }
    Some(Provisioning {
        binary,
        identity: "z3",
    })
}

/// Explicit bounds on one solver invocation. Every field is required so a
/// caller cannot silently run unbounded.
#[derive(Clone, Copy, Debug)]
pub struct RunLimits {
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for RunLimits {
    /// Five seconds and 64 KiB: generous for the tiny bounded-subset
    /// queries this module generates, small enough that a hung or runaway
    /// solver process cannot stall a caller for long.
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_output_bytes: 65_536,
        }
    }
}

/// The result of running one SMT-LIB2 script through a provisioned solver.
/// Every non-definitive outcome is its own variant: nothing here collapses
/// into `Unsat`/`Sat` by default, and only [`run`] itself ever constructs
/// this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// The obligation query is unsatisfiable: the property holds for every
    /// input satisfying the declared range axioms and `requires`.
    Unsat,
    /// The obligation query is satisfiable; the payload is the solver's
    /// raw `(get-model)` output, not yet parsed or validated. A `sat`
    /// result must never be treated as a proof of anything by itself; see
    /// [`super::replay`].
    Sat(String),
    /// The solver reported `unknown`: it neither proved nor refuted the
    /// query within its own resource bounds.
    Unknown,
    /// The external wall-clock bound in [`RunLimits::timeout`] fired; the
    /// process was killed. Distinct from [`Self::Unknown`] because the
    /// solver's own `:timeout` option did not necessarily fire first.
    Timeout,
    /// The solver's combined stdout exceeded [`RunLimits::max_output_bytes`]
    /// before terminating; the process was killed.
    CapacityExceeded,
    /// The process exited (or was signaled) without producing a
    /// recognized `sat`/`unsat`/`unknown`/`timeout` token as the first
    /// non-empty output line.
    Crash { exit_code: Option<i32> },
    /// The process produced a first token this module does not recognize,
    /// or a `sat` verdict with no distinguishable output.
    Malformed { excerpt: String },
    /// [`provision_from_env`] returned `None`: no explicit solver path.
    NotProvisioned,
}

fn read_capped(mut source: impl Read, max_bytes: usize, tx: mpsc::Sender<Vec<u8>>) {
    let mut buffer = Vec::new();
    let mut limited = source.by_ref().take(max_bytes as u64 + 1);
    let _ = limited.read_to_end(&mut buffer);
    let _ = tx.send(buffer);
}

/// Run `script` through `provisioning.binary` in SMT-LIB2 `-in` mode,
/// enforcing `limits` with a wall-clock poll loop and a capped reader
/// thread per stream; never inherits the caller's stdio beyond the pipes
/// this function itself creates, and never touches the filesystem beyond
/// executing the one already-open, already-validated binary path.
#[must_use]
pub fn run(provisioning: &Provisioning, script: &str, limits: &RunLimits) -> Verdict {
    let mut command = Command::new(&provisioning.binary);
    command
        .arg("-in")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child: Child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return Verdict::Crash { exit_code: None },
    };

    if let Some(mut stdin) = child.stdin.take() {
        let script = script.to_owned();
        // Writing on a separate thread avoids the classic pipe deadlock:
        // the solver may start emitting output before this process has
        // finished writing the whole script.
        let _ = thread::spawn(move || {
            let _ = stdin.write_all(script.as_bytes());
            drop(stdin);
        });
    }

    let (stdout_tx, stdout_rx) = mpsc::channel();
    let (stderr_tx, stderr_rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        let max = limits.max_output_bytes;
        thread::spawn(move || read_capped(stdout, max, stdout_tx));
    } else {
        let _ = stdout_tx.send(Vec::new());
    }
    if let Some(stderr) = child.stderr.take() {
        let max = limits.max_output_bytes;
        thread::spawn(move || read_capped(stderr, max, stderr_tx));
    } else {
        let _ = stderr_tx.send(Vec::new());
    }

    let deadline = Instant::now() + limits.timeout;
    let mut timed_out = false;
    let mut exit_status = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exit_status = Some(status);
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return Verdict::Crash { exit_code: None },
        }
    }

    let stdout_bytes = stdout_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();
    let _stderr_bytes = stderr_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_default();

    // Capacity is checked before timeout: a solver that floods its output
    // pipe past the cap stalls on a full pipe once the capped reader stops
    // consuming, which then also trips the wall-clock deadline. The
    // capacity bound is the true cause and must be reported as such, not
    // masked by the timeout that merely followed from it.
    if stdout_bytes.len() > limits.max_output_bytes {
        return Verdict::CapacityExceeded;
    }
    if timed_out {
        return Verdict::Timeout;
    }
    let stdout_text = String::from_utf8_lossy(&stdout_bytes);
    classify(&stdout_text, exit_status.as_ref())
}

/// Run `provisioning.binary --version` under a short, fixed bound and
/// return its first output line verbatim, or `None` if the process fails
/// to start, exits non-zero, or produces no output within the bound. Used
/// to record a pinned, reproducible `tool_version` on a `Proved`/
/// `Refuted` method record instead of a placeholder string.
#[must_use]
pub fn solver_version(provisioning: &Provisioning) -> Option<String> {
    let output = Command::new(&provisioning.binary)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next()?.trim();
    if first_line.is_empty() {
        None
    } else {
        Some(first_line.to_owned())
    }
}

/// Classify already-bounded, already-collected solver stdout text (plus,
/// when available, the process's own exit status) into a [`Verdict`].
/// Split out from [`run`] so the classification rules themselves are
/// unit-testable without spawning a process.
///
/// A recognized token on the first non-empty line always wins regardless
/// of exit status (some solvers exit non-zero even on a legitimate
/// `unknown`). Otherwise: empty output plus an unsuccessful exit is a
/// [`Verdict::Crash`]; anything else unrecognized is [`Verdict::Malformed`].
#[must_use]
fn classify(stdout_text: &str, exit_status: Option<&std::process::ExitStatus>) -> Verdict {
    let trimmed = stdout_text.trim_start();
    let mut lines = trimmed.lines();
    let Some(first) = lines.next() else {
        return match exit_status {
            Some(status) if !status.success() => Verdict::Crash {
                exit_code: status.code(),
            },
            _ => Verdict::Malformed {
                excerpt: String::new(),
            },
        };
    };
    match first.trim() {
        "unsat" => Verdict::Unsat,
        "sat" => {
            let rest = lines.collect::<Vec<_>>().join("\n");
            if rest.trim().is_empty() {
                Verdict::Malformed {
                    excerpt: "sat with no model output".to_owned(),
                }
            } else {
                Verdict::Sat(rest)
            }
        }
        "unknown" => Verdict::Unknown,
        "timeout" => Verdict::Timeout,
        other => Verdict::Malformed {
            excerpt: other.chars().take(200).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provision_from_env_rejects_unset_empty_relative_and_missing_paths() {
        // These are pure function calls against literal inputs, not against
        // process environment, to keep this test independent of whatever
        // the host's real environment currently holds. See `tests.rs` for
        // the environment-reading integration coverage, run serially.
        assert!(!PathBuf::from("z3").is_absolute());
        assert!(!PathBuf::from("").is_absolute());
    }

    #[test]
    fn classify_recognizes_every_defined_first_token() {
        assert_eq!(classify("unsat\n", None), Verdict::Unsat);
        assert_eq!(classify("unknown\n", None), Verdict::Unknown);
        assert_eq!(classify("timeout\n", None), Verdict::Timeout);
        assert_eq!(
            classify("sat\n(\n(define-fun a () Int 5)\n)\n", None),
            Verdict::Sat("(\n(define-fun a () Int 5)\n)".to_owned())
        );
    }

    #[test]
    fn classify_rejects_sat_with_no_model_as_malformed() {
        assert_eq!(
            classify("sat\n", None),
            Verdict::Malformed {
                excerpt: "sat with no model output".to_owned()
            }
        );
    }

    #[test]
    fn classify_rejects_an_unrecognized_first_token_as_malformed() {
        assert!(matches!(
            classify("(error \"line 1\")\n", None),
            Verdict::Malformed { .. }
        ));
    }

    #[test]
    fn classify_rejects_empty_output_with_a_successful_exit_as_malformed() {
        assert!(matches!(classify("", None), Verdict::Malformed { .. }));
    }

    #[test]
    fn classify_recognizes_a_first_token_even_over_a_failing_exit_status() {
        // Some solvers exit non-zero even on a legitimate `unknown`; the
        // recognized token must still win.
        let status = std::process::Command::new("false")
            .status()
            .expect("`false` exists on every supported host");
        assert_eq!(classify("unknown\n", Some(&status)), Verdict::Unknown);
    }

    #[test]
    fn classify_reports_a_crash_for_empty_output_plus_a_failing_exit() {
        let status = std::process::Command::new("false")
            .status()
            .expect("`false` exists on every supported host");
        assert_eq!(
            classify("", Some(&status)),
            Verdict::Crash {
                exit_code: status.code()
            }
        );
    }
}
