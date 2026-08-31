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

#[test]
fn complete_syscall_selection_is_default_deny_on_both_native_abis() {
    for arch in [X86_ARCH, ARM_ARCH] {
        let guard = Guard::for_arch(arch).unwrap();
        assert!(guard.filter.len() < CAPACITY);
        let x86 = arch == X86_ARCH;
        let baseline = BASELINE
            .iter()
            .map(|(_, x, a)| if x86 { *x } else { *a })
            .chain(if x86 { &[21, 89, 158][..] } else { &[] }.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        for (name, x, a) in BASELINE {
            assert_eq!(
                evaluate(&guard, arch, if x86 { *x } else { *a }, [u64::MAX; 6]),
                ALLOW,
                "{name}"
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
                "arch {arch:x} syscall {number}"
            );
        }
        // Explicitly pin security-sensitive denied families, including every
        // process/thread creation route and the newer pointer-based APIs.
        let denied: &[u32] = if x86 {
            &[
                16, 41, 53, 56, 57, 58, 101, 126, 160, 161, 165, 166, 272, 310, 311, 322, 425, 426,
                427, 435, 437, 438,
            ]
        } else {
            &[
                29, 198, 199, 220, 117, 91, 164, 51, 40, 39, 97, 270, 271, 281, 425, 426, 427, 435,
                437, 438,
            ]
        };
        for number in denied {
            assert_eq!(evaluate(&guard, arch, *number, [u64::MAX; 6]), DENY);
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

#[test]
fn readonly_open_checks_architecture_flags_and_every_scalar_bit() {
    for (arch, calls, flag_bits) in [
        (X86_ARCH, &[(2, 1), (257, 2)][..], [19, 11, 16, 17, 15]),
        (ARM_ARCH, &[(56, 2)][..], [19, 11, 14, 15, 17]),
    ] {
        let guard = Guard::for_arch(arch).unwrap();
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

#[test]
fn writes_are_exact_capture_descriptors_and_prctl_cannot_change_authority() {
    for (arch, writes, prctl) in [(X86_ARCH, [1, 20], 157), (ARM_ARCH, [64, 66], 167)] {
        let guard = Guard::for_arch(arch).unwrap();
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

#[test]
fn prlimit_queries_cannot_supply_any_new_limit_pointer() {
    for (arch, number) in [(X86_ARCH, 302), (ARM_ARCH, 261)] {
        let guard = Guard::for_arch(arch).unwrap();
        let mut args = [u64::MAX; 6];
        args[2] = 0;
        assert_eq!(evaluate(&guard, arch, number, args), ALLOW);
        for bit in 0..64 {
            args[2] = 1 << bit;
            assert_eq!(evaluate(&guard, arch, number, args), DENY);
        }
    }
    assert!(matches!(Guard::for_arch(0), Err(Error::Invalid)));
}
