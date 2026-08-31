//! Async-signal-safe, allocation-free child setup; every rejection exits.
use super::{fail_stop, guard::Guard, offline_root};
use std::ffi::CStr;

/// The clone has a private address space and descriptor table. Provisioned
/// namespace authority and clean inherited descriptor ownership are required.
pub(super) unsafe fn enter(
    plan: &offline_root::Plan<'_>,
    guard: &Guard,
    path: &CStr,
    streams: [i32; 3],
    supervisor: i32,
) -> ! {
    if unsafe { libc::getpid() } != 1 {
        fail_stop();
    }
    // Arm immediately, then repeat after capability/credential preparation.
    if !parent_alive(supervisor) {
        fail_stop();
    }
    for (destination, source) in streams.into_iter().enumerate() {
        if unsafe { libc::dup2(source, destination as i32) } != destination as i32 {
            fail_stop();
        }
    }
    let root = match unsafe { offline_root::linux::materialize(plan) } {
        Ok(root) => root,
        Err(_) => fail_stop(),
    };
    if unsafe { libc::fchdir(root.as_raw_fd()) } != 0
        || unsafe { libc::chroot(c".".as_ptr()) } != 0
        || unsafe { libc::chdir(c"/".as_ptr()) } != 0
    {
        fail_stop();
    }
    unsafe { root.close() };
    if !limits() || !remove_capabilities() || !parent_alive(supervisor) {
        fail_stop();
    }
    // Exact inherited inventory is a provisioner precondition. All remaining
    // descriptors are this worker's pipes/pidfds, not foreign filesystem files.
    if unsafe { libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, 0_u32) } != 0 {
        fail_stop();
    }
    if !signals() || !unsafe { guard.install() } {
        fail_stop();
    }
    let argv = [path.as_ptr(), c"--version".as_ptr(), std::ptr::null()];
    let environment = [
        c"LANG=C".as_ptr(),
        c"LC_ALL=C".as_ptr(),
        c"RUSTUP_AUTO_INSTALL=0".as_ptr(),
        std::ptr::null(),
    ];
    unsafe { libc::execve(path.as_ptr(), argv.as_ptr(), environment.as_ptr()) };
    fail_stop()
}

fn parent_alive(supervisor: i32) -> bool {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0 {
        return false;
    }
    let mut event = libc::pollfd {
        fd: supervisor,
        events: libc::POLLIN,
        revents: 0,
    };
    // A pidfd pins identity even though getppid() is zero inside this PID ns.
    unsafe { libc::poll(&mut event, 1, 0) == 0 }
}

fn limits() -> bool {
    for (resource, value) in [
        (libc::RLIMIT_CORE, 0),
        (libc::RLIMIT_NOFILE, 64),
        (libc::RLIMIT_AS, 2 * 1024 * 1024 * 1024),
    ] {
        let limit = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        if unsafe { libc::setrlimit(resource, &limit) } != 0 {
            return false;
        }
    }
    true
}

fn signals() -> bool {
    let mut action = unsafe { std::mem::zeroed::<libc::sigaction>() };
    action.sa_sigaction = libc::SIG_DFL;
    if unsafe { libc::sigemptyset(&mut action.sa_mask) } != 0 {
        return false;
    }
    for signal in 1..=64 {
        if signal == libc::SIGKILL || signal == libc::SIGSTOP {
            continue;
        }
        // libc reserves two internal RT signals. The provisioner owns their
        // dispositions; kernel rt_sigaction below would require ABI-sized sets.
        if signal == 32 || signal == 33 {
            continue;
        }
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } != 0 {
            return false;
        }
    }
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, &action.sa_mask, std::ptr::null_mut()) == 0 }
}

#[repr(C)]
struct CapHeader {
    version: u32,
    pid: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct CapData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

fn remove_capabilities() -> bool {
    // NOROOT + NO_SETUID_FIXUP, both locked; KEEP_CAPS locked off; ambient
    // raising disabled and locked. Never leave UID0 exec able to restore caps.
    const SECUREBITS: u32 = 1 | 2 | 4 | 8 | 32 | 64 | 128;
    if unsafe { libc::prctl(libc::PR_CAP_AMBIENT, 4, 0, 0, 0) } != 0
        || unsafe { libc::prctl(libc::PR_SET_SECUREBITS, SECUREBITS, 0, 0, 0) } != 0
    {
        return false;
    }
    let mut ended = false;
    for capability in 0..=64 {
        let value = unsafe { libc::prctl(libc::PR_CAPBSET_READ, capability, 0, 0, 0) };
        if value < 0 {
            if super::errno() != libc::EINVAL {
                return false;
            }
            ended = true;
            break;
        }
        if capability == 64
            || unsafe { libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_CAPBSET_READ, capability, 0, 0, 0) } != 0
        {
            return false;
        }
    }
    if !ended {
        return false;
    }
    let header = CapHeader {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut data = [CapData {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe { libc::syscall(libc::SYS_capset, &header as *const CapHeader, data.as_ptr()) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_capget,
                &header as *const CapHeader,
                data.as_mut_ptr(),
            )
        } != 0
        || unsafe { libc::prctl(libc::PR_GET_SECUREBITS, 0, 0, 0, 0) } != SECUREBITS as i32
    {
        return false;
    }
    data.iter()
        .all(|value| value.effective == 0 && value.permitted == 0 && value.inheritable == 0)
}
