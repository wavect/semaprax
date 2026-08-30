//! Inherited socket/syscall denial, NOT complete no-network isolation.
//! Filesystem-mediated networking and external brokers remain separate policy.
//! This filter deliberately permits other trusted-tool runtime syscalls.

const LOAD: u16 = 0x20; // BPF_LD | BPF_W | BPF_ABS
const EQUAL: u16 = 0x15; // BPF_JMP | BPF_JEQ | BPF_K
const BITS: u16 = 0x45; // BPF_JMP | BPF_JSET | BPF_K
const MASK: u16 = 0x54; // BPF_ALU | BPF_AND | BPF_K
const RETURN: u16 = 0x06; // BPF_RET | BPF_K
const KILL: u32 = 0x8000_0000;
const DENY: u32 = 0x0005_0000 | libc::EPERM as u32;
const ALLOW: u32 = 0x7fff_0000;
const X86_ARCH: u32 = 0xc000_003e;
const ARM_ARCH: u32 = 0xc000_00b7;
// socket, io_uring_setup/enter/register, pidfd_getfd, ptrace,
// process_vm_writev, connect, bind, listen, accept, accept4. Numbers are native
// Linux syscall ABIs, not host guesses. Anonymous pairs are admitted separately.
const X86_DENIED: [u32; 12] = [41, 425, 426, 427, 438, 101, 311, 42, 49, 50, 43, 288];
const ARM_DENIED: [u32; 12] = [198, 425, 426, 427, 438, 117, 271, 203, 200, 201, 202, 242];
const X86_PAIR: u32 = 53;
const ARM_PAIR: u32 = 199;
const PAIR_FLAGS: u32 = 0x80000 | 0x800; // SOCK_CLOEXEC | SOCK_NONBLOCK
const POLICY_LEN: usize = 53;

pub(super) const fn supported() -> bool {
    cfg!(all(
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))
}

const fn instruction(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, jt, jf, k }
}

const fn policy(
    arch: u32,
    x32: u32,
    denied: [u32; 12],
    socketpair: u32,
) -> [libc::sock_filter; POLICY_LEN] {
    let mut filter = [instruction(RETURN, ALLOW, 0, 0); POLICY_LEN];
    // seccomp_data: nr at offset 0, arch at offset 4. Never decode a syscall
    // number until the invocation ABI is authenticated.
    filter[0] = instruction(LOAD, 4, 0, 0);
    filter[1] = instruction(EQUAL, arch, 1, 0);
    filter[2] = instruction(RETURN, KILL, 0, 0);
    filter[3] = instruction(LOAD, 0, 0, 0);
    filter[4] = instruction(BITS, x32, 0, 1);
    filter[5] = instruction(RETURN, KILL, 0, 0);
    let mut index = 0;
    while index < denied.len() {
        filter[6 + index * 2] = instruction(EQUAL, denied[index], 0, 1);
        filter[7 + index * 2] = instruction(RETURN, DENY, 0, 0);
        index += 1;
    }
    // Rust's fork/exec handshake needs an anonymous Unix socket pair. Do not
    // allow socket() or datagram pairs: only already-connected stream/seqpacket
    // endpoints, with no address-selection syscalls. Kernel argument pointers
    // remain kernel-validated; this filter inspects only scalar values.
    // seccomp_data.args starts at 16. The admitted ABIs are little-endian;
    // validate both halves instead of silently truncating caller-supplied bits.
    let pair = [
        instruction(EQUAL, socketpair, 1, 0),
        instruction(RETURN, ALLOW, 0, 0),
        instruction(LOAD, 16, 0, 0),
        instruction(EQUAL, 1, 1, 0), // AF_UNIX
        instruction(RETURN, DENY, 0, 0),
        instruction(LOAD, 20, 0, 0),
        instruction(EQUAL, 0, 1, 0),
        instruction(RETURN, DENY, 0, 0),
        instruction(LOAD, 24, 0, 0),
        instruction(MASK, !PAIR_FLAGS, 0, 0),
        instruction(EQUAL, 1, 2, 0), // SOCK_STREAM
        instruction(EQUAL, 5, 1, 0), // SOCK_SEQPACKET
        instruction(RETURN, DENY, 0, 0),
        instruction(LOAD, 28, 0, 0),
        instruction(EQUAL, 0, 1, 0),
        instruction(RETURN, DENY, 0, 0),
        instruction(LOAD, 32, 0, 0),
        instruction(EQUAL, 0, 1, 0), // protocol
        instruction(RETURN, DENY, 0, 0),
        instruction(LOAD, 36, 0, 0),
        instruction(EQUAL, 0, 1, 0),
        instruction(RETURN, DENY, 0, 0),
        instruction(RETURN, ALLOW, 0, 0),
    ];
    index = 0;
    while index < pair.len() {
        filter[6 + denied.len() * 2 + index] = pair[index];
        index += 1;
    }
    filter
}

/// Called only in the fork child after descriptor exclusion, before execve.
/// No allocation, formatting, locks, or fallible cleanup is performed here.
pub(super) unsafe fn install(#[cfg(test)] reject: bool) -> bool {
    if !supported() {
        return false;
    }
    let mut filter = if cfg!(target_arch = "x86_64") {
        policy(X86_ARCH, 0x4000_0000, X86_DENIED, X86_PAIR)
    } else {
        policy(ARM_ARCH, 0, ARM_DENIED, ARM_PAIR)
    };
    let length = POLICY_LEN as u16;
    #[cfg(test)]
    let length = if reject { 0 } else { length };
    // A test-only zero length exercises actual kernel filter rejection, not a
    // branch that skips installation and pretends the kernel failed.
    let program = libc::sock_fprog {
        len: length,
        filter: filter.as_mut_ptr(),
    };
    // These restrictions survive exec and descendants and cannot be relaxed.
    unsafe {
        libc::prctl(
            libc::PR_SET_NO_NEW_PRIVS,
            1 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        ) == 0
            && libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER as libc::c_ulong,
                &program as *const libc::sock_fprog as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) == 0
    }
}

#[cfg(test)]
mod tests;
