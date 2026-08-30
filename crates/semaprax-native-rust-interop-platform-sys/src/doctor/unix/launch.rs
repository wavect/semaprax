//! Preallocated launch setup. Linux's child calls only async-signal-safe libc;
//! Darwin uses kernel-owned close-by-default spawn actions.
use super::{Fd, Prepared, ProbeError};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt as _;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux_socket_guard;

pub(super) struct Launch {
    path: CString,
    cwd: CString,
    environment: Vec<CString>,
    #[cfg(all(test, target_os = "linux"))]
    reject_socket_guard: bool,
}

impl Launch {
    pub(super) fn prepare(probe: &Prepared) -> Result<Self, ProbeError> {
        #[cfg(target_os = "linux")]
        if !linux_socket_guard::supported() {
            return Err(ProbeError::Unsupported);
        }
        let path =
            CString::new(probe.path.as_os_str().as_bytes()).map_err(|_| ProbeError::Invalid)?;
        let cwd =
            CString::new(probe.cwd.as_os_str().as_bytes()).map_err(|_| ProbeError::Invalid)?;
        let environment = probe
            .environment
            .iter()
            .map(|(key, value)| {
                let mut entry = key.as_os_str().as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_os_str().as_bytes());
                CString::new(entry).map_err(|_| ProbeError::Invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            path,
            cwd,
            environment,
            #[cfg(all(test, target_os = "linux"))]
            reject_socket_guard: probe.injected(super::super::Fault::SocketGuard),
        })
    }

    #[cfg(target_os = "linux")]
    pub(super) fn spawn(
        &self,
        stdout: &Fd,
        stderr: &Fd,
        null: &Fd,
    ) -> Result<(libc::pid_t, bool), ProbeError> {
        let argument = c"--version";
        let argv = [self.path.as_ptr(), argument.as_ptr(), std::ptr::null()];
        let mut environment = self
            .environment
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>();
        environment.push(std::ptr::null());
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(ProbeError::Spawn);
        }
        if pid == 0 {
            unsafe {
                if libc::setpgid(0, 0) != 0
                    || libc::chdir(self.cwd.as_ptr()) != 0
                    || libc::dup2(null.raw(), 0) < 0
                    || libc::dup2(stdout.raw(), 1) < 0
                    || libc::dup2(stderr.raw(), 2) < 0
                {
                    libc::_exit(126);
                }
                if libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, 0_u32) != 0 {
                    libc::_exit(126)
                }
                if !linux_socket_guard::install(
                    #[cfg(test)]
                    self.reject_socket_guard,
                ) {
                    libc::_exit(126)
                }
                libc::execve(self.path.as_ptr(), argv.as_ptr(), environment.as_ptr());
                libc::_exit(127);
            }
        }
        Ok((pid, false))
    }

    #[cfg(target_os = "macos")]
    pub(super) fn spawn(
        &self,
        stdout: &Fd,
        stderr: &Fd,
        null: &Fd,
    ) -> Result<(libc::pid_t, bool), ProbeError> {
        darwin::spawn(self, stdout, stderr, null)
    }
}

pub(super) fn pipe() -> Result<(Fd, Fd), ProbeError> {
    let mut raw = [-1; 2];
    if unsafe { libc::pipe(raw.as_mut_ptr()) } != 0 {
        return Err(ProbeError::Spawn);
    }
    let read = Fd(Some(raw[0]));
    let write = Fd(Some(raw[1]));
    Ok((above_stdio(read)?, above_stdio(write)?))
}

pub(super) fn null() -> Result<Fd, ProbeError> {
    let raw = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if raw < 0 {
        return Err(ProbeError::Spawn);
    }
    above_stdio(Fd(Some(raw)))
}

fn above_stdio(fd: Fd) -> Result<Fd, ProbeError> {
    if fd.raw() >= 3 {
        if unsafe { libc::fcntl(fd.raw(), libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            return Err(ProbeError::Spawn);
        }
        return Ok(fd);
    }
    let raw = unsafe { libc::fcntl(fd.raw(), libc::F_DUPFD_CLOEXEC, 3) };
    if raw < 0 {
        return Err(ProbeError::Spawn);
    }
    let duplicate = Fd(Some(raw));
    if fd.close().is_err() {
        std::process::abort()
    }
    Ok(duplicate)
}
