//! Inherited socket/syscall denial, NOT complete no-network isolation.
//! Filesystem-mediated networking and external brokers remain separate policy.
//! This filter deliberately permits other trusted-tool runtime syscalls.

const LOAD: u16 = 0x20; // BPF_LD | BPF_W | BPF_ABS
const EQUAL: u16 = 0x15; // BPF_JMP | BPF_JEQ | BPF_K
const BITS: u16 = 0x45; // BPF_JMP | BPF_JSET | BPF_K
const RETURN: u16 = 0x06; // BPF_RET | BPF_K
const KILL: u32 = 0x8000_0000;
const DENY: u32 = 0x0005_0000 | libc::EPERM as u32;
const ALLOW: u32 = 0x7fff_0000;
const X86_ARCH: u32 = 0xc000_003e;
const ARM_ARCH: u32 = 0xc000_00b7;
// socket, socketpair, io_uring_setup/enter/register, pidfd_getfd, ptrace,
// process_vm_writev. Numbers are native Linux syscall ABIs, not host guesses.
const X86_DENIED: [u32; 8] = [41, 53, 425, 426, 427, 438, 101, 311];
const ARM_DENIED: [u32; 8] = [198, 199, 425, 426, 427, 438, 117, 271];

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

const fn policy(arch: u32, x32: u32, denied: [u32; 8]) -> [libc::sock_filter; 23] {
    let mut filter = [instruction(RETURN, ALLOW, 0, 0); 23];
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
    filter
}

/// Called only in the fork child after descriptor exclusion, before execve.
/// No allocation, formatting, locks, or fallible cleanup is performed here.
pub(super) unsafe fn install(#[cfg(test)] reject: bool) -> bool {
    if !supported() {
        return false;
    }
    let mut filter = if cfg!(target_arch = "x86_64") {
        policy(X86_ARCH, 0x4000_0000, X86_DENIED)
    } else {
        policy(ARM_ARCH, 0, ARM_DENIED)
    };
    let length = 23;
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
