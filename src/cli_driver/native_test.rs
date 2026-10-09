//! Opt-in, bounded host execution of authenticated Project test roots.
use super::*;
use std::io::Read;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cli::execution::NativeTestLimits;

struct CaseResult {
    stable_id: String,
    name: String,
    is_main: bool,
    result_is_exit_status: bool,
    status: Option<ExitStatus>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    failure: Option<String>,
}

impl CaseResult {
    fn value(&self) -> Option<i64> {
        if self.result_is_exit_status {
            return self
                .status
                .as_ref()
                .and_then(ExitStatus::code)
                .map(i64::from);
        }
        if !self.status.as_ref().is_some_and(ExitStatus::success) {
            return None;
        }
        let text = std::str::from_utf8(&self.stdout).ok()?;
        let digits = text.strip_suffix('\n')?;
        if digits.is_empty() || digits.starts_with('+') || digits.trim() != digits {
            return None;
        }
        let value: i64 = digits.parse().ok()?;
        (value.to_string() == digits).then_some(value)
    }

    fn passed(&self) -> bool {
        self.failure.is_none() && self.value() == Some(0)
    }

    fn outcome(&self) -> String {
        if let Some(failure) = &self.failure {
            return failure.clone();
        }
        if let Some(status) = &self.status {
            if !status.success() && !self.result_is_exit_status {
                return format!(
                    "native process exited with {}",
                    status
                        .code()
                        .map_or_else(|| "a signal".to_owned(), |code| format!("code {code}"))
                );
            }
        }
        match self.value() {
            Some(value) => format!("returned {value}"),
            None => "native result was not a canonical i64 line".to_owned(),
        }
    }
}

fn host_error(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I101", message)]
}

pub(super) fn execute(manifest: &Path, limits: NativeTestLimits, json: bool) -> Result<(), u8> {
    let cases = project::with_authenticated_project(manifest, |snapshot| {
        let roots = snapshot.native_test_roots()?;
        let project_root = snapshot.root().to_path_buf();
        let mut cases = Vec::with_capacity(roots.len());
        for root in roots {
            let leaf = format!("test{}", std::env::consts::EXE_SUFFIX);
            let mut scratch = native_scratch::Scratch::create(&leaf, None).map_err(|error| {
                host_error(format!("cannot create native test scratch: {error}"))
            })?;
            let result = (|| {
                snapshot.build_native_test(root.stable_id(), scratch.path())?;
                scratch.seal().map_err(|error| {
                    host_error(format!("cannot seal native test executable: {error}"))
                })?;
                Ok::<_, Vec<Diagnostic>>(run_case(scratch.path(), &project_root, limits))
            })();
            // Cleanup is required even after a compile, spawn, timeout, or
            // result-format failure. A changed scratch identity fails closed.
            let cleanup = scratch.discard();
            if let Err(error) = cleanup {
                return Err(host_error(format!(
                    "cannot remove native test scratch: {error}"
                )));
            }
            let mut execution = result?;
            execution.stable_id = root.stable_id().to_owned();
            execution.name = root.name().to_owned();
            execution.is_main = root.is_main();
            execution.result_is_exit_status = root.result_is_exit_status();
            cases.push(execution);
        }
        Ok(cases)
    })
    .map_err(|errors| {
        let errors = cli::manifest_hint::hint_missing_manifest(errors, manifest);
        report(&errors, json)
    })?;
    let failed = cases.iter().filter(|case| !case.passed()).count();
    if json {
        let results: Vec<_> = cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "stable_id": case.stable_id,
                    "name": case.name,
                    "role": if case.is_main { "main" } else { "case" },
                    "passed": case.passed(),
                    "result": case.value(),
                    "exit_code": case.status.as_ref().and_then(ExitStatus::code),
                    "outcome": case.outcome(),
                    "stdout": String::from_utf8_lossy(&case.stdout),
                    "stderr": String::from_utf8_lossy(&case.stderr),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({"schema": "semaprax.native-test.v1", "target": "native", "passed": failed == 0, "cases": results})
        );
    } else if failed == 0 {
        println!(
            "project native tests passed ({} named cases)",
            cases.iter().filter(|case| !case.is_main).count()
        );
    } else {
        for case in cases.iter().filter(|case| !case.passed()) {
            eprintln!("failed {}: {}", case.stable_id, case.outcome());
            if !case.stderr.is_empty() {
                eprintln!("stderr: {}", String::from_utf8_lossy(&case.stderr));
            }
        }
        eprintln!(
            "project native tests failed: {failed} of {} roots",
            cases.len()
        );
    }
    if failed == 0 {
        Ok(())
    } else {
        Err(1)
    }
}

fn read_bounded<R: Read>(
    mut reader: R,
    max: usize,
    total: &AtomicUsize,
    overflow: &mpsc::Sender<()>,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = max.saturating_add(1).saturating_sub(bytes.len());
        if remaining == 0 {
            return Ok(bytes);
        }
        let take = remaining.min(chunk.len());
        let count = reader.read(&mut chunk[..take])?;
        if count == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk[..count]);
        if total.fetch_add(count, Ordering::Relaxed).saturating_add(count) > max {
            let _ = overflow.send(());
            return Ok(bytes);
        }
    }
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn run_case(executable: &Path, cwd: &Path, limits: NativeTestLimits) -> CaseResult {
    let mut case = CaseResult {
        stable_id: String::new(),
        name: String::new(),
        is_main: false,
        result_is_exit_status: false,
        status: None,
        stdout: Vec::new(),
        stderr: Vec::new(),
        failure: None,
    };
    let mut child = match Command::new(executable)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            case.failure = Some(format!("cannot start native test: {error}"));
            return case;
        }
    };
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let limit = limits.max_output_bytes;
    let total = Arc::new(AtomicUsize::new(0));
    let (overflow_tx, overflow_rx) = mpsc::channel();
    let stdout_total = Arc::clone(&total);
    let stdout_overflow = overflow_tx.clone();
    let stdout_reader = std::thread::spawn(move || {
        read_bounded(stdout, limit, &stdout_total, &stdout_overflow)
    });
    let stderr_reader = std::thread::spawn(move || {
        read_bounded(stderr, limit, &total, &overflow_tx)
    });
    let started = Instant::now();
    let timeout = Duration::from_millis(limits.timeout_ms);
    loop {
        if overflow_rx.try_recv().is_ok() {
            case.failure = Some(format!("native test exceeded {limit} output bytes"));
            stop(&mut child);
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                case.status = Some(status);
                break;
            }
            Ok(None) if started.elapsed() < timeout => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                case.failure = Some(format!("native test exceeded {} ms", limits.timeout_ms));
                stop(&mut child);
                break;
            }
            Err(error) => {
                case.failure = Some(format!("cannot wait for native test: {error}"));
                stop(&mut child);
                break;
            }
        }
    }
    match stdout_reader.join() {
        Ok(Ok(bytes)) => case.stdout = bytes,
        _ => case.failure = Some("cannot read native test stdout".to_owned()),
    }
    match stderr_reader.join() {
        Ok(Ok(bytes)) => case.stderr = bytes,
        _ => case.failure = Some("cannot read native test stderr".to_owned()),
    }
    if case.stdout.len().saturating_add(case.stderr.len()) > limit {
        case.failure
            .get_or_insert_with(|| format!("native test exceeded {limit} output bytes"));
    }
    case
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reader_never_accumulates_unbounded_output() {
        let total = AtomicUsize::new(0);
        let (overflow, detected) = mpsc::channel();
        let output = read_bounded(&b"abcdefgh"[..], 3, &total, &overflow).unwrap();
        assert_eq!(output, b"abcd");
        assert!(detected.try_recv().is_ok());
    }

    #[test]
    fn native_pure_result_requires_one_canonical_integer_line() {
        let mut case = CaseResult {
            stable_id: "case".to_owned(),
            name: "case".to_owned(),
            is_main: false,
            result_is_exit_status: false,
            status: Some(success_status()),
            stdout: b"0\n".to_vec(),
            stderr: Vec::new(),
            failure: None,
        };
        assert!(case.passed());
        case.stdout = b"0\n1\n".to_vec();
        assert!(!case.passed());
        case.stdout = b"00\n".to_vec();
        assert!(!case.passed());
        case.stdout = b"-0\n".to_vec();
        assert!(!case.passed());
        case.stdout = b"-9223372036854775808\n".to_vec();
        assert_eq!(case.value(), Some(i64::MIN));
        case.stdout = b"256\n".to_vec();
        assert_eq!(case.value(), Some(256));
        assert!(!case.passed());
        case.stdout = b"0\n".to_vec();
        case.failure = Some("output limit".to_owned());
        assert!(!case.passed());
    }

    fn success_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }
}
