# Provisioned offline doctor collector v1

Status: private implementation authored; unrun and unpromoted.

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
deadlines and cgroup reconciliation remain provisioner-owned. This repository change installs no
service and changes no host security configuration.

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
- Toolchain `doctor/settled_report/tests.rs` covers shared report policy, role
  aliases, versions, invalid UTF-8 and failed observations. Ordinary CLI/library
  parity remains covered separately in `cli_doctor_v1.rs`.
- Collector `tests/provisioned.rs` contains ignored physical fixtures for the
  actual native worker-to-report path with a synthetic tool bundle, calibrated
  malformed/replayed replies, complete
  replies followed by nonzero exit, and complete reply/EOF without worker exit.
  The latter cases retain the actual sixty-second deadline. Sealed executable
  memfd surrogates test collector mechanics, not approved-worker provenance.

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
These gates are authored, not executed. Physical wrong-child/close/settlement
fault injection, report-delivery failures and real-tool compatibility remain
additional pending evidence.

This component does not install or discover a provisioner, authenticate arbitrary
startup state, make Linux observations represent macOS/Windows, or promote WP-05.
Deployment and complete no-network support remain unproven until the worker and
collector physical gates execute on the exact claimed head.
