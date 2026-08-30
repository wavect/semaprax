# Doctor version-probe lifecycle v1

Status: authored, unrun private implementation contract; WP-05 is unpromoted.

Audience: CLI/platform contributors and reviewers.

## Scope

The unpublished `semaprax-full doctor` host invokes one trusted installed executable with exactly
`--version`. The injected `DoctorHost`, check order, and
`semaprax.doctor.v1` report schema are unchanged. The lexical absolute path is
preserved, including its basename for multicall tools; this is not executable
identity attestation or authenticated build-tool authority.

The standalone crates.io CLI rejects `doctor` without invoking tools. Report
policy and version parsing live in `crates/semaprax-toolchain`; the full CLI
continues to use the existing safe platform facade and sys quarantine.

The safe platform facade owns no policy for general commands. Its existing
platform-sys quarantine owns the Linux/macOS and Windows OS operations. No
unsafe code enters the root compiler, no external dependency is added, and
the authenticated build runner is unchanged.

## Invocation bounds

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
It returns `EPERM` for `socket`, `socketpair`, `io_uring_setup`, `io_uring_enter`,
`io_uring_register`, `pidfd_getfd`, `ptrace`, and `process_vm_writev`. Other
syscalls remain admitted. This closes direct socket creation and the enumerated
descriptor-acquisition/asynchronous-I/O/process-injection routes; it is not a
default-deny executable sandbox. The restrictions survive fork and exec and
cannot be relaxed by the tool. See the [Linux seccomp contract](https://docs.kernel.org/userspace-api/seccomp_filter.html).

`socketpair` is denied even for anonymous local IPC. Tools requiring any denied
call can fail a version probe; ABI admission is not a promise of compatibility
with every tool or libc. In particular, some [Rust process-spawn paths](https://doc.rust-lang.org/src/std/sys/process/unix/unix.rs.html)
use a local socket pair. Existing Command-based descendant fixtures remain
required, independently of the controlled fork/exec filter-inheritance fixture.
No compatibility failure may trigger an unsandboxed retry or be hidden by
weakening those fixtures.

This layer does **not** complete WP-05's no-network requirement. Parent PATH
discovery and metadata lookup happen before it; network-backed filesystem
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
Existing CLI fake-host, version-token, PATH, and multicall tests remain required.
Linux adds a pure interpreter of the actual BPF instruction vectors for both
admitted ABIs, foreign-architecture/x32 rejection, and exact deny/allow decisions.
Physical fixtures calibrate unguarded socket creation before and after guarded
invocations, assert actual kernel denial and inherited no-new-privileges/filter
state in a tool and exec descendant, and force real kernel filter-installation
rejection to prove the executable-entry marker is never created. Unsupported
Linux ABIs have an explicit pre-fork rejection case, not output/settlement
support evidence. These new fixtures are also unrun.

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys doctor::tests
cargo test --locked -p semaprax-toolchain --test cli_doctor_v1
```

These fixtures are authored but not executed in this batch. They need physical
Linux, macOS, and Windows runs and do not establish no-network enforcement,
hosted support, or production readiness.
