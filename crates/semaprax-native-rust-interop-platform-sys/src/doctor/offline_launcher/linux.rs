//! One dedicated process becomes the collector; no caller regains this scope.
use super::lifetime::Lifetime;

mod child;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
struct Prepared {
    request: i32,
    bundle: i32,
    worker: i32,
    collector: i32,
    input: i32,
    reply: [i32; 2],
    error: [i32; 2],
    parent: i32,
}

impl Prepared {
    fn inventory(&self) -> [i32; 10] {
        [
            self.request,
            self.bundle,
            self.worker,
            self.collector,
            self.input,
            self.reply[0],
            self.reply[1],
            self.error[0],
            self.error[1],
            self.parent,
        ]
    }
}

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

pub(super) fn entry() -> ! {
    if super::admission::validate().is_err() {
        stop();
    }
    let prepared = prepare().unwrap_or_else(|()| stop());
    let mut pidfd = -1i32;
    let arguments = CloneArgs {
        flags: libc::CLONE_PIDFD as u64,
        pidfd: (&mut pidfd as *mut i32) as u64,
        exit_signal: libc::SIGCHLD as u64,
        ..CloneArgs::default()
    };
    // Private address space/table, no thread or namespace flags and no atfork
    // callbacks. The externally supplied process is already single-threaded.
    let pid = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &arguments as *const CloneArgs,
            std::mem::size_of::<CloneArgs>(),
        )
    };
    if pid < 0 {
        stop();
    }
    if pid == 0 {
        child::enter(&prepared);
    }
    // This infallible constructor precedes every post-clone fallible action.
    // The child owns no copy of the pidfd returned into the parent's table.
    let mut lifetime = Lifetime::new(pidfd, pid as libc::pid_t);
    if handoff(&prepared, pidfd, &mut lifetime).is_err() {
        lifetime.abort();
    }
    // Successful exec cannot return or run Rust destructors. Any return,
    // regardless of errno, settles this one owned worker and never retries.
    lifetime.abort()
}

fn prepare() -> Result<Prepared, ()> {
    // These owned pipes survive collector exec even when an embedding
    // provisioner deliberately set CLOEXEC before calling the unsafe entry.
    for fd in 0..=2 {
        inherit(fd)?;
    }
    // Admission authenticates original 3..6. Retain same objects, not paths or
    // reconstructed carrier bytes; all future mapping sources are collision-free.
    let request = high(3)?;
    let bundle = high(4)?;
    let worker = high(5)?;
    let collector = high(6)?;
    let input = pipe()?;
    let reply = pipe()?;
    let error = pipe()?;
    close(input[1])?;
    // An inherited pidfd pins the parent across the child's PDEATHSIG setup
    // race without relying on numeric PID reuse or a particular PID namespace.
    let parent = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0u32) };
    if parent < 0 {
        return Err(());
    }
    let parent_high = high(parent as i32)?;
    close(parent as i32)?;
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
    {
        return Err(());
    }
    Ok(Prepared {
        request,
        bundle,
        worker,
        collector,
        input: input[0],
        reply,
        error,
        parent: parent_high,
    })
}

fn handoff(prepared: &Prepared, original_pidfd: i32, lifetime: &mut Lifetime) -> Result<(), ()> {
    let pidfd = high(original_pidfd)?;
    // Redirect first: if closing the previous descriptor fails, emergency
    // settlement still uses a known live duplicate, never the uncertain one.
    lifetime.redirect(pidfd);
    close(original_pidfd)?;
    for fd in 3..=6 {
        close(fd)?;
    }
    for (destination, source) in [
        (3, prepared.request),
        (4, prepared.bundle),
        (5, pidfd),
        (6, prepared.reply[0]),
        (7, prepared.error[0]),
    ] {
        map(source, destination)?;
        if destination == 5 {
            lifetime.redirect(5);
        }
    }
    close(pidfd)?;
    for fd in prepared.inventory() {
        if fd != prepared.collector {
            close(fd)?;
        }
    }
    // Exactly 0..7 and the CLOEXEC collector image now remain. Only that held
    // immutable image is closed by successful exec, not a hidden pipe writer.
    exec(prepared.collector, c"semaprax-doctor-collector");
    Err(())
}

fn pipe() -> Result<[i32; 2], ()> {
    let mut pair = [-1; 2];
    if unsafe { libc::pipe2(pair.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(());
    }
    let pinned = [high(pair[0])?, high(pair[1])?];
    close(pair[0])?;
    close(pair[1])?;
    Ok(pinned)
}

fn high(fd: i32) -> Result<i32, ()> {
    let result = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 64) };
    if result < 64 {
        Err(())
    } else {
        Ok(result)
    }
}

fn inherit(fd: i32) -> Result<(), ()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } != 0 {
        Err(())
    } else {
        Ok(())
    }
}

fn close(fd: i32) -> Result<(), ()> {
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        // No retry: a failure does not establish whether the fd was consumed.
        Err(())
    }
}

fn map(source: i32, destination: i32) -> Result<(), ()> {
    // The fixed destination was explicitly closed, or is the known vacant 7.
    // No live descriptor is silently closed by this successful transfer.
    if unsafe { libc::dup3(source, destination, 0) } == destination {
        Ok(())
    } else {
        Err(())
    }
}

fn exec(fd: i32, name: &std::ffi::CStr) {
    let argv = [name.as_ptr(), std::ptr::null()];
    let environment: [*const libc::c_char; 1] = [std::ptr::null()];
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            fd,
            c"".as_ptr(),
            argv.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
    }
}

fn stop() -> ! {
    unsafe { libc::_exit(126) }
}
