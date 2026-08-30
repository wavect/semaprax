//! Pure classic-BPF interpretation: no kernel installation or tool execution.
use super::*;

fn evaluate(filter: &[libc::sock_filter], arch: u32, syscall: u32) -> u32 {
    let mut accumulator = 0;
    let mut pc = 0;
    for _ in 0..filter.len() {
        let instruction = &filter[pc];
        match instruction.code {
            LOAD => {
                accumulator = match instruction.k {
                    0 => syscall,
                    4 => arch,
                    _ => panic!("unexpected field"),
                }
            }
            EQUAL | BITS => {
                let yes = if instruction.code == EQUAL {
                    accumulator == instruction.k
                } else {
                    accumulator & instruction.k != 0
                };
                pc += usize::from(if yes { instruction.jt } else { instruction.jf });
            }
            RETURN => return instruction.k,
            _ => panic!("unexpected opcode"),
        }
        pc += 1;
        assert!(pc < filter.len());
    }
    panic!("filter failed to terminate")
}

#[test]
fn both_native_abis_deny_exact_policy_and_reject_foreign_invocation_abis() {
    for (arch, mask, denied) in [
        (X86_ARCH, 0x4000_0000, X86_DENIED),
        (ARM_ARCH, 0, ARM_DENIED),
    ] {
        let filter = policy(arch, mask, denied);
        assert_eq!(filter.len(), 23);
        for syscall in 0..=1024 {
            assert_eq!(
                evaluate(&filter, arch, syscall),
                if denied.contains(&syscall) {
                    DENY
                } else {
                    ALLOW
                }
            );
        }
        for foreign in [
            0,
            0x4000_0003,
            arch ^ 1,
            arch ^ 0x8000_0000,
            arch ^ 0x4000_0000,
        ] {
            assert_eq!(evaluate(&filter, foreign, 0), KILL);
            for syscall in denied {
                assert_eq!(evaluate(&filter, foreign, syscall), KILL);
            }
        }
        if mask != 0 {
            for syscall in [0, 41, 53, 425, 438, 1024] {
                assert_eq!(evaluate(&filter, arch, syscall | mask), KILL);
            }
        }
    }
}

#[test]
#[cfg(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn native_syscall_numbers_match_libc_and_setup_contract() {
    assert!(supported());
    let expected = if cfg!(target_arch = "x86_64") {
        X86_DENIED
    } else {
        ARM_DENIED
    };
    assert_eq!(
        expected,
        [
            libc::SYS_socket as u32,
            libc::SYS_socketpair as u32,
            libc::SYS_io_uring_setup as u32,
            libc::SYS_io_uring_enter as u32,
            libc::SYS_io_uring_register as u32,
            libc::SYS_pidfd_getfd as u32,
            libc::SYS_ptrace as u32,
            libc::SYS_process_vm_writev as u32
        ]
    );
    assert_eq!(KILL, libc::SECCOMP_RET_KILL_PROCESS);
    assert_eq!(ALLOW, libc::SECCOMP_RET_ALLOW);
    assert_eq!(DENY, libc::SECCOMP_RET_ERRNO | libc::EPERM as u32);
}
