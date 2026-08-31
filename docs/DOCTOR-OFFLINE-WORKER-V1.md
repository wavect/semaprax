# Provisioned offline doctor worker v1

Status: authored private Linux worker; unrun and unpromoted.

Audience: toolchain maintainers, worker provisioners and security reviewers.

## Deployment and authority

The separately invoked `semaprax-doctor-worker` executable is not an ordinary
CLI subprocess. A trusted provisioner must establish its clean launch context
before entry. No service installation, privileged host configuration, ambient
worker discovery or ordinary `doctor` activation is part of this change.
The binary and its unsafe process-consuming entry live in the existing sys
quarantine crate. No safe platform facade exposes this entry with hidden
single-thread/descriptor-lifetime prerequisites; the root compiler and ordinary
toolchain remain unsafe-free.

For the initial native64 Linux x86-64/AArch64 implementation the provisioner
must supply a single-threaded worker, exclusive descriptor ownership, no foreign
reapers or signal-policy mutators, private mapped user and mount namespaces,
and immutable local worker/loader inputs. Descriptors 0, 1 and 2 are anonymous
pipes; 1 is the invocation-local report sink. Descriptors 3 and 4 are sealed
request and bundle memory files. No other inherited descriptors are permitted.
This is a trusted launch contract, not a claim that inspecting five handles
proves the absence of every other handle or authenticates the provisioner.

Kernel, LSM and VM/swap behavior are trusted. Provisioning must exclude external
filesystem/binfmt autoload helpers and piped core-dump handlers during worker
execution. A core-size limit alone does not constrain piped core handlers.
Provisioning, executable startup and the collector's use of report bytes are
outside the worker's offline boundary. Do not describe this as zero network
activity by the whole host or by an arbitrary caller's startup and exit.

The provisioner also owns aggregate resource admission for the worker and all
descendants. The initial policy denies tool process/thread creation; per-tool
address-space, descriptor and core limits are additional defenses, not a claim
to bound total kernel memory. No deployment or compatibility support is claimed
without real selected-tool and hostile-worker execution evidence.

## Closed request and result

Request bytes are a separate sealed memory file, at most 149 bytes, with no
trailing bytes: `SPXDWK1\0`, OS byte 1 (Linux), architecture byte 1 (x86-64) or
2 (AArch64), target byte, exact role mask, 32 nonzero-as-a-whole opaque nonce
bytes, little-endian u64 bundle length, 32 raw SHA-256 bundle digest bytes,
one-byte selector length, and 1–64 selector bytes. The existing selector grammar
is `[a-z][a-z0-9-]{0,63}`. Targets are contributor 0, native 1, web 2, all 3;
their role masks are respectively 4, 1, 2 and 7. Role bits are Clang 1, Node 2,
Rust 4. Unknown fields, noncanonical combinations, absent requested tools,
wrong length/hash/selector/architecture and unsupported hosts reject before
creating a child. Unrequested bundle roles never select an invocation.

The worker acquires both inputs through the existing sealed-input reader and
consumes the existing opaque bundle parser. It prepares paths, root inventory,
syscall guard and capture storage before launching tools. No request field can
choose argv, environment, a pathname, limits or a fallback.

Reply bytes are `SPXDWR1\0`, raw SHA-256 of the exact request, the nonce,
OS/architecture/target/roles bytes, and one row-count byte. Ordered rows contain
role byte, status byte, u32 little-endian stdout length and stdout bytes.
Statuses are success 0, invalid 1, unsupported 2, supervisor launch failure 3,
unsuccessful child termination 4,
output-limit 5, timeout 6 and I/O 7. Failure rows contain no payload. Each
success payload is at most 65,536 bytes; stdout and stderr are charged together
during execution. Only one exact requested row set is admitted.
Child setup and executable entry failures terminate that child and therefore
use status 4; an executable can itself return the same exit status, so the
worker does not pretend to distinguish those causes without a setup handshake.

The reply is a bounded tool observation, not a version-policy report or proof
of executable provenance. Nonces and digests bind bytes; they grant no authority.
A collector must own the provisioned invocation, compare the exact request,
require one complete reply without trailing bytes, and observe successful
worker termination and owned-worker settlement. A complete frame followed by
failed or uncertain termination is not a successful observation. No
`reply bytes -> AdmittedProfile` conversion is provided.

## Tool execution and settlement

Each selected tool gets a fresh PID namespace through `clone3` with a pidfd.
The child materializes the existing detached read-only tmpfs inventory, enters
its root/cwd, and retains only private stdin/stdout/stderr pipes. It removes
bounding, effective, permitted, inheritable and ambient capabilities, locks
securebits against root capability recovery, and sets no-new-privileges before
installing a native-ABI default-deny syscall filter and executing exactly the
selected absolute in-root tool pathname with `--version` and a fixed environment.

The guard denies process/thread creation and network/IPC/descriptor acquisition;
opens are read-only, writes target only the two capture streams, and tools may
not lift resource limits or clear the parent-death signal. A retained supervisor
pidfd closes the parent-death setup race before executable entry. Unsupported
syscalls fail; compatibility failures never trigger an unconfined retry.

The supervisor fairly drains both streams under one ten-second execution
deadline, beginning before child creation, and one five-second settlement
budget. Failure is sticky. PID-namespace init death and exact reap precede any
next tool or reply. Uncertain kill, wait or owned descriptor close is fail-stop.
Reply output has a separate bounded write deadline and cannot trigger execution
again. These are bounded observation protocols, not hard-real-time syscall
latency guarantees.

## Evidence and remaining integration

Wire mutation/binding/boundary tests, actual BPF interpretation, and provisioned
worker execution fixtures must remain separate evidence layers. Parser tests
do not establish physical confinement. Required physical cases include real
Clang/Node/Rust distributions, forbidden files/syscalls, capability recovery,
process/thread creation, supervisor death, output overflow, timeout, setup
failure and uncertain settlement, plus exact role order and no premature reply.

Authored physical fixtures in `doctor/offline_worker/tests.rs` cover actual
synthetic native ELF execution, socket-denial observation, exact 65,536-byte
success, overflow, timeout, and invalid request/hash/missing-role rejection.
A separate hostile-program fixture covers exact mutation/process-creation
denials, stdin EOF, inherited descriptor exclusion, readable bundled content,
root traversal and an inaccessible, unchanged outside sentinel. These programs
do not receive a test-only syscall policy. A denied capability-changing call
does not itself prove that the effective capability set was cleared.
A separate fixture requires externally provisioned real Clang/Node/Rust bundle
inputs. Selecting these ignored tests requires the absolute worker path in
`SEMAPRAX_DOCTOR_WORKER` and the explicit context acknowledgement
`SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT=private-mapped-user-mount-clean-worker-cgroup-v1`.
The acknowledgement does not authenticate provisioning. The driver owns its
close-on-exec descriptor flushes outside the worker boundary. Provisioned
cgroup cleanup is still required on driver or worker uncertainty.

The external lifecycle fixture pauses the exclusively owned supervisor and
consumes its exact stop event before looking up its sole child. The stopped
single-threaded supervisor and exclusion of other reapers pin that child's PID
until the driver retains its pidfd. While the supervisor remains stopped, the
driver requires exact executed-image bytes and observes empty post-exec
capability sets, no-new-privileges and filter mode through procfs. It then kills
only the supervisor, observes the retained child's termination, and requires
worker capture EOF without a premature reply. No numeric child PID is reused
after the supervisor resumes or dies. Orphan reaping is the provisioner's
responsibility, not a claimed driver-owned reap.

This fixture additionally requires readable procfs using the driver's PID
namespace, executable/status inspection permission, and no competing external
termination policy during the parent-death observation. It grants no procfs or
additional syscall access to the tool. The driver requires one self `NSpid`
entry matching its own PID and verifies that it is not a child subreaper before
launch. These observations follow the kernel's
[stop-event semantics](https://man7.org/linux/man-pages/man2/waitid.2.html),
[pidfd lifetime](https://man7.org/linux/man-pages/man2/pidfd_open.2.html), and
[process status fields](https://man7.org/linux/man-pages/man5/proc_pid_status.5.html).

The capture/settlement algorithm is shared by native owned operations and a
private scripted test implementation. Scripts cover fair drainage, exact and
over-limit output, EOF without exit, sticky first failure, and uncertain
kill/reap/drain/deadline fail-stop with no later action. Scripted operations
carry no process authority and do not prove physical syscall-failure behavior;
no production environment variable enables injection.

No fixture has been executed here. Syscall-denial observations alone cannot
distinguish the worker filter from a stricter provisioner policy; securebits
locking and physical injected settlement failures still need their own
evidence. The real distributions may reject the initial no-thread policy.
These are pending gates, not evidence of compatible deployment.

Select the authority-free wire, BPF and shared-control-flow tests separately:

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_worker::wire
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_worker::guard
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_worker::capture
```

Only inside the separately provisioned context described above, with the worker
artifact and real-bundle inputs already supplied, select the ignored physical
gates serially. Missing prerequisites fail; no fixture silently skips:

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_worker::tests -- --ignored --test-threads=1
```

The real-distribution fixture additionally requires absolute
`SEMAPRAX_DOCTOR_REAL_BUNDLE` and its exact `SEMAPRAX_DOCTOR_REAL_SELECTOR`.
These commands describe pending evidence, not commands executed in this batch.

The separate [live collector](DOCTOR-OFFLINE-COLLECTOR-V1.md) connects provisioned
handoff to shared doctor version/report policy. It is not ordinary CLI worker
discovery: that CLI remains unavailable. Linux
worker output does not represent native macOS or Windows tool availability;
those hosts need their own worker/storage/confinement implementations. No
existing report schema, generated artifact, release package inventory or
completion status changes here.
