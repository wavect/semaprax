//! One-shot process entry. Only a trusted provisioner may supply its launch context.
//! See docs/DOCTOR-OFFLINE-WORKER-V1.md; this is not ordinary CLI admission.
use super::{offline_root, DoctorOfflineBundle, DoctorOfflineInput, ProbeError};
use sha2::{Digest as _, Sha256};
use std::ffi::CString;
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::FromRawFd;
use std::time::{Duration, Instant};

mod capture;
mod child;
mod guard;
pub(in crate::doctor) mod wire;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::doctor) enum Error {
    Invalid,
    Limit,
    Allocation,
    Io,
}

/// Never return to the embedding process or run its destructors. All descriptors
/// belong to this dedicated worker by the external launch contract.
pub(super) fn entry() -> ! {
    let code = match execute() {
        Ok(()) => 0,
        Err(_) => 2,
    };
    unsafe { libc::_exit(code) }
}

fn execute() -> Result<(), Error> {
    // A pipe-kind check does not authenticate the provisioner or prove the
    // absence of other descriptors. Those are explicit launch prerequisites.
    for fd in 0..=2 {
        if unsafe { libc::fcntl(fd, libc::F_GETPIPE_SZ) } < 0 {
            return Err(Error::Invalid);
        }
    }
    require_wait_policy()?;
    nonblocking(1)?;
    let request_input = acquire(3, 149)?;
    let request = wire::Request::parse(request_input.bytes())?;
    let input = acquire(4, request.bundle_len)?;
    if input.bytes().len() != request.bundle_len
        || <[u8; 32]>::from(Sha256::digest(input.bytes())) != request.bundle_digest
    {
        return Err(Error::Invalid);
    }
    let bundle =
        DoctorOfflineBundle::parse(input, &request.selector).map_err(|_| Error::Invalid)?;
    if bundle.architecture() != request.architecture {
        return Err(Error::Invalid);
    }
    // All requested roles and preparation succeed before the first child.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let page = usize::try_from(page).map_err(|_| Error::Invalid)?;
    let root = offline_root::Plan::prepare(&bundle, page).map_err(|_| Error::Invalid)?;
    let guard = guard::Guard::prepare()?;
    let mut tools = Vec::new();
    tools.try_reserve_exact(3).map_err(|_| Error::Allocation)?;
    for (role, tool) in request.roles() {
        let file = bundle.tool(tool).ok_or(Error::Invalid)?;
        let mut path = Vec::new();
        path.try_reserve_exact(file.path().len() + 2)
            .map_err(|_| Error::Allocation)?;
        path.push(b'/');
        path.extend_from_slice(file.path().as_bytes());
        path.push(0);
        let path = CString::from_vec_with_nul(path).map_err(|_| Error::Invalid)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(65_536)
            .map_err(|_| Error::Allocation)?;
        tools.push((role, path, output));
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(3).map_err(|_| Error::Allocation)?;
    // No input descriptors enter a tool. Snapshot storage stays immutable.
    close_owned(3);
    close_owned(4);
    for (role, path, mut output) in tools {
        let outcome = capture::run(&root, &guard, &path, &mut output);
        rows.push((role, outcome.map(|()| output)));
    }
    let reply = wire::encode_reply(&request, &rows)?;
    // Independently require exact framing before touching the report sink.
    wire::validate_reply(&request, &reply)?;
    write_reply(&reply)?;
    close_owned(1);
    close_owned(0);
    close_owned(2);
    Ok(())
}

fn acquire(fd: i32, limit: usize) -> Result<DoctorOfflineInput, Error> {
    if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(Error::Invalid);
    }
    // Fixed descriptors are transferred by the provisioner, never request-
    // chosen. Borrow for sealed acquisition; explicit close happens above.
    let file = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    DoctorOfflineInput::acquire(&file, limit).map_err(|_| Error::Invalid)
}

fn require_wait_policy() -> Result<(), Error> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(Error::Io);
    }
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn nonblocking(fd: i32) -> Result<(), Error> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        return Err(Error::Io);
    }
    Ok(())
}

fn write_reply(bytes: &[u8]) -> Result<(), Error> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut offset = 0;
    while offset < bytes.len() {
        if Instant::now() >= deadline {
            return Err(Error::Io);
        }
        let end = bytes.len().min(offset + 8192);
        let count = unsafe { libc::write(1, bytes[offset..end].as_ptr().cast(), end - offset) };
        if count > 0 {
            offset += count as usize;
        } else if count < 0 && errno() == libc::EAGAIN {
            std::thread::sleep(Duration::from_millis(1));
        } else {
            return Err(Error::Io);
        }
    }
    Ok(())
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn close_owned(fd: i32) {
    if unsafe { libc::close(fd) } != 0 {
        fail_stop();
    }
}

fn fail_stop() -> ! {
    unsafe { libc::_exit(126) }
}

struct Fd(i32);
impl Drop for Fd {
    fn drop(&mut self) {
        close_owned(self.0);
    }
}

fn pipe() -> Result<(Fd, Fd), ProbeError> {
    let mut pair = [-1; 2];
    if unsafe { libc::pipe2(pair.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(ProbeError::Spawn);
    }
    Ok((Fd(pair[0]), Fd(pair[1])))
}
