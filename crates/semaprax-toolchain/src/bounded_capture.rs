//! Shared Unix process-group capture. No provider or credential selection.
use crate::opencode_host::OpenCodeRunnerFailure;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};
fn kill_group(child: &mut std::process::Child) {
    if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
}

fn terminate(child: &mut std::process::Child) {
    kill_group(child);
    let _ = child.kill();
    let _ = child.wait();
}

pub(crate) fn capture(
    mut command: Command,
    deadline: Instant,
    limit: usize,
    cancelled: impl Fn() -> bool,
) -> Result<(ExitStatus, Vec<u8>), OpenCodeRunnerFailure> {
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|_| OpenCodeRunnerFailure::Refused)?;
    let Some(stdout) = child.stdout.take() else {
        terminate(&mut child);
        return Err(OpenCodeRunnerFailure::Provider);
    };
    let flags = match rustix::fs::fcntl_getfl(&stdout) {
        Ok(flags) => flags,
        Err(_) => {
            terminate(&mut child);
            return Err(OpenCodeRunnerFailure::Provider);
        }
    };
    if rustix::fs::fcntl_setfl(&stdout, flags | rustix::fs::OFlags::NONBLOCK).is_err() {
        terminate(&mut child);
        return Err(OpenCodeRunnerFailure::Provider);
    }
    let mut output = Vec::new();
    let mut eof = false;
    let mut status = None;
    let mut chunk = [0u8; 8192];
    loop {
        if cancelled() {
            terminate(&mut child);
            return Err(OpenCodeRunnerFailure::Cancelled);
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            return Err(OpenCodeRunnerFailure::Timeout);
        }
        loop {
            if cancelled() || Instant::now() >= deadline {
                terminate(&mut child);
                return Err(if cancelled() {
                    OpenCodeRunnerFailure::Cancelled
                } else {
                    OpenCodeRunnerFailure::Timeout
                });
            }
            match rustix::io::read(&stdout, &mut chunk[..]) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(count) if output.len().saturating_add(count) <= limit => {
                    output.extend_from_slice(&chunk[..count])
                }
                Ok(_) => {
                    terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Malformed);
                }
                Err(rustix::io::Errno::AGAIN) => break,
                Err(_) => {
                    terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Provider);
                }
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    status = Some(exit);
                    // The leader may have exited while a descendant still
                    // owns stdout. Group kill forces the pipe to EOF.
                    kill_group(&mut child);
                }
                Ok(None) => {}
                Err(_) => {
                    terminate(&mut child);
                    return Err(OpenCodeRunnerFailure::Provider);
                }
            }
        }
        if let Some(exit) = status {
            if eof {
                return Ok((exit, output));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
