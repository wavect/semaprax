//! Default-deny syscall policy for the provisioned, single-process tool child.
//!
//! Root/cwd confinement, clean owned descriptors, capability removal, immutable
//! inputs, resource limits and parent-death ownership MUST precede installation.
//! This policy does not inspect pointer contents or authenticate those premises.
use super::Error;

const LOAD: u16 = 0x20;
const EQUAL: u16 = 0x15;
const BITS: u16 = 0x45;
const MASK: u16 = 0x54;
const RETURN: u16 = 0x06;
const KILL: u32 = 0x8000_0000;
const DENY: u32 = 0x0005_0000 | libc::EPERM as u32;
const ALLOW: u32 = 0x7fff_0000;
const X86_ARCH: u32 = 0xc000_003e;
const ARM_ARCH: u32 = 0xc000_00b7;
const CAPACITY: usize = 256;

// Linux native syscall ABIs: arch/x86/entry/syscalls/syscall_64.tbl and
// include/uapi/asm-generic/unistd.h. AArch64 has no legacy open/access/readlink.
const X86_BASELINE: &[u32] = &[
    0, 19, 17, 3, 5, 262, 332, 8, 79, 89, 267, 21, 269, 439, 12, 9, 10, 11, 25, 28, 13, 14, 15,
    131, 228, 96, 35, 230, 39, 110, 186, 102, 107, 104, 108, 63, 24, 204, 202, 218, 273, 334, 158,
    318, 60, 231, 59,
];
const ARM_BASELINE: &[u32] = &[
    63, 65, 67, 57, 80, 79, 291, 62, 17, 78, 48, 439, 214, 222, 226, 215, 216, 233, 134, 135, 139,
    132, 113, 169, 101, 115, 172, 173, 178, 174, 175, 176, 177, 160, 124, 123, 98, 96, 99, 293,
    278, 93, 94, 221,
];

pub(super) struct Guard {
    filter: Vec<libc::sock_filter>,
}

impl Guard {
    pub(super) fn prepare() -> Result<Self, Error> {
        Self::for_arch(if cfg!(target_arch = "x86_64") {
            X86_ARCH
        } else {
            ARM_ARCH
        })
    }

    fn for_arch(arch: u32) -> Result<Self, Error> {
        let (baseline, open, openat, write, writev, prctl, prlimit, open_flags) = match arch {
            X86_ARCH => (X86_BASELINE, Some(2), 257, 1, 20, 157, 302, 0xb8800),
            ARM_ARCH => (ARM_BASELINE, None, 56, 64, 66, 167, 261, 0xac800),
            _ => return Err(Error::Invalid),
        };
        // O_RDONLY (zero), CLOEXEC, NONBLOCK, DIRECTORY, NOFOLLOW, LARGEFILE.
        // AArch64 overrides the last three architecture-specific flag bits.
        let mut filter = Vec::new();
        filter
            .try_reserve_exact(CAPACITY)
            .map_err(|_| Error::Allocation)?;
        filter.extend_from_slice(&[
            ins(LOAD, 4, 0, 0),
            ins(EQUAL, arch, 1, 0),
            ins(RETURN, KILL, 0, 0),
            ins(LOAD, 0, 0, 0),
        ]);
        if arch == X86_ARCH {
            filter.extend_from_slice(&[ins(BITS, 0x4000_0000, 0, 1), ins(RETURN, KILL, 0, 0)]);
        }
        // arch_prctl is x86-only process-local FS/GS/runtime state; it grants no
        // files, namespaces, capabilities, process creation or IPC authority.
        for number in baseline {
            rule(&mut filter, *number, &[ins(RETURN, ALLOW, 0, 0)])?;
        }
        for (number, argument) in open.into_iter().map(|n| (n, 1)).chain([(openat, 2)]) {
            rule(
                &mut filter,
                number,
                &[
                    ins(LOAD, offset(argument) + 4, 0, 0),
                    ins(EQUAL, 0, 1, 0),
                    ins(RETURN, DENY, 0, 0),
                    ins(LOAD, offset(argument), 0, 0),
                    ins(MASK, !open_flags, 0, 0),
                    ins(EQUAL, 0, 1, 0),
                    ins(RETURN, DENY, 0, 0),
                    ins(RETURN, ALLOW, 0, 0),
                ],
            )?;
        }
        for number in [write, writev] {
            rule(
                &mut filter,
                number,
                &[
                    ins(LOAD, offset(0) + 4, 0, 0),
                    ins(EQUAL, 0, 1, 0),
                    ins(RETURN, DENY, 0, 0),
                    ins(LOAD, offset(0), 0, 0),
                    ins(EQUAL, 1, 2, 0),
                    ins(EQUAL, 2, 1, 0),
                    ins(RETURN, DENY, 0, 0),
                    ins(RETURN, ALLOW, 0, 0),
                ],
            )?;
        }
        rule(
            &mut filter,
            prctl,
            &[
                ins(LOAD, offset(0) + 4, 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(LOAD, offset(0), 0, 0),
                ins(EQUAL, 15, 2, 0),
                ins(EQUAL, 16, 1, 0), // SET_NAME / GET_NAME
                ins(RETURN, DENY, 0, 0),
                ins(RETURN, ALLOW, 0, 0),
            ],
        )?;
        rule(
            &mut filter,
            prlimit,
            &[
                ins(LOAD, offset(2), 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(LOAD, offset(2) + 4, 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(RETURN, ALLOW, 0, 0),
            ],
        )?;
        if filter.len() >= CAPACITY {
            return Err(Error::Limit);
        }
        filter.push(ins(RETURN, DENY, 0, 0));
        Ok(Self { filter })
    }

    /// Install only in the exclusively owned child, after all setup syscalls.
    /// No allocation or cleanup occurs. Failure must never enter the tool.
    pub(super) unsafe fn install(&self) -> bool {
        let program = libc::sock_fprog {
            len: self.filter.len() as u16,
            // The kernel copies, never writes, this vector during installation.
            filter: self.filter.as_ptr().cast_mut(),
        };
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
}

const fn ins(code: u16, k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    libc::sock_filter { code, k, jt, jf }
}
const fn offset(argument: u32) -> u32 {
    16 + 8 * argument
}
fn rule(
    filter: &mut Vec<libc::sock_filter>,
    number: u32,
    body: &[libc::sock_filter],
) -> Result<(), Error> {
    let skip = u8::try_from(body.len()).map_err(|_| Error::Limit)?;
    if filter.len() + 1 + body.len() >= CAPACITY {
        return Err(Error::Limit);
    }
    filter.push(ins(EQUAL, number, 0, skip));
    filter.extend_from_slice(body);
    Ok(())
}

#[cfg(test)]
mod tests;
