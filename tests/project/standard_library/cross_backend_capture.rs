//! Issue #102: helpers that capture a backend's *actual* computed value
//! instead of independently checking it against a hardcoded sentinel. Three
//! backends each individually asserting they returned `0` proves nothing
//! about equivalence between them; a real comparison must read one
//! backend's computed value and check it against another's.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

use semaprax::project::ProjectExecutionOutcome;

/// Runs a binary produced by the entry wrapper `codegen::emit_hir_c` emits
/// and returns the full-fidelity `i64` it printed to stdout (the wrapper
/// always `printf("%lld\n", result)` before returning process exit code
/// `0`), instead of only checking that printed value against a literal.
/// Callers use this to compare the actual computed value across backends
/// rather than independently checking each backend against a hardcoded
/// sentinel.
pub(super) fn run_and_capture_i64(path: &Path) -> i64 {
    let (output, timed_out) =
        capture_with_timeout(&mut Command::new(path), Duration::from_secs(30))
            .unwrap_or_else(|error| panic!("{} capture failed: {error}", path.display()));
    assert!(
        !timed_out && output.status.success(),
        "{} {}: {} stdout={:?} stderr={:?}",
        path.display(),
        if timed_out {
            "exceeded 30 seconds"
        } else {
            "failed"
        },
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.trim().parse::<i64>().unwrap_or_else(|error| {
        panic!(
            "{} printed a non-i64 result {stdout:?}: {error}",
            path.display()
        )
    })
}

struct CaptureDirectory(PathBuf);

impl Drop for CaptureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn capture_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<(Output, bool)> {
    let directory = CaptureDirectory(super::temporary("native-capture"));
    let stdout = directory.0.join("stdout");
    let stderr = directory.0.join("stderr");
    // Files keep output collection independent of pipe capacity or EOF held
    // by a descendant. Only the exact owned child is polled, killed and reaped.
    command
        .stdin(Stdio::null())
        .stdout(File::create(&stdout)?)
        .stderr(File::create(&stderr)?);
    let spawned = command.spawn();
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = spawned?;
    let (status, timed_out) = wait_with_timeout(&mut child, timeout)?;
    drop(child);
    Ok((
        Output {
            status,
            stdout: fs::read(stdout)?,
            stderr: fs::read(stderr)?,
        },
        timed_out,
    ))
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> io::Result<(ExitStatus, bool)> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((status, false)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok(None) => {}
        }
        if started.elapsed() >= timeout {
            let killed = child.kill();
            let reaped = child.wait();
            killed?;
            return Ok((reaped?, true));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[cfg(unix)]
fn native_capture_preserves_completed_child_output() {
    let mut command = Command::new("/bin/sh");
    command.args([
        "-c",
        "printf '37\\n'; i=0; while [ \"$i\" -lt 10000 ]; do printf '0123456789' >&2; i=$((i + 1)); done",
    ]);
    let (output, timed_out) = capture_with_timeout(&mut command, Duration::from_secs(5)).unwrap();
    assert!(!timed_out);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"37\n");
    assert_eq!(output.stderr, b"0123456789".repeat(10000));
}

#[test]
#[cfg(unix)]
fn native_capture_watchdog_kills_and_reaps_timed_out_child() {
    let mut child = Command::new("/bin/sh")
        .args(["-c", "exec sleep 60"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let (status, timed_out) = wait_with_timeout(&mut child, Duration::from_millis(50)).unwrap();
    assert!(timed_out);
    assert!(!status.success());
    assert!(started.elapsed() < Duration::from_secs(3));
    assert_eq!(child.try_wait().unwrap(), Some(status));
}

/// The `i64` a project execution returned, or a panic naming `context` if it
/// did not run to a return at all (a language failure, fuel exhaustion, or a
/// call-depth violation). Used to thread the interpreter's actual computed
/// value into a cross-backend comparison instead of asserting each backend
/// individually returned a hardcoded sentinel.
pub(super) fn interpreter_i64(outcome: &ProjectExecutionOutcome, context: &str) -> i64 {
    match outcome {
        ProjectExecutionOutcome::Returned(value) => *value,
        other => panic!("{context}: {other:?}"),
    }
}
