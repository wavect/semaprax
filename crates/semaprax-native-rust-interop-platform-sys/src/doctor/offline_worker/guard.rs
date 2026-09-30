//! Default-deny syscall policy for the provisioned, single-process tool child.
//!
//! Root/cwd confinement, clean owned descriptors, capability removal, immutable
//! inputs, resource limits and parent-death ownership MUST precede installation.
//! This policy does not inspect pointer contents or authenticate those premises.
use super::super::DoctorOfflineTool;
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
const X86_FCNTL: u32 = 72;
const ARM_FCNTL: u32 = 25;
const F_SETFD: u32 = 2;
const F_GETFL: u32 = 3;
const F_SETFL: u32 = 4;
const FD_CLOEXEC: u32 = 1;
const O_RDONLY_OR_NONBLOCK: u32 = 0x800;
// ENOSYS, not EPERM: glibc's thread creation retries through clone(2) only when
// clone3 reports that the kernel lacks it.
const UNAVAILABLE: u32 = 0x0005_0000 | libc::ENOSYS as u32;
const X86_CLONE: u32 = 56;
const ARM_CLONE: u32 = 220;
// clone3 has the same number on both native ABIs.
const CLONE3: u32 = 435;
// The exact flag word glibc 2.34+ `create_thread` passes for pthread_create,
// with a zero exit signal. Every bit keeps the new task inside the calling
// process: shared memory, descriptor table, filesystem context, signal
// handlers and thread group. No namespace, parent, pidfd, vfork, ptrace or
// exit-signal bit can be present, so this word cannot create a process.
const PTHREAD_CLONE_FLAGS: u32 = (libc::CLONE_VM
    | libc::CLONE_FS
    | libc::CLONE_FILES
    | libc::CLONE_SYSVSEM
    | libc::CLONE_SIGHAND
    | libc::CLONE_THREAD
    | libc::CLONE_SETTLS
    | libc::CLONE_PARENT_SETTID
    | libc::CLONE_CHILD_CLEARTID) as u32;
const DEFAULT_ADDRESS_SPACE_LIMIT: libc::rlim_t = 4 * 1024 * 1024 * 1024;
// Official x86-64 Node 22 builds enable V8's sandbox, whose 1 TiB reservation
// must itself be aligned to a 1 TiB boundary. The reservation path can map a
// 2 TiB candidate range and trim it after selecting the aligned subrange.
// RLIMIT_AS charges that transient mapping as well as the loader and ordinary
// process mappings, so a 2 TiB ceiling is still structurally too small. Hosted
// run 35568902945 confirmed `node --version` still terminated with SIGSEGV at
// exactly 2 TiB. Four TiB leaves one complete alignment-sized margin without
// making the virtual-address budget unbounded. The worker's output/time bounds
// and delegated cgroup still bound physical cost.
// Keep this role-local and finite: Clang and rustc retain the tighter ceiling.
const NODE_ADDRESS_SPACE_LIMIT: libc::rlim_t = 4 * 1024 * 1024 * 1024 * 1024;

// Linux native syscall ABIs: arch/x86/entry/syscalls/syscall_64.tbl and
// include/uapi/asm-generic/unistd.h. AArch64 has no legacy open/access/readlink.
const X86_COMMON: &[u32] = &[
    0, 19, 17, 3, 5, 262, 332, 8, 79, 89, 267, 21, 269, 439, 12, 9, 10, 11, 25, 28, 13, 14, 15,
    131, 228, 96, 35, 230, 39, 110, 186, 102, 107, 104, 108, 63, 24, 204, 202, 218, 273, 334, 158,
    318, 60, 231, 59,
];

// Real Node 22 and Rust 1.88 reach libuv/tokio event-loop primitives even for
// `--version`; the original inventory denied them with EPERM, which is why the
// real-distribution gate failed while the static clang and the synthetic
// fixtures passed. These are x86-only; AArch64 rows carry only what the AArch64
// trace below observed (issue #334).
//
// They are admitted through the role table rather than added to `X86_COMMON`,
// because that union is shared with clang -- a static binary that needs none
// of them -- and the role table's own contract says "later compatibility work
// must add a syscall to exactly one reviewed row rather than widen a union".
//
// Issue #270 removed two of the nineteen this originally carried, because
// neither is an event-loop primitive:
//
// - `accept4` (288) is provably inert here, not merely unused. `socket` (41)
//   and `socketpair` (53) are in `X86_MANDATORY_DENY`, `accept` (43) and
//   `listen` (50) are admitted by no role under a default-deny filter, and the
//   worker `dup2`s only its two `pipe2` descriptors to 3 and 4 before
//   `close_range(5.., CLOEXEC)`. No socket descriptor can exist, so `accept4`
//   had nothing it could ever operate on.
// - `perf_event_open` (298) is a real capability -- performance counters --
//   and nothing on a `--version` path reads them. Unlike `accept4` this has no
//   structural proof behind it; it is a judgement that a confinement allowlist
//   should not carry a counter syscall on the chance a loader wants it.
const X86_EVENT_LOOP: &[u32] = &[
    232, 233, 281, 283, 284, 286, 287, 290, 291, 292, 293, 294, 295, 296, 297,
];
// Hosted exact-head run 35582699426 narrowed the remaining real Rust 1.88
// startup failure to poll(2). Keep it out of Node and the shared inventory.
const X86_RUST_STARTUP: &[u32] = &[
    7, 232, 233, 281, 283, 284, 286, 287, 290, 291, 292, 293, 294, 295, 296, 297,
];
const ARM_COMMON: &[u32] = &[
    63, 65, 67, 57, 80, 79, 291, 62, 17, 78, 48, 439, 214, 222, 226, 215, 216, 233, 134, 135, 139,
    132, 113, 169, 101, 115, 172, 173, 178, 174, 175, 176, 177, 160, 124, 123, 98, 96, 99, 293,
    278, 93, 94, 221,
];

// A role-local syscall must first be admitted here for its exact native ABI.
// This is an independent second key: populating a RolePolicy row alone must
// never be enough to widen the executable filter, and `validate_policy`
// refuses a role addition that is not also listed here.
const X86_SAFE_ADDITIONS: &[u32] = X86_RUST_STARTUP;

// Hosted run 35591319262 passed Clang and Node and still failed Rust 1.88. Its
// unconfined `rustc --version` census shows one clone3 after the pipe2/fcntl
// pair: rustc_driver installs its Ctrl-C handler, whose watcher thread must
// start or rustc aborts. The shared floor answered EPERM, which glibc does not
// retry. Rustc alone receives ENOSYS for clone3 and the exact pthread clone(2)
// word; clang and Node keep the floor's EPERM for both.

// Issue #334, native AArch64 (glibc 2.36, Rust 1.88.0): the argument trace of
// `rustc --version` shows ppoll(73) over fds 0..2 with a zero timeout -- the
// AArch64 form of the x86 poll(2) above, since this ABI has no poll -- and
// pipe2(59, O_CLOEXEC) for the Ctrl-C self-pipe. Its fcntl and clone calls go
// through the argument-constrained rules below. Node 22 `--version` needs no
// AArch64 addition beyond its fcntl rule; clang needs none.
const ARM_RUST_STARTUP: &[u32] = &[73, 59];
const ARM_SAFE_ADDITIONS: &[u32] = ARM_RUST_STARTUP;

// A role row is the only route from an authenticated worker tool identity to
// syscall policy. Keep the rows explicit: compatibility work must add a
// syscall to exactly the reviewed rows that need it rather than widen a
// union. clang is a static binary with no event loop, Node carries the common
// event-loop set, and Rust adds only its separately observed poll startup call
// (ppoll and pipe2 on AArch64).
#[derive(Clone, Copy)]
struct RolePolicy {
    role: u8,
    tool: DoctorOfflineTool,
    address_space_limit: libc::rlim_t,
    x86_additional: &'static [u32],
    fcntl: FcntlPolicy,
    thread: ThreadPolicy,
    arm_additional: &'static [u32],
}

// `fcntl` is deliberately not an inventory addition: it is a descriptor
// authority multiplexer. Each nonempty row below has an independently traced,
// argument-constrained rule emitted only for that authenticated role. The
// traced commands and descriptors are identical on x86-64 (hosted run
// 35575666208) and AArch64 (issue #334), so one row serves both ABIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FcntlPolicy {
    None,
    Node,
    Rustc,
}

// clone and clone3 stay in the mandatory floor, so no inventory row can admit
// them. `Pthread` is the only route to a thread, emitted as argument-exact
// rules for an authenticated role on either native ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThreadPolicy {
    None,
    Pthread,
}

const ROLE_POLICIES: [RolePolicy; 3] = [
    RolePolicy {
        role: 1,
        tool: DoctorOfflineTool::Clang,
        address_space_limit: DEFAULT_ADDRESS_SPACE_LIMIT,
        x86_additional: &[],
        fcntl: FcntlPolicy::None,
        thread: ThreadPolicy::None,
        arm_additional: &[],
    },
    RolePolicy {
        role: 2,
        tool: DoctorOfflineTool::Node,
        address_space_limit: NODE_ADDRESS_SPACE_LIMIT,
        x86_additional: X86_EVENT_LOOP,
        // Hosted run 35575666208: F_GETFL on 0/1/2 and
        // F_SETFD(FD_CLOEXEC) on 0 through 16 only.
        fcntl: FcntlPolicy::Node,
        thread: ThreadPolicy::None,
        arm_additional: &[],
    },
    RolePolicy {
        role: 4,
        tool: DoctorOfflineTool::Rustc,
        address_space_limit: DEFAULT_ADDRESS_SPACE_LIMIT,
        x86_additional: X86_RUST_STARTUP,
        // Hosted run 35575666208 and the #334 AArch64 trace:
        // F_SETFL(O_RDONLY|O_NONBLOCK) on fd 4 only.
        fcntl: FcntlPolicy::Rustc,
        // Hosted run 35591319262 (clone3) and the #334 AArch64 trace (clone):
        // one exact pthread creation for the Ctrl-C watcher thread.
        thread: ThreadPolicy::Pthread,
        arm_additional: ARM_RUST_STARTUP,
    },
];

// This floor is shared by every role and is checked before filter emission.
// Default deny enforces it; retaining an explicit closed inventory prevents a
// future role-local compatibility edit from accidentally granting process,
// namespace, mount, tracing, cross-process memory, modern pointer-selected
// descriptor, or async-I/O authority. clone3 remains denied because classic
// BPF cannot inspect the pointed-to clone_args structure safely.
const X86_MANDATORY_DENY: &[u32] = &[
    16, 41, 42, 44, 49, 53, 56, 57, 58, 62, 101, 126, 155, 160, 161, 165, 166, 272, 310, 311, 321,
    322, 424, 425, 426, 427, 434, 435, 437, 438,
];
const ARM_MANDATORY_DENY: &[u32] = &[
    29, 39, 40, 41, 51, 91, 97, 117, 129, 164, 198, 199, 200, 203, 206, 220, 270, 271, 280, 281,
    424, 425, 426, 427, 434, 435, 437, 438,
];

pub(super) struct Guard {
    filter: Vec<libc::sock_filter>,
    address_space_limit: libc::rlim_t,
}

impl Guard {
    pub(super) fn prepare(role: u8, tool: DoctorOfflineTool) -> Result<Self, Error> {
        Self::for_arch(
            role,
            tool,
            if cfg!(target_arch = "x86_64") {
                X86_ARCH
            } else {
                ARM_ARCH
            },
        )
    }

    fn for_arch(role: u8, tool: DoctorOfflineTool, arch: u32) -> Result<Self, Error> {
        let policy = role_policy(role)?;
        if policy.tool != tool {
            return Err(Error::Invalid);
        }
        let (
            common,
            additional,
            safe_additional,
            deny,
            open,
            openat,
            write,
            writev,
            prctl,
            prlimit,
            fcntl,
            clone,
            open_flags,
        ) = match arch {
            X86_ARCH => (
                X86_COMMON,
                policy.x86_additional,
                X86_SAFE_ADDITIONS,
                X86_MANDATORY_DENY,
                Some(2),
                257,
                1,
                20,
                157,
                302,
                Some(X86_FCNTL),
                X86_CLONE,
                0xb8800 | 0x40000 | 0x200000, // allow O_NOATIME and O_PATH (real Node/Rust loader uses them)
            ),
            ARM_ARCH => (
                ARM_COMMON,
                policy.arm_additional,
                ARM_SAFE_ADDITIONS,
                ARM_MANDATORY_DENY,
                None,
                56,
                64,
                66,
                167,
                261,
                Some(ARM_FCNTL),
                ARM_CLONE,
                0xac800 | 0x40000 | 0x200000,
            ),
            _ => return Err(Error::Invalid),
        };
        let constrained = [
            open,
            Some(openat),
            Some(write),
            Some(writev),
            Some(prctl),
            Some(prlimit),
            fcntl,
        ];
        validate_policy(common, additional, safe_additional, deny, &constrained)?;
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
        for number in common.iter().chain(additional) {
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
                ins(LOAD, offset(0), 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(LOAD, offset(0) + 4, 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(LOAD, offset(2), 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(LOAD, offset(2) + 4, 0, 0),
                ins(EQUAL, 0, 1, 0),
                ins(RETURN, DENY, 0, 0),
                ins(RETURN, ALLOW, 0, 0),
            ],
        )?;
        if let Some(fcntl) = fcntl {
            match policy.fcntl {
                FcntlPolicy::None => {}
                FcntlPolicy::Node => node_fcntl_rule(&mut filter, fcntl)?,
                FcntlPolicy::Rustc => rustc_fcntl_rule(&mut filter, fcntl)?,
            }
        }
        if policy.thread == ThreadPolicy::Pthread {
            pthread_rules(&mut filter, clone)?;
        }
        if filter.len() >= CAPACITY {
            return Err(Error::Limit);
        }
        filter.push(ins(RETURN, DENY, 0, 0));
        Ok(Self {
            filter,
            address_space_limit: policy.address_space_limit,
        })
    }

    /// Finite virtual-address ceiling selected by the authenticated tool role.
    /// A larger reservation budget grants no new syscall or filesystem access
    /// and does not increase the delegated cgroup's physical-memory authority.
    pub(super) fn address_space_limit(&self) -> libc::rlim_t {
        self.address_space_limit
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

fn role_policy(role: u8) -> Result<&'static RolePolicy, Error> {
    ROLE_POLICIES
        .iter()
        .find(|policy| policy.role == role)
        .ok_or(Error::Invalid)
}

fn validate_policy(
    common: &[u32],
    additional: &[u32],
    safe_additional: &[u32],
    deny: &[u32],
    constrained: &[Option<u32>],
) -> Result<(), Error> {
    if additional
        .iter()
        .any(|number| !safe_additional.contains(number))
    {
        return Err(Error::Invalid);
    }
    for (index, number) in common.iter().chain(additional).enumerate() {
        if deny.contains(number)
            || common
                .iter()
                .chain(additional)
                .take(index)
                .any(|seen| seen == number)
        {
            return Err(Error::Invalid);
        }
    }
    for (index, number) in constrained.iter().flatten().enumerate() {
        if deny.contains(number)
            || common
                .iter()
                .chain(additional)
                .any(|allowed| allowed == number)
            || constrained
                .iter()
                .flatten()
                .take(index)
                .any(|seen| seen == number)
        {
            return Err(Error::Invalid);
        }
    }
    Ok(())
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

// The child closes every descriptor from 3 upward before installing this
// filter. Node nevertheless probes F_SETFD through 16; admitting those exact
// calls preserves the kernel's EBADF result for the observed startup topology.
// If a later already-admitted pipe2/dup3 call creates one of those descriptor
// numbers, this rule can only set its close-on-exec bit; it cannot duplicate,
// acquire, lock, lease, or otherwise make that descriptor more capable.
// F_GETFL is narrower because it was observed only for the three surviving
// standard streams.
fn node_fcntl_rule(filter: &mut Vec<libc::sock_filter>, number: u32) -> Result<(), Error> {
    rule(
        filter,
        number,
        &[
            ins(LOAD, offset(0) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(1) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(1), 0, 0),
            // F_GETFL has no third argument. Its observed descriptors are
            // exactly stdin/stdout/stderr, so do not turn it into a generic
            // descriptor-status oracle.
            ins(EQUAL, F_GETFL, 0, 6),
            ins(LOAD, offset(0), 0, 0),
            ins(EQUAL, 0, 3, 0),
            ins(EQUAL, 1, 2, 0),
            ins(EQUAL, 2, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(RETURN, ALLOW, 0, 0),
            // FD_CLOEXEC is the sole descriptor-flag mutation observed. The
            // explicit 0..=16 row keeps the closed-probe EBADF behaviour and
            // cannot grant F_DUPFD, ownership, locks, leases, seals or pipe
            // resizing through another fcntl command.
            ins(EQUAL, F_SETFD, 0, 24),
            ins(LOAD, offset(2) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(2), 0, 0),
            ins(EQUAL, FD_CLOEXEC, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(0), 0, 0),
            ins(EQUAL, 0, 17, 0),
            ins(EQUAL, 1, 16, 0),
            ins(EQUAL, 2, 15, 0),
            ins(EQUAL, 3, 14, 0),
            ins(EQUAL, 4, 13, 0),
            ins(EQUAL, 5, 12, 0),
            ins(EQUAL, 6, 11, 0),
            ins(EQUAL, 7, 10, 0),
            ins(EQUAL, 8, 9, 0),
            ins(EQUAL, 9, 8, 0),
            ins(EQUAL, 10, 7, 0),
            ins(EQUAL, 11, 6, 0),
            ins(EQUAL, 12, 5, 0),
            ins(EQUAL, 13, 4, 0),
            ins(EQUAL, 14, 3, 0),
            ins(EQUAL, 15, 2, 0),
            ins(EQUAL, 16, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(RETURN, ALLOW, 0, 0),
        ],
    )
}

// The post-close-range topology makes fd 4 absent during the observed startup
// probe. A later already-admitted pipe2/dup3 can populate it, but this exact
// rule can then only request O_NONBLOCK for that one descriptor. It cannot
// alter a surviving standard stream, select another descriptor, duplicate,
// acquire, lock, lease, or widen any other descriptor authority.
fn rustc_fcntl_rule(filter: &mut Vec<libc::sock_filter>, number: u32) -> Result<(), Error> {
    rule(
        filter,
        number,
        &[
            ins(LOAD, offset(0) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(1) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(1), 0, 0),
            ins(EQUAL, F_SETFL, 0, 8),
            ins(LOAD, offset(2) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(2), 0, 0),
            ins(EQUAL, O_RDONLY_OR_NONBLOCK, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(0), 0, 0),
            ins(EQUAL, 4, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(RETURN, ALLOW, 0, 0),
        ],
    )
}

// clone3 passes its flags through a pointer that classic BPF cannot inspect, so
// it is never admitted; ENOSYS makes glibc retry with clone(2), whose flags are
// a register. Only the exact pthread word with a zero upper half is allowed.
// The new thread inherits this filter, rlimits and the delegated cgroup.
// Both native ABIs pass the clone flags in the first argument register.
fn pthread_rules(filter: &mut Vec<libc::sock_filter>, clone: u32) -> Result<(), Error> {
    rule(filter, CLONE3, &[ins(RETURN, UNAVAILABLE, 0, 0)])?;
    rule(
        filter,
        clone,
        &[
            ins(LOAD, offset(0) + 4, 0, 0),
            ins(EQUAL, 0, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(LOAD, offset(0), 0, 0),
            ins(EQUAL, PTHREAD_CLONE_FLAGS, 1, 0),
            ins(RETURN, DENY, 0, 0),
            ins(RETURN, ALLOW, 0, 0),
        ],
    )
}

#[cfg(test)]
mod tests;
