//! Trusted, finite product commands only. File-length polling is not a disk
//! quota; exact-child settlement is not descendant containment or a sandbox.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

const MAXIMUM: usize = 64 * 1024 * 1024;

struct OwnedChild {
    child: Child,
    settled: bool,
    kill_attempted: bool,
    cleanup_attempted: bool,
}

impl OwnedChild {
    fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        let status = self.child.try_wait().map_err(|e| e.to_string())?;
        self.settled |= status.is_some();
        Ok(status)
    }

    fn terminate(&mut self) -> Result<(), String> {
        if self.cleanup_attempted {
            return Err("owned child cleanup already attempted".into());
        }
        self.cleanup_attempted = true;
        if self.settled || self.poll()?.is_some() {
            return Ok(());
        }
        if !self.kill_attempted {
            self.kill_attempted = true;
            // No retry or fallback signal. We own this unreaped child; no other
            // reaper or concurrent process-handle mutator is admitted here.
            self.child
                .kill()
                .map_err(|e| format!("owned child kill failed: {e}"))?;
        }
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if self.poll()?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("owned child reap remained uncertain after five seconds".into())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.settled && !self.cleanup_attempted {
            if let Err(error) = self.terminate() {
                eprintln!("archive command cleanup: {error}");
            }
        }
    }
}

fn create(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())
}

fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > maximum {
        return Err("command capture exceeded its byte limit".into());
    }
    Ok(bytes)
}

fn attempt(
    command: &mut Command,
    input: &[u8],
    captures: &Path,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Output, String> {
    if !captures.is_absolute()
        || input.len() > 1024 * 1024
        || stdout_limit > MAXIMUM
        || stderr_limit > MAXIMUM
        || timeout.is_zero()
        || timeout > Duration::from_secs(600)
    {
        return Err("invalid finite-command limits or capture path".into());
    }
    fs::create_dir(captures).map_err(|e| e.to_string())?;
    let stdin_path = captures.join("stdin");
    let stdout_path = captures.join("stdout");
    let stderr_path = captures.join("stderr");
    let mut stdin = create(&stdin_path)?;
    stdin.write_all(input).map_err(|e| e.to_string())?;
    drop(stdin);
    command
        .stdin(Stdio::from(
            File::open(stdin_path).map_err(|e| e.to_string())?,
        ))
        .stdout(Stdio::from(create(&stdout_path)?))
        .stderr(Stdio::from(create(&stderr_path)?));
    let mut owned = OwnedChild {
        child: command.spawn().map_err(|e| e.to_string())?,
        settled: false,
        kill_attempted: false,
        cleanup_attempted: false,
    };
    let start = Instant::now();
    let status = loop {
        let lengths = fs::metadata(&stdout_path)
            .and_then(|stdout| {
                fs::metadata(&stderr_path).map(|stderr| (stdout.len(), stderr.len()))
            })
            .map_err(|e| e.to_string());
        let failure = match lengths {
            Err(error) => Some(error),
            Ok((out, err)) if out > stdout_limit as u64 || err > stderr_limit as u64 => {
                Some("command capture exceeded its byte limit".to_owned())
            }
            _ if start.elapsed() >= timeout => Some("command exceeded its deadline".to_owned()),
            _ => None,
        };
        if let Some(primary) = failure {
            let cleanup = owned.terminate();
            return Err(match cleanup {
                Ok(()) => primary,
                Err(error) => format!("{primary}; {error}"),
            });
        }
        if let Some(status) = owned.poll()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // No pipe readers or unbounded joins. Failed invocations retain all files.
    Ok(Output {
        status,
        stdout: read(&stdout_path, stdout_limit)?,
        stderr: read(&stderr_path, stderr_limit)?,
    })
}

pub(super) fn run(
    command: &mut Command,
    input: &[u8],
    captures: &Path,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Output {
    attempt(
        command,
        input,
        captures,
        timeout,
        stdout_limit,
        stderr_limit,
    )
    .unwrap_or_else(|error| panic!("{}: {error}", captures.display()))
}

#[path = "command/tests.rs"]
mod tests;
