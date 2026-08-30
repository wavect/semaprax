//! Pure classic-BPF interpretation: no kernel installation or tool execution.
use super::*;

fn evaluate(filter: &[libc::sock_filter], arch: u32, syscall: u32, args: [u64; 6]) -> u32 {
    let mut accumulator = 0;
    let mut pc = 0;
    for _ in 0..filter.len() {
        let instruction = &filter[pc];
        match instruction.code {
            LOAD => {
                accumulator = match instruction.k {
                    0 => syscall,
                    4 => arch,
                    offset @ 16..=60 if offset % 4 == 0 => {
                        let value = args[((offset - 16) / 8) as usize];
                        if offset % 8 == 0 {
                            value as u32
                        } else {
                            (value >> 32) as u32
                        }
                    }
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
            MASK => accumulator &= instruction.k,
            _ => panic!("unexpected opcode"),
        }
        pc += 1;
        assert!(pc < filter.len());
    }
    panic!("filter failed to terminate")
}

#[test]
fn both_native_abis_deny_exact_policy_and_reject_foreign_invocation_abis() {
    for (arch, mask, denied, pair) in [
        (X86_ARCH, 0x4000_0000, X86_DENIED, X86_PAIR),
        (ARM_ARCH, 0, ARM_DENIED, ARM_PAIR),
    ] {
        let filter = policy(arch, mask, denied, pair);
        assert_eq!(filter.len(), 53);
        for syscall in 0..=1024 {
            assert_eq!(
                evaluate(&filter, arch, syscall, [0; 6]),
                if denied.contains(&syscall) || syscall == pair {
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
            assert_eq!(evaluate(&filter, foreign, 0, [0; 6]), KILL);
            for syscall in denied.into_iter().chain([pair]) {
                assert_eq!(
                    evaluate(&filter, foreign, syscall, [1, 5, 0, 0, 0, 0]),
                    KILL
                );
            }
        }
        if mask != 0 {
            for syscall in [0, 41, 53, 425, 438, 1024] {
                assert_eq!(
                    evaluate(&filter, arch, syscall | mask, [1, 5, 0, 0, 0, 0]),
                    KILL
                );
            }
        }
    }
}

// Literal independent admissible set, including both permitted flag bits.
fn pair_allowed(args: [u64; 6]) -> bool {
    args[0] == 1
        && args[2] == 0
        && matches!(
            args[1],
            1 | 5 | 0x801 | 0x805 | 0x80001 | 0x80005 | 0x80801 | 0x80805
        )
}

#[test]
fn anonymous_pairs_validate_complete_arguments_without_opening_datagrams_or_named_peers() {
    for (arch, mask, denied, pair) in [
        (X86_ARCH, 0x4000_0000, X86_DENIED, X86_PAIR),
        (ARM_ARCH, 0, ARM_DENIED, ARM_PAIR),
    ] {
        let filter = policy(arch, mask, denied, pair);
        for kind in [1, 5, 0x801, 0x805, 0x80001, 0x80005, 0x80801, 0x80805] {
            let args = [1, kind, 0, u64::MAX, u64::MAX, u64::MAX];
            assert_eq!(evaluate(&filter, arch, pair, args), ALLOW);
            // Every scalar bit is relevant, including bits which the kernel's
            // int arguments would otherwise truncate. The output pointer and
            // unused arguments deliberately do not influence this policy.
            for argument in 0..3 {
                for bit in 0..64 {
                    let mut mutated = args;
                    mutated[argument] ^= 1_u64 << bit;
                    assert_eq!(
                        evaluate(&filter, arch, pair, mutated),
                        if pair_allowed(mutated) { ALLOW } else { DENY }
                    );
                }
            }
            for syscall in denied {
                assert_eq!(evaluate(&filter, arch, syscall, args), DENY);
            }
            // Non-pair calls must not accidentally load/interpret pair arguments.
            for syscall in 0..=1024 {
                if syscall != pair && !denied.contains(&syscall) {
                    assert_eq!(evaluate(&filter, arch, syscall, [u64::MAX; 6]), ALLOW);
                }
            }
        }
        for family in [0, 2, 10, 16, 17, 30, u64::MAX] {
            assert_eq!(evaluate(&filter, arch, pair, [family, 1, 0, 0, 0, 0]), DENY);
        }
        for kind in [0, 2, 3, 4, 6, 10, 0x80002, 0x80802, u64::MAX] {
            assert_eq!(evaluate(&filter, arch, pair, [1, kind, 0, 0, 0, 0]), DENY);
        }
        for protocol in [1, 6, 17, u32::MAX as u64, u64::MAX] {
            assert_eq!(
                evaluate(&filter, arch, pair, [1, 1, protocol, 0, 0, 0]),
                DENY
            );
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
            libc::SYS_io_uring_setup as u32,
            libc::SYS_io_uring_enter as u32,
            libc::SYS_io_uring_register as u32,
            libc::SYS_pidfd_getfd as u32,
            libc::SYS_ptrace as u32,
            libc::SYS_process_vm_writev as u32,
            libc::SYS_connect as u32,
            libc::SYS_bind as u32,
            libc::SYS_listen as u32,
            libc::SYS_accept as u32,
            libc::SYS_accept4 as u32
        ]
    );
    assert_eq!(
        if cfg!(target_arch = "x86_64") {
            X86_PAIR
        } else {
            ARM_PAIR
        },
        libc::SYS_socketpair as u32
    );
    assert_eq!(
        PAIR_FLAGS,
        (libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK) as u32
    );
    assert_eq!(
        (libc::AF_UNIX, libc::SOCK_STREAM, libc::SOCK_SEQPACKET),
        (1, 1, 5)
    );
    assert_eq!(KILL, libc::SECCOMP_RET_KILL_PROCESS);
    assert_eq!(ALLOW, libc::SECCOMP_RET_ALLOW);
    assert_eq!(DENY, libc::SECCOMP_RET_ERRNO | libc::EPERM as u32);
}
