//! Independent argument oracles over the actual production BPF vectors.
//! These tests neither install seccomp nor execute a tool.
use super::*;

fn evaluate(guard: &Guard, arch: u32, number: u32, args: [u64; 6]) -> u32 {
    let mut accumulator = 0;
    let mut pc = 0;
    for _ in 0..guard.filter.len() {
        let op = &guard.filter[pc];
        match op.code {
            LOAD => {
                accumulator = match op.k {
                    0 => number,
                    4 => arch,
                    offset @ 16..=60 if offset % 4 == 0 => {
                        let argument = args[((offset - 16) / 8) as usize];
                        if offset % 8 == 0 {
                            argument as u32
                        } else {
                            (argument >> 32) as u32
                        }
                    }
                    _ => panic!("invalid seccomp-data load"),
                }
            }
            EQUAL | BITS => {
                let yes = if op.code == EQUAL {
                    accumulator == op.k
                } else {
                    accumulator & op.k != 0
                };
                pc += usize::from(if yes { op.jt } else { op.jf });
            }
            MASK => accumulator &= op.k,
            RETURN => return op.k,
            _ => panic!("unexpected BPF opcode"),
        }
        pc += 1;
        assert!(pc < guard.filter.len());
    }
    panic!("unterminated policy")
}

// Independent Linux syscall-number oracle, grouped by named operation family.
// Missing legacy calls on AArch64 are not mapped onto unrelated syscall slots.
const BASELINE: &[(&str, u32, u32)] = &[
    ("read", 0, 63),
    ("readv", 19, 65),
    ("pread64", 17, 67),
    ("close", 3, 57),
    ("fstat", 5, 80),
    ("newfstatat", 262, 79),
    ("statx", 332, 291),
    ("lseek", 8, 62),
    ("getcwd", 79, 17),
    ("readlinkat", 267, 78),
    ("faccessat", 269, 48),
    ("faccessat2", 439, 439),
    ("brk", 12, 214),
    ("mmap", 9, 222),
    ("mprotect", 10, 226),
    ("munmap", 11, 215),
    ("mremap", 25, 216),
    ("madvise", 28, 233),
    ("rt_sigaction", 13, 134),
    ("rt_sigprocmask", 14, 135),
    ("rt_sigreturn", 15, 139),
    ("sigaltstack", 131, 132),
    ("clock_gettime", 228, 113),
    ("gettimeofday", 96, 169),
    ("nanosleep", 35, 101),
    ("clock_nanosleep", 230, 115),
    ("getpid", 39, 172),
    ("getppid", 110, 173),
    ("gettid", 186, 178),
    ("getuid", 102, 174),
    ("geteuid", 107, 175),
    ("getgid", 104, 176),
    ("getegid", 108, 177),
    ("uname", 63, 160),
    ("sched_yield", 24, 124),
    ("sched_getaffinity", 204, 123),
    ("futex", 202, 98),
    ("set_tid_address", 218, 96),
    ("set_robust_list", 273, 99),
    ("rseq", 334, 293),
    ("getrandom", 318, 278),
    ("exit", 60, 93),
    ("exit_group", 231, 94),
    ("execve", 59, 221),
];

const TOOLS: [DoctorOfflineTool; 3] = [
    DoctorOfflineTool::Clang,
    DoctorOfflineTool::Node,
    DoctorOfflineTool::Rustc,
];

fn expected_role(tool: DoctorOfflineTool) -> u8 {
    match tool {
        DoctorOfflineTool::Clang => 1,
        DoctorOfflineTool::Node => 2,
        DoctorOfflineTool::Rustc => 4,
    }
}

#[test]
fn common_and_deny_inventories_are_exact_and_role_extensions_start_empty() {
    let expected_x86 = BASELINE
        .iter()
        .map(|(_, x86, _)| *x86)
        .chain([21, 89, 158])
        .collect::<std::collections::BTreeSet<_>>();
    let expected_arm = BASELINE
        .iter()
        .map(|(_, _, arm)| *arm)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        X86_COMMON
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        expected_x86
    );
    assert_eq!(
        ARM_COMMON
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        expected_arm
    );
    assert_eq!(
        X86_MANDATORY_DENY,
        &[
            16, 41, 42, 44, 49, 53, 56, 57, 58, 62, 101, 126, 155, 160, 161, 165, 166, 272, 310,
            311, 321, 322, 424, 425, 426, 427, 434, 435, 437, 438,
        ]
    );
    assert_eq!(
        ARM_MANDATORY_DENY,
        &[
            29, 39, 40, 41, 51, 91, 97, 117, 129, 164, 198, 199, 200, 203, 206, 220, 270, 271, 280,
            281, 424, 425, 426, 427, 434, 435, 437, 438,
        ]
    );
    assert!(X86_SAFE_ADDITIONS.is_empty());
    assert!(ARM_SAFE_ADDITIONS.is_empty());
    for policy in ROLE_POLICIES {
        assert!(policy.x86_additional.is_empty());
        assert!(policy.arm_additional.is_empty());
    }
}

#[test]
fn role_table_is_closed_single_role_only_and_rejects_union_widening() {
    assert_eq!(ROLE_POLICIES.len(), TOOLS.len());
    for (role, tool) in [(1, TOOLS[0]), (2, TOOLS[1]), (4, TOOLS[2])] {
        let policy = role_policy(role).unwrap();
        assert_eq!(policy.role, role);
        assert_eq!(policy.tool, tool);
        assert_eq!(expected_role(tool), role);
        for other in TOOLS {
            assert_eq!(
                Guard::for_arch(role, other, X86_ARCH).is_ok(),
                other == tool
            );
        }

        for bit in 0..8 {
            assert!(role_policy(role ^ (1 << bit)).is_err());
        }
    }
    for widened in [0, 3, 5, 6, 7, 8, u8::MAX] {
        assert!(role_policy(widened).is_err(), "role mask {widened}");
    }
}

#[test]
fn every_role_policy_preserves_the_shared_mandatory_deny_floor() {
    for (arch, common, floor) in [
        (X86_ARCH, X86_COMMON, X86_MANDATORY_DENY),
        (ARM_ARCH, ARM_COMMON, ARM_MANDATORY_DENY),
    ] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            let policy = role_policy(expected_role(tool)).unwrap();
            let additional = if arch == X86_ARCH {
                policy.x86_additional
            } else {
                policy.arm_additional
            };
            let safe = if arch == X86_ARCH {
                X86_SAFE_ADDITIONS
            } else {
                ARM_SAFE_ADDITIONS
            };
            assert!(validate_policy(common, additional, safe, floor, &[]).is_ok());
            for number in floor {
                assert_eq!(
                    evaluate(&guard, arch, *number, [u64::MAX; 6]),
                    DENY,
                    "{tool:?} arch {arch:x} mandatory deny {number}"
                );
                assert!(matches!(
                    validate_policy(common, &[*number], safe, floor, &[]),
                    Err(Error::Invalid)
                ));
            }
        }
    }
    for number in (0..=1024).chain([u32::MAX]) {
        assert!(matches!(
            validate_policy(&[], &[number], &[], &[], &[]),
            Err(Error::Invalid)
        ));
    }
    assert!(matches!(
        validate_policy(&[1], &[1], &[1], &[], &[]),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        validate_policy(&[], &[], &[], &[7], &[Some(7)]),
        Err(Error::Invalid)
    ));
    assert!(matches!(
        validate_policy(&[], &[], &[], &[], &[Some(7), Some(7)]),
        Err(Error::Invalid)
    ));
}

#[test]
fn exact_high_risk_syscalls_deny_for_every_role() {
    for (arch, numbers) in [
        (X86_ARCH, &[42, 49, 44, 62, 424, 434, 321, 155][..]),
        (ARM_ARCH, &[203, 200, 206, 129, 424, 434, 280, 41][..]),
    ] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            for number in numbers {
                assert_eq!(
                    evaluate(&guard, arch, *number, [u64::MAX; 6]),
                    DENY,
                    "{tool:?} arch {arch:x} high-risk syscall {number}"
                );
            }
        }
    }
}

#[test]
fn complete_syscall_selection_is_default_deny_on_both_native_abis() {
    for arch in [X86_ARCH, ARM_ARCH] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            assert!(guard.filter.len() < CAPACITY);
            let x86 = arch == X86_ARCH;
            let policy = role_policy(expected_role(tool)).unwrap();
            let baseline = BASELINE
                .iter()
                .map(|(_, x, a)| if x86 { *x } else { *a })
                .chain(if x86 { &[21, 89, 158][..] } else { &[] }.iter().copied())
                .chain(if x86 {
                    policy.x86_additional.iter().copied()
                } else {
                    policy.arm_additional.iter().copied()
                })
                .collect::<std::collections::BTreeSet<_>>();
            for (name, x, a) in BASELINE {
                assert_eq!(
                    evaluate(&guard, arch, if x86 { *x } else { *a }, [u64::MAX; 6]),
                    ALLOW,
                    "{tool:?} {name}"
                );
            }
            for number in 0..=1024 {
                let allowed_zero = baseline.contains(&number)
                    || if x86 {
                        matches!(number, 2 | 257 | 302)
                    } else {
                        matches!(number, 56 | 261)
                    };
                assert_eq!(
                    evaluate(&guard, arch, number, [0; 6]),
                    if allowed_zero { ALLOW } else { DENY },
                    "{tool:?} arch {arch:x} syscall {number}"
                );
            }
            for bit in 0..32 {
                assert_eq!(evaluate(&guard, arch ^ (1 << bit), 0, [0; 6]), KILL);
            }
            for number in [0, 2, 56, 59, 221, 435, 0x3fff_ffff] {
                assert_eq!(
                    evaluate(&guard, arch, number | 0x4000_0000, [0; 6]),
                    if x86 { KILL } else { DENY }
                );
            }
        }
    }
}

#[test]
fn readonly_open_checks_architecture_flags_and_every_scalar_bit() {
    for (arch, calls, flag_bits) in [
        (X86_ARCH, &[(2, 1), (257, 2)][..], [19, 11, 16, 17, 15]),
        (ARM_ARCH, &[(56, 2)][..], [19, 11, 14, 15, 17]),
    ] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            for (number, argument) in calls {
                for subset in 0..32 {
                    let flags = flag_bits
                        .iter()
                        .enumerate()
                        .fold(0_u64, |value, (index, bit)| {
                            value
                                | if subset & (1 << index) != 0 {
                                    1 << bit
                                } else {
                                    0
                                }
                        });
                    let mut args = [u64::MAX; 6];
                    args[*argument] = flags;
                    assert_eq!(evaluate(&guard, arch, *number, args), ALLOW);
                    for bit in 0..64 {
                        let mut mutated = args;
                        mutated[*argument] ^= 1 << bit;
                        assert_eq!(
                            evaluate(&guard, arch, *number, mutated),
                            if flag_bits.contains(&bit) {
                                ALLOW
                            } else {
                                DENY
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn writes_are_exact_capture_descriptors_and_prctl_cannot_change_authority() {
    for (arch, writes, prctl) in [(X86_ARCH, [1, 20], 157), (ARM_ARCH, [64, 66], 167)] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            for (numbers, permitted) in [
                (&writes[..], &[1_u64, 2][..]),
                (&[prctl][..], &[15, 16][..]),
            ] {
                for number in numbers {
                    for value in permitted {
                        let mut args = [u64::MAX; 6];
                        args[0] = *value;
                        assert_eq!(evaluate(&guard, arch, *number, args), ALLOW);
                        for bit in 0..64 {
                            let mut mutated = args;
                            mutated[0] ^= 1 << bit;
                            assert_eq!(
                                evaluate(&guard, arch, *number, mutated),
                                if permitted.contains(&mutated[0]) {
                                    ALLOW
                                } else {
                                    DENY
                                }
                            );
                        }
                    }
                    for value in [0, 3, 4, 8, 22, 24, 38, 47, u64::MAX] {
                        let mut args = [0; 6];
                        args[0] = value;
                        assert_eq!(evaluate(&guard, arch, *number, args), DENY);
                    }
                }
            }
        }
    }
}

#[test]
fn prlimit_queries_are_self_only_and_cannot_supply_any_new_limit_pointer() {
    for (arch, number) in [(X86_ARCH, 302), (ARM_ARCH, 261)] {
        for tool in TOOLS {
            let guard = Guard::for_arch(expected_role(tool), tool, arch).unwrap();
            let mut args = [u64::MAX; 6];
            args[0] = 0;
            args[2] = 0;
            assert_eq!(evaluate(&guard, arch, number, args), ALLOW);
            for bit in 0..64 {
                args[0] = 1 << bit;
                args[2] = 0;
                assert_eq!(evaluate(&guard, arch, number, args), DENY);
                args[0] = 0;
                args[2] = 1 << bit;
                assert_eq!(evaluate(&guard, arch, number, args), DENY);
            }
        }
    }
    for tool in TOOLS {
        assert!(matches!(
            Guard::for_arch(expected_role(tool), tool, 0),
            Err(Error::Invalid)
        ));
    }
}
