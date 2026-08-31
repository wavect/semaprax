//! Complete byte/storage preflight before creating pipes or a worker.
use crate::doctor::{
    offline_bundle::elf, offline_worker::wire, DoctorOfflineArchitecture, DoctorOfflineBundle,
    DoctorOfflineInput, DOCTOR_OFFLINE_INPUT_MAX_BYTES,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::mem::ManuallyDrop;
use std::os::fd::{AsRawFd, FromRawFd};

pub(super) fn validate() -> Result<(), ()> {
    for fd in 0..=6 {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(());
        }
    }
    for fd in 0..=2 {
        if unsafe { libc::fcntl(fd, libc::F_GETPIPE_SZ) } < 0 {
            return Err(());
        }
    }
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err(());
    }
    // Fixed descriptors belong to this process by the unsafe entry contract.
    // Borrow for validation; the launcher later owns explicit closure/remapping.
    let files = [3, 4, 5, 6].map(|fd| ManuallyDrop::new(unsafe { File::from_raw_fd(fd) }));
    validate_files(&files[0], &files[1], &files[2], &files[3])
}

fn native() -> DoctorOfflineArchitecture {
    if cfg!(target_arch = "x86_64") {
        DoctorOfflineArchitecture::LinuxX86_64
    } else {
        DoctorOfflineArchitecture::LinuxAarch64
    }
}

fn validate_files(
    request: &File,
    bundle: &File,
    worker: &File,
    collector: &File,
) -> Result<(), ()> {
    let request_input =
        DoctorOfflineInput::acquire(request, wire::MAX_REQUEST_BYTES).map_err(|_| ())?;
    let request = wire::Request::parse(request_input.bytes()).map_err(|_| ())?;
    if request.architecture != native() {
        return Err(());
    }
    let input = DoctorOfflineInput::acquire(bundle, request.bundle_len).map_err(|_| ())?;
    if input.bytes().len() != request.bundle_len
        || <[u8; 32]>::from(Sha256::digest(input.bytes())) != request.bundle_digest
    {
        return Err(());
    }
    let bundle = DoctorOfflineBundle::parse(input, &request.selector).map_err(|_| ())?;
    for (_, tool) in request.roles() {
        if bundle.tool(tool).is_none() {
            return Err(());
        }
    }
    drop(bundle);
    // Image snapshots are validated and dropped one at a time. The byte cap is
    // per input, not a promise about aggregate memory or executable loadability.
    validate_image(worker)?;
    validate_image(collector)
}

fn validate_image(file: &File) -> Result<(), ()> {
    // Seal-first acquisition prevents arbitrary supplied filesystem reads.
    let input =
        DoctorOfflineInput::acquire(file, DOCTOR_OFFLINE_INPUT_MAX_BYTES).map_err(|_| ())?;
    let fd = file.as_raw_fd();
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    if seals < 0 || seals & libc::F_SEAL_EXEC == 0 {
        return Err(());
    }
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(fd, metadata.as_mut_ptr()) } != 0 {
        return Err(());
    }
    let mode = unsafe { metadata.assume_init() }.st_mode;
    if mode & 0o111 == 0 || mode & (libc::S_ISUID | libc::S_ISGID) != 0 {
        return Err(());
    }
    // Reuse minimum native ELF framing; this does not attest PT_INTERP/library
    // closure, approved image role, capabilities, or binfmt dispatch policy.
    elf::validate(input.bytes(), native()).map_err(|_| ())?;
    Ok(())
}

#[cfg(test)]
mod tests;
