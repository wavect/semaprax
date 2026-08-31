//! PID-namespace ownership and fair, bounded capture for one prepared tool.
use super::{child, fail_stop, guard::Guard, nonblocking, offline_root, pipe, Fd, ProbeError};
use std::ffi::CStr;
use std::time::{Duration, Instant};

mod operations;
use operations::{Native, Operations};
#[cfg(test)]
mod tests;

#[repr(C)]
#[derive(Default)]
struct CloneArgs {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

pub(super) fn run(
    plan: &offline_root::Plan<'_>,
    guard: &Guard,
    path: &CStr,
    output: &mut Vec<u8>,
) -> Result<(), ProbeError> {
    let (stdin, empty) = pipe()?;
    drop(empty);
    let (stdout, stdout_writer) = pipe()?;
    let (stderr, stderr_writer) = pipe()?;
    nonblocking(stdout.0).map_err(|_| ProbeError::Io)?;
    nonblocking(stderr.0).map_err(|_| ProbeError::Io)?;
    let supervisor = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0_u32) };
    if supervisor < 0 {
        return Err(ProbeError::Spawn);
    }
    let supervisor = Fd(supervisor as i32);
    let mut pidfd = -1_i32;
    let arguments = CloneArgs {
        flags: (libc::CLONE_NEWPID | libc::CLONE_PIDFD) as u64,
        pidfd: (&mut pidfd as *mut i32) as u64,
        exit_signal: libc::SIGCHLD as u64,
        ..CloneArgs::default()
    };
    let origin = Instant::now();
    // No CLONE_VM/FILES/THREAD: ordinary private fork-like state, but a fresh
    // PID namespace for each tool, not an irreversible supervisor unshare.
    let pid = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &arguments as *const CloneArgs,
            std::mem::size_of::<CloneArgs>(),
        )
    };
    if pid == 0 {
        unsafe {
            child::enter(
                plan,
                guard,
                path,
                [stdin.0, stdout_writer.0, stderr_writer.0],
                supervisor.0,
            )
        }
    }
    if pid < 0 {
        return Err(ProbeError::Spawn);
    }
    if pidfd < 0 || pid > i32::MAX as libc::c_long {
        fail_stop();
    }
    let mut native = Native::new(pid as i32, pidfd, stdout, stderr, origin);
    drop((stdin, stdout_writer, stderr_writer, supervisor));
    drive(&mut native, output)
}

/// The real supervisor and authority-free scripts use this identical state
/// machine. Scripted outcomes prove control flow, never physical settlement.
fn drive(operations: &mut impl Operations, output: &mut Vec<u8>) -> Result<(), ProbeError> {
    let mut selected = None;
    let mut total = 0;
    let mut ended = [false; 2];
    loop {
        for (index, eof) in ended.iter_mut().enumerate() {
            if !*eof {
                match read(
                    operations,
                    index,
                    &mut total,
                    (index == 0).then_some(&mut *output),
                ) {
                    Ok(value) => *eof = value,
                    Err(error) => {
                        selected.get_or_insert(error);
                    }
                }
            }
        }
        if selected.is_some() {
            break;
        }
        match operations.observe_exit() {
            Ok(true) => break,
            Ok(false) => {}
            Err(()) => {
                selected = Some(ProbeError::Io);
                break;
            }
        }
        if operations.now() >= Duration::from_secs(10) {
            selected = Some(ProbeError::Timeout);
            break;
        }
        operations.pause();
    }
    // Signal only the pinned, unreaped identity. Reaping namespace PID 1 also
    // waits for the kernel's namespace-descendant teardown before publication.
    let settle_deadline = operations
        .now()
        .checked_add(Duration::from_secs(5))
        .unwrap_or_else(|| operations.fail_stop());
    let success = settle(operations, settle_deadline);
    while !ended.iter().all(|eof| *eof) {
        if operations.now() >= settle_deadline {
            operations.fail_stop();
        }
        for (index, eof) in ended.iter_mut().enumerate() {
            if !*eof {
                match read(
                    operations,
                    index,
                    &mut total,
                    (index == 0 && selected.is_none()).then_some(&mut *output),
                ) {
                    Ok(value) => *eof = value,
                    Err(error) => {
                        selected.get_or_insert(error);
                        // An I/O error cannot establish EOF. No successful or
                        // ordinary-error reply may hide uncertain drainage.
                        if error == ProbeError::Io {
                            operations.fail_stop();
                        }
                    }
                }
            }
        }
    }
    if !success {
        selected.get_or_insert(ProbeError::Exit);
    }
    if let Some(error) = selected {
        output.clear();
        Err(error)
    } else {
        Ok(())
    }
}

fn read(
    operations: &mut impl Operations,
    stream: usize,
    total: &mut usize,
    output: Option<&mut Vec<u8>>,
) -> Result<bool, ProbeError> {
    let mut bytes = [0_u8; 8192];
    let Some(count) = operations
        .read(stream, &mut bytes)
        .map_err(|()| ProbeError::Io)?
    else {
        return Ok(false);
    };
    if count == 0 {
        return Ok(true);
    }
    // The native read cannot exceed its buffer. Preserve that invariant even
    // for a malformed scripted implementation rather than indexing unchecked.
    if count > bytes.len() {
        return Err(ProbeError::Io);
    }
    account(total, &bytes[..count], output)?;
    Ok(false)
}

fn account(
    total: &mut usize,
    bytes: &[u8],
    output: Option<&mut Vec<u8>>,
) -> Result<(), ProbeError> {
    let count = bytes.len();
    *total = total.checked_add(count).ok_or(ProbeError::OutputLimit)?;
    if *total > 65_536 {
        return Err(ProbeError::OutputLimit);
    }
    if let Some(output) = output {
        if output
            .capacity()
            .checked_sub(output.len())
            .is_none_or(|remaining| remaining < count)
        {
            return Err(ProbeError::OutputLimit);
        }
        output.extend_from_slice(bytes);
    }
    Ok(())
}

fn settle(operations: &mut impl Operations, deadline: Duration) -> bool {
    if operations.kill_owned().is_err() {
        operations.fail_stop();
    }
    loop {
        match operations.reap_owned() {
            Ok(Some(success)) => return success,
            Ok(None) => {}
            Err(()) => operations.fail_stop(),
        }
        if operations.now() >= deadline {
            operations.fail_stop();
        }
        operations.pause();
    }
}
