//! Allocation-free child setup before the held launcher image.
use super::{close, Prepared};

pub(super) fn enter(prepared: &Prepared) -> ! {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0
        || unsafe { libc::getppid() } != prepared.parent
    {
        stop();
    }
    let mut ready = 0u8;
    if unsafe { libc::read(prepared.ready[0], (&mut ready as *mut u8).cast(), 1) } != 1
        || ready != b'1'
    {
        stop();
    }
    if unsafe {
        libc::mount(
            std::ptr::null(),
            c"/".as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    } != 0
        || unsafe { libc::sethostname(c"semaprax-doctor".as_ptr().cast(), 15) } != 0
        || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || !signals()
    {
        stop();
    }
    // Every destination is known-live on entry. Close it explicitly so dup3
    // never hides a destination-close failure.
    for fd in 0..=10 {
        if close(fd).is_err() {
            stop();
        }
    }
    for (source, destination) in [
        (prepared.stdin[0], 0),
        (prepared.report[1], 1),
        (prepared.error[1], 2),
        (prepared.request, 3),
        (prepared.bundle, 4),
        (prepared.worker, 5),
        (prepared.collector, 6),
    ] {
        if unsafe { libc::dup3(source, destination, 0) } != destination {
            stop();
        }
    }
    for fd in prepared.inventory() {
        if fd != prepared.launcher && close(fd).is_err() {
            stop();
        }
    }
    if unsafe { libc::getppid() } != prepared.parent {
        stop();
    }
    let argv = [c"semaprax-doctor-launcher".as_ptr(), std::ptr::null()];
    let environment: [*const libc::c_char; 1] = [std::ptr::null()];
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            prepared.launcher,
            c"".as_ptr(),
            argv.as_ptr(),
            environment.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
    }
    stop()
}

fn signals() -> bool {
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = libc::SIG_DFL;
    if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
        return false;
    }
    for signal in 1..=64 {
        if signal == libc::SIGKILL || signal == libc::SIGSTOP || signal == 32 || signal == 33 {
            continue;
        }
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
            return false;
        }
    }
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, &action.sa_mask, std::ptr::null_mut()) == 0 }
}

fn stop() -> ! {
    unsafe { libc::_exit(126) }
}
