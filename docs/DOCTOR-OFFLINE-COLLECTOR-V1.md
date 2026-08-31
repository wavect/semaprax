# Provisioned offline doctor collector v1

Status: private implementation; selected Linux unit evidence passes locally;
physical lifecycle evidence unrun and unpromoted.

Audience: toolchain maintainers, trusted provisioners and security reviewers.

## Live handoff, not reply-file admission

The private `semaprax-doctor-collector` executable joins the existing
[worker](DOCTOR-OFFLINE-WORKER-V1.md) to the toolchain's ordinary doctor version
and report policy. It is a separate provisioner-owned entry, not a new ambient
discovery path in `semaprax-full doctor`. Neither an arbitrary reply file nor a
nonce, digest, environment flag or pidfd alone grants profile authority.

The trusted provisioner starts exactly one immutable worker with the agreed
sealed request/bundle and exclusive capture pipes, then execs into the collector,
preserving parenthood and exclusive reaping ownership. It supplies a dedicated
single-threaded collector with these exclusively transferred live descriptors:

| Descriptor | Owned object |
| --- | --- |
| 0, 1, 2 | Anonymous standard pipes; 1 is the final report sink |
| 3 | The exact sealed worker request |
| 4 | The exact sealed worker bundle |
| 5 | The owned worker's pidfd |
| 6 | Exclusive reader of the worker's reply pipe |
| 7 | Exclusive reader of the worker's stderr pipe |

The separate [private launcher](DOCTOR-OFFLINE-LAUNCHER-V1.md) now authors this
worker-start/collector-exec wiring for an already provisioned process. It does
not remove the external image, loader, namespace or cgroup prerequisites below.

There are no other inherited descriptors, competing readers/writers, foreign
reapers or signal/descriptor mutators. The provisioner authenticates endpoint
binding, worker/collector executable and loader provenance, and the worker's
complete namespace/input/security context. It also supplies a fixed sanitized
startup environment with no loader injection. The collector cannot infer those
facts by inspecting these handles. These endpoints must belong to the exact
approved worker image, not merely any immutable executable. Parenthood and
default `SIGCHLD` policy without automatic reaping must hold from worker creation
through collector exec and final reap. The collector checks that the pidfd names
its own unreaped child before obtaining signaling authority; a wrong or nonchild
pidfd fails without being signaled. Aggregate resource admission, startup
deadlines and cgroup reconciliation remain provisioner-owned. This repository
change installs no service and changes no host security configuration.

The initial implementation is native64 little-endian Linux x86-64/AArch64 only.
It uses the worker's exact closed request and reply wire version, verifies the
bundle length/hash/selector/architecture and requested role inventory, and never
selects tools from ambient paths. Unsupported hosts cannot produce an admitted
observation. Ordinary CLI profile acquisition remains unavailable.

## Collection and settlement

The collector owns worker settlement from entry, before reading request bytes.
It drains at most one 8 KiB chunk from each capture stream per turn, retaining
only bounded reply bytes and rejecting any worker stderr. The reply limit is
the worker wire's maximum; all observations have a sixty-second deadline from
collector entry. Provisioning and worker startup before entry are separately
bounded by the provisioner.

EOF is not exit. A complete frame is not exit. The collector observes the
owned worker through nonblocking pidfd `waitid` with `WNOWAIT`, then reaps that
same owned identity with nonblocking `waitid`. Successful return requires exact
normal exit zero on both observations, complete capture EOF, one exact reply
bound to the immutable request, and successful closure of input/capture/pidfd
handles. No destructive action occurs through a reaped numeric PID.

Any input, framing, I/O, deadline, worker-exit or close failure produces no
ordinary observation or report. After ownership authentication and before reap,
the collector attempts bounded pidfd kill/reap and then terminates fail-stop;
after reap it never waits again. Unauthenticated descriptors are never signaled.
Reaping a forcibly terminated worker does not prove its tool descendants were
settled, so external cgroup reconciliation remains mandatory on failure.
No error path runs another worker or falls back to installed tools.

## Opaque observation and report ownership

Only this unsafe live collector entry can construct `SettledDoctorObservation`.
Its fields and per-tool results are immutable, with getters but no public
constructor or deserializer. The safe platform facade may reexport those types;
it does not expose a safe function with hidden process-lifetime prerequisites.
Worker rows remain keyed by role, even when several roles name the same bundled
executable path. Observation data cannot grant further execution authority.

The toolchain library owns the existing doctor version/check/report policy for
both its ordinary CLI and the collector adapter. The adapter renders the exact
selected target and profile without reexecuting tools. Invalid UTF-8 and failed
tool observations become failed required checks. It does not interpret stderr
as a version or identify ambient installed-tool readiness.

A small unpublished collector-entry crate depends on the existing sys quarantine
and the toolchain library. It owns only the unsafe entry and bounded report-output
boundary, avoiding a sys-to-toolchain dependency cycle. Root compiler and ordinary
toolchain code stay unsafe-free. The executable emits canonical doctor JSON;
arguments and environment do not choose a request, output path, fallback or
additional operation. Report delivery requires complete bounded output and
normal collector exit; a partial frame or uncertain termination is not success.
Output is capped at two MiB with a five-second write deadline; only ordinary
doctor exit codes zero and one may be emitted after successful collection.
Report setup initializes the complete Rust signal-set storage before the checked
libc calls and blocks `SIGPIPE` through process termination. This avoids assuming
that `sigemptyset` initializes unused storage: the
[Linux glibc implementation](https://codebrowser.dev/glibc/glibc/sysdeps/unix/sysv/linux/sigsetops.h.html)
only clears the words used for kernel signals. A broken report pipe is an output
failure, not permission to retry collection or report success.

## Evidence and non-claims

Required evidence includes authentic worker-to-collector-to-report success,
exact target/role/version behavior, malformed/cross-bound replies, nonzero exit
after a complete frame, EOF without exit, overflow, timeout, failed settlement,
and unchanged ordinary CLI unavailable behavior. Scripted state tests do not
establish physical lifecycle ownership. Synthetic executables are not real
Clang/Node/Rust compatibility evidence.

Authored evidence is split by ownership:

- Sys collector `linux/capture/tests.rs` exercises the production capture loop
  with scripted reads, exact exit/reap agreement, deadlines, limits and sticky
  failures. It does not exercise physical syscalls or prove OS settlement.
- Private lifetime and report-delivery state machines are also shared by native
  operations and resource-free scripts. Lifetime scripts cover ownership
  authentication before signaling, bounded emergency kill/reap, the irreversible
  reap latch and fixed one-shot handle closure. Report scripts cover exact byte
  suffixes and accepted-byte conservation after partial writes, including
  repeated `EAGAIN` across 8 KiB chunk boundaries, the unchanged write deadline,
  setup failure,
  zero/impossible/error writes and termination after each uncertain close.
  Only native adapters own descriptors and process termination. Scripted
  outcomes cannot construct a settled observation or enable fault injection
  in a running collector.
- Toolchain `doctor/settled_report/tests.rs` covers shared report policy, role
  aliases, versions, invalid UTF-8 and failed observations. Ordinary CLI/library
  parity remains covered separately in `cli_doctor_v1.rs`.
- Collector `tests/provisioned.rs` contains ignored physical fixtures for the
  actual native worker-to-report path with a synthetic tool bundle, calibrated
  malformed/replayed replies, complete replies followed by nonzero exit, and
  complete reply/EOF without worker exit.
  The latter cases retain the actual sixty-second deadline. Sealed executable
  memfd surrogates test collector mechanics, not approved-worker provenance.
- Additional provisioned cases execute all three roles with exact ordered JSON,
  bracket a failed Node invocation with healthy controls, and require the later
  Rust role's successful observation in the ordinary exit-one report. These
  execute synthetic bundled programs, not real tool distributions.
- The separate real-distribution gate described below routes an explicit real
  bundle through the production launcher, worker and collector, using independent
  expected tool details. It is authored but physically unrun.
- The closed-sink case first calibrates a complete large report from the actual
  worker, then observes an exact report prefix before closing its sole reader.
  The checked pipe capacity proves that the complete report was not yet written;
  the collector must still be live just before the close and must terminate
  fail-stop. Scheduling can race the unchanged output deadline, so this does not
  isolate a physical `EPIPE` return. Scripts separately select that error branch.
  Partial report bytes are expected here and are never successful delivery.
- The nonchild case transfers a pidfd for an exclusively owned sibling sentinel,
  with no actual worker to orphan. After collector rejection, the sentinel must
  answer a newly released challenge and exit normally. The calibrated literal
  protocol detects unintended lethal/stopping effects, not every possible
  signaling syscall; the shared ownership scripts separately assert no signal
  operation before authentication. No arbitrary host process is targeted.

Physical fixtures require immutable current-head worker and collector binaries,
the complete worker namespace and cleanup prerequisites, and explicit executable
memfd support (`MFD_EXEC`) for surrogate cases. Set absolute
`SEMAPRAX_DOCTOR_WORKER` and `SEMAPRAX_DOCTOR_COLLECTOR` paths plus the existing
`SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT=private-mapped-user-mount-clean-worker-cgroup-v1`
acknowledgement only inside that trusted provisioned context. No environment
value proves provisioning. Run serially, with no competing descriptor mutators:

```sh
cargo test --locked -p semaprax-doctor-collector --test provisioned -- --ignored --test-threads=1
```

Missing prerequisites fail rather than skip or weaken the policy. The external
provisioner must bound startup and reconcile the entire fixture cgroup on
failure; reaping the collector alone does not prove descendant settlement.
The physical gates are authored, not executed. Selected sys unit suites pass
locally on Linux AArch64/Rust 1.88: worker wire (7), guard (4), capture (7),
collector Linux (21), and launcher (13), for 52 tests. This is scoped scripted
control-flow and native input/admission evidence, not physical tool execution,
process settlement fault injection or complete doctor validation.
Physical owned-handle close and
settlement fault injection, syscall-specific report-failure observation and
real-tool compatibility remain additional pending evidence. Sibling rejection
does not establish executable/endpoint provenance for arbitrary child handoffs.

### Real-distribution production-launcher gate

`real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions`
requires the full context above, plus an absolute immutable current-head
`SEMAPRAX_DOCTOR_LAUNCHER` path. The trusted provisioner also supplies:

- `SEMAPRAX_DOCTOR_REAL_BUNDLE`: an absolute, quiescent regular file containing
  the admitted bundle, nonempty and no larger than 512 MiB; the harness bounds
  the read and the production bundle parser validates its closed inventory.
- `SEMAPRAX_DOCTOR_REAL_SELECTOR`: the bundle's exact admitted selector.
- `SEMAPRAX_DOCTOR_EXPECTED_CLANG_DETAIL`,
  `SEMAPRAX_DOCTOR_EXPECTED_NODE_DETAIL`, and
  `SEMAPRAX_DOCTOR_EXPECTED_RUST_DETAIL`: independent expected report details,
  each nonempty UTF-8, already trimmed, control-free and at most 8 KiB.

The Clang detail includes its absolute in-root executable path followed by the
normalized first version line in parentheses, such as
`/bin/clang (clang version ...)`. Node and Rust details are their normalized
first version lines. Supply these expectations independently of the observed
report; the harness neither derives them from actual output nor runs an
unconfined version command. Actual shared policy still requires Node 22 or newer
and Rust 1.88 or newer. The provisioner owns real-distribution provenance and
the complete tool, loader, library and configuration closure; expected strings
and exact JSON are not provenance evidence.

The gate uses production sealed-input and executable factories, derives an
`All` request from the admitted bundle, and invokes the production launcher.
It requires exactly eight canonical ordered successful rows (SEMAPRAX, OS,
architecture, release, profile, Clang, Node, Rust), exit zero and empty stderr,
then reacquires the retained sealed request and bundle and compares their bytes.
There is no synthetic fallback or confinement relaxation.

Inside the genuinely provisioned context only, select this gate serially:

```sh
cargo test --locked -p semaprax-doctor-collector --test provisioned real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions -- --exact --ignored --test-threads=1
```

Compiling this ignored test or running its resource-free report-oracle tests
does not execute the physical gate. The harness compiles locally on Linux
AArch64/Rust 1.88; both literal report-oracle tests pass, with all 13 physical
tests left ignored. A default restricted Docker container does
not supply the mapped namespaces, capabilities and dedicated cgroup prerequisite;
setting the context acknowledgement cannot make it do so.

This component does not install or discover a provisioner, authenticate arbitrary
startup state, make Linux observations represent macOS/Windows, or promote WP-05.
Deployment and complete no-network support remain unproven until the worker and
collector physical gates execute on the exact claimed head.
