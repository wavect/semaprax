//! Only this dedicated-process boundary can mint a settled observation.
use super::{
    DoctorOfflineArchitecture, DoctorOfflineTarget, SettledDoctorObservation, SettledDoctorTool,
};
use crate::doctor::{offline_worker::wire, DoctorOfflineBundle, DoctorOfflineInput};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::FromRawFd;
use std::time::Instant;

mod capture;
mod lifetime;
mod report;
use lifetime::Lifetime;

pub(super) fn collect() -> SettledDoctorObservation {
    // Guard acquisition precedes validation, allocation and input acquisition.
    let mut lifetime = Lifetime::new(Instant::now());
    match execute(&mut lifetime) {
        Ok(observation) => observation,
        Err(()) => lifetime.abort(),
    }
}

fn execute(lifetime: &mut Lifetime) -> Result<SettledDoctorObservation, ()> {
    require_wait_policy()?;
    // A pidfd alone is not child ownership. Never signal an unvalidated fd.
    lifetime.authenticate()?;
    for fd in [0, 1, 2, 6, 7] {
        require_pipe(fd)?;
    }
    nonblocking(6)?;
    nonblocking(7)?;
    let request_input = acquire(3, wire::MAX_REQUEST_BYTES)?;
    let request = wire::Request::parse(request_input.bytes()).map_err(|_| ())?;
    #[cfg(target_arch = "x86_64")]
    let native = DoctorOfflineArchitecture::LinuxX86_64;
    #[cfg(target_arch = "aarch64")]
    let native = DoctorOfflineArchitecture::LinuxAarch64;
    if request.architecture != native {
        return Err(());
    }
    let input = acquire(4, request.bundle_len)?;
    if input.bytes().len() != request.bundle_len
        || <[u8; 32]>::from(Sha256::digest(input.bytes())) != request.bundle_digest
    {
        return Err(());
    }
    let bundle = DoctorOfflineBundle::parse(input, &request.selector).map_err(|_| ())?;
    if bundle.architecture() != native {
        return Err(());
    }
    let target = match request.target {
        0 => DoctorOfflineTarget::Contributor,
        1 => DoctorOfflineTarget::Native,
        2 => DoctorOfflineTarget::Web,
        3 => DoctorOfflineTarget::All,
        _ => return Err(()),
    };
    let mut paths = Vec::new();
    paths.try_reserve_exact(3).map_err(|_| ())?;
    for (role, tool) in request.roles() {
        let file = bundle.tool(tool).ok_or(())?;
        let length = file.path().len().checked_add(1).ok_or(())?;
        let mut path = String::new();
        path.try_reserve_exact(length).map_err(|_| ())?;
        path.push('/');
        path.push_str(file.path());
        paths.push((role, tool, path));
    }
    let reply = capture::collect(lifetime)?;
    let rows = wire::validate_reply(&request, &reply).map_err(|_| ())?;
    if rows.len() != paths.len() {
        return Err(());
    }
    let mut tools = Vec::new();
    tools.try_reserve_exact(rows.len()).map_err(|_| ())?;
    for ((role, output), (expected, tool, path)) in rows.into_iter().zip(paths) {
        if role != expected {
            return Err(());
        }
        tools.push(SettledDoctorTool { tool, path, output });
    }
    lifetime.require_time()?;
    // All allocations and validation precede closure; the opaque result is
    // constructed only after every transferred authority handle closes once.
    lifetime.complete();
    Ok(SettledDoctorObservation {
        selector: request.selector,
        architecture: native,
        target,
        tools,
    })
}

fn acquire(fd: i32, limit: usize) -> Result<DoctorOfflineInput, ()> {
    if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return Err(());
    }
    let file = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    DoctorOfflineInput::acquire(&file, limit).map_err(|_| ())
}

fn require_wait_policy() -> Result<(), ()> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err(());
    }
    Ok(())
}
fn require_pipe(fd: i32) -> Result<(), ()> {
    if unsafe { libc::fcntl(fd, libc::F_GETPIPE_SZ) } < 0 {
        Err(())
    } else {
        Ok(())
    }
}
fn nonblocking(fd: i32) -> Result<(), ()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        Err(())
    } else {
        Ok(())
    }
}
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}
fn stop() -> ! {
    unsafe { libc::_exit(126) }
}

pub(super) fn finish(report: &[u8], exit_code: u8) -> ! {
    report::finish(report, exit_code)
}
