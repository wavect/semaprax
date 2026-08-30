# Doctor offline-profile admission and retained version-probe lifecycle v1

Status: authored, unrun private implementation contract; WP-05 is unpromoted.

Audience: CLI/platform contributors and reviewers.

## Scope

The unpublished `semaprax-full doctor` now requires an explicitly selected
offline tool profile instead of ambient PATH/home discovery. Selection is not
authority: a platform backend must admit the complete offline execution/input
closure before any tool lookup or version probe. **No production profile
backend or provisioning route is implemented yet.** Consequently, the real CLI
currently reports unavailable required checks (exit 1), even for a syntactically
valid selector. It never falls back to the retained installed-tool probe.

The standalone crates.io CLI rejects `doctor` without invoking tools. Report
policy and version parsing live in `crates/semaprax-toolchain`. The retained safe
platform facade/sys quarantine probe is described below, but is no longer
connected to the real CLI. Its partial isolation is not profile admission.

The safe platform facade owns no policy for general commands. Its existing
platform-sys quarantine owns the Linux/macOS and Windows OS operations. No
unsafe code enters the root compiler, no external dependency is added, and
the authenticated build runner is unchanged.

## Offline profile selection

```text
semaprax-full doctor [--profile <id>] [--target native|web|all] [--json]
```

An ID is 1–64 ASCII bytes matching `[a-z][a-z0-9-]{0,63}`. It selects one
profile; it is not a pathname, environment variable, JSON document, or permission
to read a registry. No on-disk profile format is admitted by this contract.
The default target remains contributor (Rust >=1.88); native requests Clang,
web requests Node >=22, and all requests Clang, Node, then Rust.

- All options and selector syntax are validated before any host callback.
  Malformed or duplicate options are CLI errors (exit 2, no report).
- A missing selector emits failed required profile/tool checks without acquiring
  a profile. A valid selector makes exactly one acquisition attempt for the
  entire report. Unavailability emits failed required checks (exit 1), without
  tool lookup or execution.
- An admitted host is scoped to that report. Its returned selector and OS/arch
  must match the requested selector and outer host facts before tool callbacks;
  disagreement is an internal CLI error (exit 2). A tool failure cannot acquire
  another profile or consult ambient tools.
- Checks describe only the admitted profile, never general build readiness.
  The common version/OS/arch/release rows remain first, followed by the required
  `profile` row and the requested tool rows. The `semaprax.doctor.v1` object
  shape is retained, but real CLI report bytes intentionally change with the
  added row. Exit 0 requires all required checks to pass.

The production acquisition currently returns unavailable using compile-time
platform facts only: no PATH, HOME, cwd, filesystem, registry or tool access.
This fail-closed gate does not claim an implemented executable sandbox, profile
provenance, or protection of process startup before doctor dispatch. Future
backends require independent review of offline provisioning/bootstrap, immutable
tool/loader/configuration inputs, OS filesystem/IPC/network boundaries, and
owned descendant settlement before returning an admitted host. An identifier,
digest, or declared “local” path alone cannot establish those properties.

The separate [sealed input primitive](DOCTOR-SEALED-INPUT-V1.md) now authors the
Linux borrowed-file acquisition boundary: seals before metadata/read, no
duplication or close, and bounded immutable output bytes. It is not a profile
parser or production admission backend, and is not connected to this CLI.

## Retained lower-level probe invocation bounds

The separate trusted-installed-tool probe invokes exactly `--version`, preserving
the lexical absolute path and basename for multicall tools. This is not identity
attestation or authenticated build-tool authority. These bounds remain the
lower-level contract, not the current CLI route or a complete offline backend:

- Capture the current directory and pass null standard input.
- Clear the child environment. Retain only `HOME`, `CARGO_HOME`, `RUSTUP_HOME`,
  and `RUSTUP_TOOLCHAIN`; additionally retain `DEVELOPER_DIR` on macOS and
  `SystemRoot`, `WINDIR`, and `USERPROFILE` on Windows.
- Force `RUSTUP_AUTO_INSTALL=0`. Do not inherit PATH, Node options, loader
  injection variables, proxy settings, or rustup download-server overrides.
- Bound each retained environment value to 8,192 native units and the complete
  environment to 32,768 units, including framing and the forced row. Reject
  paths/current directories beyond 32,768 units; platform-specific encoding
  may impose a tighter bound. Reject embedded NUL before process creation.
- Charge stdout and stderr together against 65,536 bytes. Retain stdout only,
  with storage allocated before spawn. Read at most one 8 KiB chunk from each
  stream per observation turn, so a flood cannot starve the other stream or
  deadline checks.
- Begin the ten-second execution deadline before process creation. EOF does
  not establish process exit. After exit, timeout, output overflow, or an I/O
  error, allow one five-second settlement budget, including final capture.

These are bounded polling protocols, not hard-real-time guarantees for OS
calls. Ordinary results retain the first selected error and are returned only
after the owned process scope has settled. Uncertain settlement or handle
closure is fail-stop: no report and no later doctor check may follow it.

## Owned process scope

Unix launches a private process group and observes the leader without reaping
it. Before launch, it requires the default `SIGCHLD` disposition without
`SA_NOCLDWAIT`; nondefault handlers and automatic reaping are rejected without
changing process-global state. The embedding host must also exclude other
waiters that reap this child and concurrent changes to that disposition.
Under that explicit coordination precondition, destructive group/leader
signals occur only while the unreaped leader pins its PID. No destructive
numeric-PID operation occurs after reap.

Darwin's zombie-only group `EPERM` is accepted only after non-reaping leader
exit observation and an independent bounded enumeration proving there are no
other group members. A live or uninspectable group's denial remains fail-stop.

Linux uses preallocated async-signal-safe child setup followed by `execve`,
requiring `close_range` support to exclude unrelated inherited descriptors and
the inherited syscall guard below before executing the tool.
macOS uses `posix_spawn` with `CLOEXEC_DEFAULT`, exact standard-handle actions,
and a held working-directory action. Descriptor exclusion does not depend on
the current soft file limit or a racy enumeration of inherited descriptors.

Windows creates the leader suspended, explicitly inherits only its three
standard handles, and assigns it to a non-breakaway, kill-on-close job before
resuming it. Settlement uses retained process/job handles and requires both
leader termination and zero active job processes. Assignment/resume failures
settle the still-owned suspended leader before returning.

The scope excludes descendants that escape the Unix session/group or are
created through external brokers. It is not an adversarial executable sandbox.
Environment filtering and disabling rustup auto-install do **not** enforce the
programme's no-network requirement. That WP-05 gate remains open; tools may
still read their own configuration and invoke OS facilities.

## Linux inherited syscall guard (partial isolation)

The Linux launch admits native 64-bit little-endian x86-64 and AArch64 only.
Other Linux ABIs return `Unsupported` during launch preparation, before fork;
there is no unfiltered fallback. After closing unrelated descriptors, the child
sets `PR_SET_NO_NEW_PRIVS` and installs a classic-BPF seccomp filter before
`execve`. Either setup failure exits the child without executing the tool and
uses the existing failure/settlement path. The parent remains unfiltered.

The filter authenticates the kernel-reported audit architecture before decoding
syscall numbers; foreign architectures and x86 x32 invocations kill the process.
It returns `EPERM` for `socket`, `io_uring_setup`, `io_uring_enter`,
`io_uring_register`, `pidfd_getfd`, `ptrace`, `process_vm_writev`, `connect`,
`bind`, `listen`, `accept`, and `accept4`. `socketpair` is admitted only for
`AF_UNIX`, protocol zero, and `SOCK_STREAM` or `SOCK_SEQPACKET`, optionally
combined with `SOCK_CLOEXEC` and/or `SOCK_NONBLOCK`. Every other family, type,
protocol or flag is rejected. Both 32-bit halves of each scalar argument are
checked; nonzero upper halves are rejected even where the kernel would truncate
them. The output-array pointer remains kernel-validated, not inspected by BPF.

These anonymous connected pairs support the real [Rust fork/exec handshake](https://doc.rust-lang.org/src/std/sys/process/unix/unix.rs.html)
without permitting datagram pairs or named-peer selection. Linux rejects
addressed sends on connected stream sockets and ignores the supplied address
for connected seqpacket sends; see the [kernel Unix-socket implementation](https://raw.githubusercontent.com/torvalds/linux/master/net/unix/af_unix.c).
`sendmsg`/`recvmsg` remain available for the handshake's descriptor transfer
between the pair's endpoints. Existing inherited-descriptor exclusion remains
required; this allowance does not grant socket creation or connection authority.

Other syscalls remain admitted. This closes the enumerated socket-creation,
named-endpoint, descriptor-acquisition, asynchronous-I/O and process-injection
routes; it is not a default-deny executable sandbox. Restrictions survive fork
and exec and cannot be relaxed by the tool. See the [Linux seccomp contract](https://docs.kernel.org/userspace-api/seccomp_filter.html).
Tools requiring denied calls can still fail a version probe; ABI admission is
not a promise of compatibility with every tool or libc. Existing Command-based
descendant fixtures remain required, independently of the controlled fork/exec
filter-inheritance fixture. No compatibility failure may trigger an unsandboxed
retry or be hidden by weakening those fixtures.

This layer does **not** complete WP-05's no-network requirement. The former CLI's
PATH discovery and metadata lookup happened before it; network-backed filesystem
access, tool/loader/configuration reads, and external filesystem/IPC brokers
remain outside its guarantee. macOS and Windows do not gain this guard. Full
cross-platform no-network admission requires a separately reviewed offline
discovery/tool-input closure and enforceable OS boundary; it remains open.

## Authored evidence

The sys `doctor::tests` fixtures cover fixed argv/basename, null stdin, exact
and plus-one combined output, stderr floods, nonzero exits, invalid UTF-8,
timeouts with open or closed pipes, descendants with open or closed pipes,
ordinary injected failures, and subprocess-only fail-stop uncertainty.
Separate subprocess fixtures cover descriptor exclusion above a lowered macOS
soft limit and rejection of incompatible Unix child-reaping dispositions.
Existing CLI fake-host and version-token tests remain required. The obsolete
real PATH-success test is replaced by calibrated healthy multicall fixtures
which the real CLI must never launch, under different PATH/home environments,
with missing/unavailable profiles. Exact canonical failure reports, malformed
selectors, untouched marker/sentinel inventory, one scoped acquisition, selected
version separation, and mismatched admission rejection are authored regressions.
Legacy injected `inspect` fixtures retain their old exact report bytes; new
profile fixtures exercise the added row. These checks are unrun and fake hosts
prove sequencing only, not physical isolation.
Linux adds a pure interpreter of the actual BPF instruction vectors for both
admitted ABIs, foreign-architecture/x32 rejection, and exact deny/allow decisions.
Physical fixtures calibrate unguarded socket creation before and after guarded
invocations, assert actual kernel denial and inherited no-new-privileges/filter
state in a tool and exec descendant, and force real kernel filter-installation
rejection to prove the executable-entry marker is never created. Unsupported
Linux ABIs have an explicit pre-fork rejection case, not output/settlement
support evidence. These new fixtures are also unrun.

The anonymous-pair correction adds independent literal argument inventories and
single-bit mutations across all 64 bits of each filtered scalar. Physical cases
exchange bytes in both directions for all eight admitted type/flag combinations,
reject invalid arguments without writing the descriptor array, and preserve
the remaining syscall-denial controls. A no-op `pre_exec` callback forces the
real Rust `Command` fork/exec fallback: both successful descendant execution and
failed-exec error reporting must work while descendants retain the guard. The
callback allocates nothing and performs no operation. Existing ordinary spawn,
capture, descendant-settlement and filter-installation failure cases remain.
These compatibility checks are authored and unrun, not a full no-network gate.

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys doctor::tests
cargo test --locked -p semaprax-toolchain --test cli_doctor_v1
```

These fixtures are authored but not executed in this batch. They need physical
Linux, macOS, and Windows runs and do not establish no-network enforcement,
hosted support, or production readiness.
