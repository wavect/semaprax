# AArch64 Linux offline doctor confinement tracking v1

Status: **all twenty-six fixtures executed locally on native AArch64 with real
carriers; not executed on the proposed hosted runner.**
This is a separate AArch64 tracking contract, not an extension of the
x86-64-only [Provisioned Linux gate v1](DOCTOR-PROVISIONED-LINUX-GATE-V1.md).
It neither promotes WP-05 nor establishes hosted, physical-device, or general
production support.

Audience: release engineers, platform maintainers, and security reviewers.

Tracking issues: [#279](https://github.com/wavect/semaprax/issues/279),
[#334](https://github.com/wavect/semaprax/issues/334).

## Scope decision

The private [Provisioner v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md#purpose-and-boundary)
defines the platform boundary. Its first implementation is native 64-bit little-endian Linux
x86-64 **and AArch64**, with a default-deny syscall table for each native ABI.
Implementation scope is not execution evidence. The existing x86-64 gate rejects AArch64, so an x86-64
run cannot satisfy this contract.

Accordingly, this document commits AArch64 to separate tracking. A future
fully provisioned AArch64 gate must exercise the same twenty-six lifecycle
fixtures, through a native AArch64 package, real Clang/Node/Rust carrier, and
the architecture-specific closed policy. It must report an absent namespace,
cgroup, image, sealed input, real bundle, selector, or kernel prerequisite as
a failure rather than a skip. It must not change a syscall, open-flag, rlimit,
or capability allowance merely because a hosted run fails: a proposed widening
needs an observed real-binary behaviour and a negative control demonstrating
that the protected operation remains denied.

## Historical local evidence

Commit [`734e67af`](https://github.com/wavect/semaprax/commit/734e67af)
recorded one native AArch64 Linux 6.12 lifecycle run in Docker Desktop's Linux
VM on Apple Silicon. The worker, launcher, and collector were built and run as
native AArch64 binaries; this was not cross-compilation or emulation. It is
**local Docker-VM evidence**, not GitHub-hosted evidence and not
physical-device evidence. It was recorded at that commit and is not a claim
about the current head.

The run passed 24 of the 26 ignored lifecycle fixtures. It included the
zero-syscall spin sentinel
`doctor::offline_worker::tests::lifecycle::post_exec_capabilities_and_supervisor_death_are_observed_externally`,
which is relevant context for the separate x86-64 investigation but proves
nothing about that host's unresolved real-distribution failure.

The two remaining fixtures were not confinement passes and were not silently
skipped:

- `doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions`
- `real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions`

They failed fast because that local run supplied neither the required real
Clang/Node/Rust bundle nor its selector and independent expected details. Their
absence was an explicit unresolved exclusion, not a result from which
success could be inferred. At that commit the local script kept the
twenty-four-case selection separate and probed the missing real-distribution
precondition; the real-carrier run below replaces those probes.

## Real-carrier local evidence (issue #334)

On 2026-09-30 both repository scripts, unmodified, ran all twenty-six
fixtures to success on native AArch64: 13 of 13 `platform-sys-lib` and 13 of
13 `doctor-collector` `provisioned` cases, with no skipped or ignored case. The
two real-distribution fixtures ran against real carriers:

- Clang 17.0.6 (`clang-17` from LLVM's `clang+llvm-17.0.6-aarch64-linux-gnu`
  archive, SHA-256 `6dd62762…87b75a`; its first version line names Linaro's
  build, `clang version 17.0.6 (http://git.linaro.org/toolchain/jenkins-scripts.git 09f505ca…)`),
- Node v22.23.2 (`node-v22.23.2-linux-arm64`, SHA-256 `fff4078c…94abb8`),
- Rust 1.88.0 (`rustc 1.88.0 (6b00bc388 2025-06-23)`, from rustup),

bundled by `scripts/doctor-provisioned-linux-bundle.py --architecture aarch64`
into a 580 MB carrier. The host was an Apple Container Linux VM on Apple
Silicon (a native AArch64 guest, not emulation): Linux 6.18.35, Debian 12
glibc 2.36, the `rust:1.97.1-slim-bookworm` image. It is **local VM
evidence**, not GitHub-hosted evidence and not physical-device evidence.

The default Apple Container kernel omits `CONFIG_PROC_CHILDREN`, which the
supervisor-death fixture needs to read `/proc/<pid>/task/<tid>/children`, so
that fixture failed there with `NotFound`. The run used that kernel's own
configuration rebuilt from the kernel.org 6.18.35 tarball with GCC 11.4 and
exactly one change, `PROC_CHILDREN n -> y`. With it, the historical
twenty-four cases passed again before any policy change.

The first real-carrier run exposed three AArch64 defects, each now fixed:

- the packager admitted only x86-64 ELF images; `--architecture aarch64` now
  selects the AArch64 wire byte and ELF machine by name, and x86-64 remains
  the default the gate uses;
- Node aborted at `node.cc:653` (`(s.flags) != (-1)`) because AArch64 had no
  `fcntl` rule, and rustc died of `SIGTRAP`;
- the AArch64 role rows were empty.

The argument trace of the real `--version` runs showed Node's `fcntl` calls
(`F_GETFL` on fds 0–2, `F_SETFD(FD_CLOEXEC)` on 0–16) and rustc's
(`F_SETFL(O_RDONLY|O_NONBLOCK)` on fd 4) to be identical to the traced x86-64
calls, so the existing role-local `fcntl` rules now apply to AArch64's
`fcntl` (25). rustc also needs `ppoll` (73, AArch64's form of `poll`),
`pipe2` (59) and one pthread `clone` (220) for its Ctrl-C watcher thread,
admitted only for glibc's exact flag word, with `clone3` answering `ENOSYS`.
Clang and Node get no AArch64 addition. The negative control is the run
before this change, in which the same carriers failed at exactly those calls,
together with the BPF oracle tests in `offline_worker/guard/tests.rs`: every
single-bit mutation of each admitted argument, every other `fcntl` command,
every process-creating `clone` word and every other role remain denied.

The fixtures consume the current-head worker, launcher and collector directly.
No signed AArch64 release package was built or is claimed.

## Hosted runner candidate

`.github/workflows/doctor-provisioned-linux-aarch64.yml` is a dispatch-only,
unexecuted candidate for a GitHub-hosted `ubuntu-24.04-arm` runner. It is not a
CI-required job, has no push or pull-request trigger, and has no
`continue-on-error`. It first proves that its native host is Linux AArch64,
fetches the locked dependency graph, provisions the real carriers with
`scripts/doctor-provisioned-linux-aarch64-carriers.sh` (its only network step),
and then runs the lifecycle driver offline with the resulting `carriers.env`.
The driver has no `--skip` selector: it runs all twenty-six fixtures, each
named with `--exact`, and refuses before building if any real-carrier input is
missing, malformed, duplicated or unexpected. Every lifecycle command is
unmasked and fail-fast. Missing user namespaces, Cargo, a native AArch64 host,
dependencies, carriers or any lifecycle assertion makes the job fail. A runner
label is not an attestation; the driver re-observes the host properties it
can.

A passing dispatch would be hosted AArch64 lifecycle evidence, not a
signed-release gate and not a production or WP-05 promotion. A failed dispatch
is evidence of a failure only after the job log identifies the exact failed
precondition or fixture; it is never permission to widen the policy.

## Completion boundary

The runtime acceptance of issue #334 is met by the local real-carrier run
above. A hosted dispatch, a signed AArch64 release path and any wider
architecture support promise remain separate decisions. No result from the
x86-64 gate can stand in for this contract, and no result recorded here can
stand in for the x86-64 gate.

## Nonclaims

This document does not claim a hosted run, a physical device, a signed-release
AArch64 gate, an ordinary CLI route, an authenticated build host, or
production readiness. It does not broaden any x86-64 policy or
permit a future AArch64 policy change without direct evidence and a surviving
negative control.
