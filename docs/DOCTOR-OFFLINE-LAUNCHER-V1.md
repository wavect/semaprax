# Provisioned offline doctor launcher v1

Status: private Linux launch implementation authored; unrun and unpromoted.

Audience: trusted provisioners, toolchain contributors and security reviewers.

## Authority and entry

The private `semaprax-doctor-launcher` starts the existing
[worker](DOCTOR-OFFLINE-WORKER-V1.md) and becomes its
[collector](DOCTOR-OFFLINE-COLLECTOR-V1.md). Its process-consuming unsafe entry
lives in the existing sys quarantine, with no safe embedding facade and no
ordinary `doctor` discovery or activation. It installs no service, configures no
host policy, and performs no namespace bootstrap or image-path lookup.

A trusted provisioner must supply a dedicated single-threaded process with
exclusively transferred descriptors exactly as follows:

| Descriptor | Supplied object |
| --- | --- |
| 0, 1, 2 | Anonymous standard pipes; 1 is the final report sink |
| 3 | Immutable sealed request memory file |
| 4 | Immutable sealed bundle memory file |
| 5 | Approved immutable executable worker memory file |
| 6 | Approved immutable executable collector memory file |

There are no other inherited descriptors, competing endpoint users, foreign
reapers, asynchronous application handlers, or signal/descriptor/image-metadata
mutators. Default `SIGCHLD` without automatic reaping is checked before clone.
The provisioner owns private mapped user/mount namespaces, sufficient existing
namespace capabilities, aggregate resources, launcher startup deadlines and
whole-cgroup reconciliation. The worker's per-tool PID namespaces remain its
own responsibility. Successful launcher exec into the collector preserves the
same parent process and its exclusive worker-reaping ownership.

The provisioner must authenticate the exact approved image for each role and
the complete immutable local interpreter, library and configuration closure
for launcher, worker and collector startup. It must exclude unapproved binfmt
redirection, external autoload/broker helpers, piped core handlers, file
capabilities and credential-changing startup. These are trusted launch
preconditions, not properties a descriptor, digest, ELF header or environment
flag proves. In particular, [binfmt registrations can redirect ELF execution](https://docs.kernel.org/admin-guide/binfmt-misc.html).
Passing ordinary filesystem descriptors violates the launch contract even when
preflight rejects them: process termination closes inherited descriptors and is
not a sandbox around an arbitrary caller's original files.

## Preflight and executable storage

Native64 little-endian Linux x86-64/AArch64 is the only initial implementation.
Unsupported hosts exit 125 without interpreting inputs. Native rejection or
uncertainty exits 126 without an ordinary report. Arguments and environment do
not select images, requests, paths, flags, deadlines or fallback behavior.

Before creating pipes or a child, the launcher checks the fixed descriptor and
standard-pipe inventory and child-reaping policy. It uses existing sealed-input
acquisition and worker request/bundle validators to require exact native
architecture, length, digest, selector and requested roles. Both images then
pass seal-first acquisition with the existing 512 MiB ceiling per image,
`F_SEAL_EXEC`, an execute permission bit, no set-user-ID/set-group-ID mode, and
the shared minimum native ELF validator. Scripts and malformed or foreign ELF
headers reject. The large bundle snapshot is dropped before sequential image
snapshots; limits bound each carrier, not total resident memory or kernel work.

Executable images require explicit executable-memfd provisioning and the four
immutable seals plus `F_SEAL_EXEC`. The
[non-executable input factory](DOCTOR-SEALED-INPUT-V1.md#anonymous-carrier-creation)
is appropriate for descriptors 3/4, never 5/6. There is no executable-mode
fallback or conversion in the launcher. Structural image validation does not
prove loadability, correct role, trusted provenance or loader closure. The
same approved image descriptors remain pinned through their actual exec.

The launcher sets and checks no-new-privileges before clone. This persists
through exec but is not a sandbox or a replacement for LSM admission; the
provisioner must account for its effect on executable security transitions.
See the [kernel contract](https://www.kernel.org/doc/html/latest/userspace-api/no_new_privs.html).

## Fixed launch and handoff

All preparation and allocation precedes clone. Checked flag operations clear
close-on-exec on the three owned standard pipes so they survive collector exec.
The four inputs/images are duplicated to owned close-on-exec descriptors at or
above 64. Three anonymous worker pipes and a pinned parent pidfd are prepared
there as well; the stdin writer is closed before clone, establishing private
EOF independently of collector scheduling. Small temporary descriptors are
closed with checked results. No directory enumeration or `close_range` sweep
is used.

`clone3` uses only `CLONE_PIDFD` and `SIGCHLD`, with private address space and
descriptor table, no shared threads or extra namespace flags. The child first
arms `SIGKILL` parent-death signaling and polls the inherited parent pidfd to
close the setup race. Privilege-changing execution can clear that linkage,
hence the startup restrictions above. See [parent-death semantics](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).
It explicitly closes original descriptors 0..6, maps its high pipe/request/
bundle sources to 0..4, and closes every unused known source before executing
the held worker with fixed argv and an empty environment.

The parent arms worker ownership immediately after successful clone. It pins
the returned child pidfd high before closing its original, closes original
inputs/images 3..6, then maps request, bundle, pidfd, reply reader and stderr
reader to the collector's fixed 3..7 inventory. Each destination is vacant
before mapping; implicit destination-close errors are not hidden inside `dup3`.
When duplicating the child pidfd, the settlement guard switches to the new valid
duplicate before closing the old one. Every unused known source, including
worker capture writers, is explicitly closed before collector entry.

Each branch retains only its selected high executable source until
`execveat(AT_EMPTY_PATH)` with fixed argv and empty environment. Its close-on-exec
close is part of successful trusted ELF startup, not deferred pipe settlement.
There is no pathname reopen, shell/script fallback, execution retry or ordinary
return. The collector independently repeats request/bundle/reply binding and
owns all subsequent capture, worker settlement and report delivery.

## Failure settlement and evidence

After clone, every parent-side mapping, close or collector-exec failure attempts
bounded settlement through the retained child pidfd. One kill attempt, exact
nonblocking reap, irreversible reap state and checked pidfd closure precede
exit 126; uncertain operations also fail-stop. The settlement budget is five
seconds. Reap disables later signaling or waiting, including disagreement after
a returned reap. No numeric-PID signaling or execution fallback is used.
Launcher death before successful handoff is covered by the child's parent-death
guard, but whole-cgroup reconciliation remains mandatory on uncertainty.
Bounds constrain observation loops, not hard real-time syscall latency.

Authored admission fixtures use real sealed files and shared validators;
resource-free lifetime scripts exercise the native settlement control flow.
These do not execute real tools or prove physical syscall fault behavior.
Ignored collector fixtures separately exercise the actual launcher with
production-created request/bundle files, explicit executable images, native/all
reports and malformed-image/digest rejection. A missing-interpreter negative
case intentionally violates healthy loader closure; rejection alone is not a
physical no-child-created or no-tool-effect witness.

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_launcher
cargo test --locked -p semaprax-doctor-collector --test provisioned -- --ignored --test-threads=1
```

Physical fixtures require the complete
[collector provisioning context](DOCTOR-OFFLINE-COLLECTOR-V1.md#evidence-and-non-claims)
and immutable current-head launcher, worker and collector paths supplied through
`SEMAPRAX_DOCTOR_LAUNCHER`, `SEMAPRAX_DOCTOR_WORKER` and
`SEMAPRAX_DOCTOR_COLLECTOR`. These fixture variables are not production admission.
Missing prerequisites fail; tests never downgrade executable sealing or
isolation. Existing direct worker/collector fixtures stay independent and
required. All new evidence is authored and unrun. Physical fault injection,
real-tool compatibility, complete deployment and cross-platform support remain
pending; ordinary CLI profiles remain unavailable and WP-05 is unpromoted.
