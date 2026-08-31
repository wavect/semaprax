//! Allocation-free child branch; no failure returns to the cloned Rust caller.
use super::{close, exec, map, stop, Prepared};

pub(super) fn enter(prepared: &Prepared) -> ! {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0 {
        stop();
    }
    let mut parent = libc::pollfd {
        fd: prepared.parent,
        events: libc::POLLIN,
        revents: 0,
    };
    if unsafe { libc::poll(&mut parent, 1, 0) } != 0 {
        stop();
    }
    // Every occupied target is closed with an observed result before mapping.
    // In particular no dup3 silently consumes a standard pipe or image handle.
    for fd in 0..=6 {
        if close(fd).is_err() {
            stop();
        }
    }
    for (destination, source) in [
        prepared.input,
        prepared.reply[1],
        prepared.error[1],
        prepared.request,
        prepared.bundle,
    ]
    .into_iter()
    .enumerate()
    {
        if map(source, destination as i32).is_err() {
            stop();
        }
    }
    for fd in prepared.inventory() {
        if fd != prepared.worker && close(fd).is_err() {
            stop();
        }
    }
    // The worker's held executable is the sole descriptor above4. Its CLOEXEC
    // close is part of trusted successful ELF startup, not endpoint cleanup.
    exec(prepared.worker, c"semaprax-doctor-worker");
    stop()
}
