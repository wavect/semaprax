# Windows doctor confinement and settlement contract v1

Status: **design only. No code lands with this document.** Every execution
claim in this file is `HUMAN_BLOCKED: needs a Windows host` -- this authoring
session ran on macOS arm64, has no Windows machine, no provisioned Windows CI
runner, and no cross-compilation or emulated substitute is treated as
Windows evidence anywhere below. This document is the Windows half of the
split [Issue #61](https://github.com/wavect/semaprax/issues/61) asked for;
the macOS half is
[DOCTOR-PRODUCTION-PROVISIONER-MACOS-V1](DOCTOR-PRODUCTION-PROVISIONER-MACOS-V1.md),
which does carry real local execution evidence because that authoring session
ran on macOS.

Audience: release engineers, platform maintainers, and security reviewers
with access to a real Windows host or a Windows CI runner.

## Why this document has no accompanying code

[DOCTOR-PRODUCTION-PROVISIONER-V1](DOCTOR-PRODUCTION-PROVISIONER-V1.md)
states that "macOS and Windows need separate native confinement and
settlement contracts" and that "Linux evidence never promotes those hosts." A
prior session on this repository already established the adjacent principle
this document holds to: it "correctly refused to substitute a mingw
cross-compile for real Windows evidence, reasoning it would be 'weaker
evidence dressed as stronger'." This authoring session extends that same
refusal one step earlier, to authorship: this host has no `rustup`, no
`windows-pc-msvc`/`windows-pc-gnu` target, and no Windows toolchain of any
kind (`rustc --version` here reports a single Homebrew-installed macOS arm64
toolchain with no other installed target). It therefore cannot even
type-check new Windows-only source, let alone execute it. Shipping unverified
`unsafe` FFI into `windows.rs`/`windows/launch.rs` -- files that already
compile and pass on real Windows CI today (`.github/workflows/ci.yml`'s
`windows-2025` matrix legs) -- risks silently breaking a currently-green
Windows build with code nobody in this session could check. That would be a
worse outcome than authoring the contract alone and leaving the primitive
itself for a session (or reviewer) with real Windows execution ability.

Separately, `Cargo.toml` is outside this session's lease (`DO NOT TOUCH`
list), and the concrete primitive sketched below needs at least one
`windows-sys` feature (`Win32_Security_Isolation`, for `CreateAppContainerProfile`
and its SID accessor, if that route is chosen -- see
[Filesystem confinement](#filesystem-confinement)) that is not already
enabled. Even a session willing to write unverified code could not complete
this primitive without a `Cargo.toml` change outside its lease. Both
constraints point the same way: this document specifies the exact contract
and the exact calls a Windows-capable session should add, not a diff.

## Current state (unchanged by this document)

`crates/semaprax-native-rust-interop-platform-sys/src/doctor/windows.rs` (296
lines) and `windows/launch.rs` (221 lines) already implement the *ordinary
probe's* launch path: a process created suspended
(`CREATE_SUSPENDED`), assigned to a fresh, non-breakaway-eligible job object
(`CreateJobObjectW` + `SetInformationJobObject` with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) via `AssignProcessToJobObject` *before*
`ResumeThread` lets any target code run, with settlement observed through
`QueryInformationJobObject(JobObjectBasicAccountingInformation)`'s
`ActiveProcesses` field reaching zero. That is real job-object confinement of
process *lifetime and settlement observation*. It is not filesystem or
network confinement, does not restrict the child's security token, and is not
the production provisioner: there is no sealed-capsule consumption, no
integrity-level or token restriction, and no settlement boundary beyond the
one job object the ordinary probe itself creates.

This document builds on that existing plumbing rather than inventing a
parallel one, per the issue's explicit request.

## Confinement primitive (proposed; unverified)

Windows has no namespace or cgroup-v2 equivalent. The three building blocks
this contract proposes, all already partially present in `windows-sys`'
enabled feature set (`Win32_Foundation`, `Win32_Security`,
`Win32_Storage_FileSystem`, `Win32_System_JobObjects`,
`Win32_System_Threading`, `Wdk_Foundation`, `Wdk_Storage_FileSystem`) or one
feature away from it, are:

### 1. Job-object limits, tightened

Extend the existing `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` call in
`windows/launch.rs` (currently setting only `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`)
to additionally set:

- `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` with `BasicLimitInformation.ActiveProcessLimit`
  set to a small fixed bound (one, for a tool invocation with no expected
  descendants; the doctor collector's existing hostile-input philosophy would
  treat a tool that spawns a second process as something to *observe and
  reject*, not silently accommodate). A limit violation terminates the whole
  job, which is the Windows analog of the Linux contract's
  `memory.oom.group = 1`: an overshoot kills the whole scope rather than
  refusing cleanly mid-invocation.
- `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`, so a crashing tool cannot
  leave a debugger-attachable faulted process alive inside the job.
- A `JOBOBJECT_BASIC_UI_RESTRICTIONS` call (`SetInformationJobObject` with
  `JobObjectBasicUIRestrictions`) denying `JOB_OBJECT_UILIMIT_HANDLES`,
  `JOB_OBJECT_UILIMIT_READCLIPBOARD`, `JOB_OBJECT_UILIMIT_WRITECLIPBOARD`,
  `JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS`, `JOB_OBJECT_UILIMIT_DESKTOP`,
  `JOB_OBJECT_UILIMIT_DISPLAYSETTINGS`, `JOB_OBJECT_UILIMIT_GLOBALATOMS`,
  and `JOB_OBJECT_UILIMIT_EXITWINDOWS`.

All of the above are exact fields/constants of `Win32_System_JobObjects`,
already an enabled feature; a Windows-capable session should be able to add
these without a `Cargo.toml` change.

### 2. A restricted token

Before `CreateProcessW`, build a restricted token with `CreateRestrictedToken`
(`DISABLE_MAX_PRIVILEGE`, and an explicit `SidsToDisable` list covering the
caller's own logon SID so the child cannot access the interactive desktop's
resources by inherited identity) and pass it via
`CreateProcessAsUserW`/`CreateProcessWithTokenW` instead of the current
identity-inheriting `CreateProcessW`. `Win32_Security` is already an enabled
feature; `CreateRestrictedToken` and `DISABLE_MAX_PRIVILEGE` live in that same
module in `windows-sys`, so this should not need a `Cargo.toml` change either
-- but this document does not assert that without having compiled it.

### 3. Filesystem confinement

This is the one building block genuinely open between two designs, and this
document deliberately does not pick one without a Windows-capable session
able to validate it:

- **A restricted-ACL scratch root**: create one directory per invocation with
  an explicit DACL granting only the restricted token's SID read/write, deny
  everything else, and confine the tool's working directory and any
  environment-visible temp-directory variables (`TEMP`, `TMP`) to it. This
  needs no new `windows-sys` feature beyond what ACL/SID manipulation already
  requires under `Win32_Security`.
- **An AppContainer profile**: `CreateAppContainerProfile` plus an explicit
  capability SID list, passed to `CreateProcessW` via
  `PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES`. This is closer in spirit to
  macOS's Seatbelt profile (a named, kernel-enforced container) but needs
  `Win32_Security_Isolation`, which is **not** in the crate's currently
  enabled `windows-sys` feature list -- a `Cargo.toml` change outside this
  session's lease.

A Windows-capable session should pick one, document why, and record it here
as a revision to this section -- exactly the kind of design decision the
issue asks be made before code lands, not inferred from either platform's
existing shape.

## Sealed input and process handoff

[DOCTOR-SEALED-INPUT-V1](DOCTOR-SEALED-INPUT-V1.md)'s sealed-carrier concept
(`F_SEAL_*` on a Linux memfd) has no Windows equivalent primitive at the OS
level; the closest analog is an anonymous, unnamed file mapping
(`CreateFileMappingW` with `NULL` name and no `FILE_MAP_WRITE` reopen path
after initial population) combined with the existing crate's own bounded-copy
and digest-comparison discipline, reused verbatim rather than re-derived. This
document does not specify that mapping's exact flag set; it requires that
whoever implements it hold the same invariants
`DOCTOR-SEALED-INPUT-V1` proves on Linux: immutability after creation,
provable to the acquiring code without trusting the writer, and a closed set
of `create`/`acquire` operations with no ambient pathname discovery.

## Settlement contract

The same four-outcome shape this repository's macOS contract uses is proposed
here, for consistency across both non-Linux platforms, though this document
does not claim the two share an implementation:

```rust
enum Settlement {
    Completed,
    Failed(FailureReason),      // ExitCode(u32) | job-limit-violation
    Cancelled,                  // supervisor deadline fired
    Uncertain(UncertainReason), // wait/query failure, or ActiveProcesses > 0
                                 // after the leader is observed to have exited
}
```

**Settled** for a Windows job object means: `WaitForSingleObject` on the
leader process handle returns `WAIT_OBJECT_0`, *and* a subsequent
`QueryInformationJobObject(JobObjectBasicAccountingInformation)` reports
`ActiveProcesses == 0` -- mirroring the existing ordinary-probe `Child::settle`
in `windows.rs`, which already performs exactly this check
(`accounting.ActiveProcesses == 0`) after `TerminateJobObject`. This document's
one addition to that existing logic is the *sticky, four-way* outcome
distinction: the current ordinary probe only distinguishes "settled" from
"abort the whole harness process" (`std::process::abort()` on any observation
failure), which is correct for a bounded developer-machine probe but is not
the same as recording `Completed` vs `Failed` vs `Cancelled` vs `Uncertain` as
data a caller can inspect and act on differently, per this repository's
non-negotiable invariant that "a settlement or concurrency model is proof
data, not permission to perform a physical finalizer." A production
provisioner needs to report *which* of the four happened, not merely succeed
or abort the whole process.

Failure selection must be sticky here exactly as macOS's `StickySettlement`
enforces: once `Failed`, `Cancelled`, or `Uncertain` is selected, no later
observation -- including a job-object accounting reread ostensibly proving
`ActiveProcesses == 0` -- may replace it with `Completed`.

## Proposed gate workflow (not landed; `.github/workflows/**` is out of scope
## for this authoring session)

Unlike the macOS contract, this document proposes a `workflow_dispatch`-only
gate, per the issue's explicit request, because the confinement primitive
above -- once implemented -- mutates job-object limits, restricted tokens,
and possibly an AppContainer profile or ACL'd filesystem root, none of which
this authoring session can characterize as non-destructive to a shared runner
without real execution evidence. The proposed shape mirrors
`.github/workflows/doctor-provisioned-linux.yml`:

```yaml
name: Doctor provisioned Windows gate

on:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: doctor-provisioned-windows
  cancel-in-progress: false

jobs:
  doctor-provisioned-windows:
    name: Windows offline doctor lifecycle (provisioned)
    runs-on: windows-2025
    timeout-minutes: 90
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
        with:
          fetch-depth: 1
          filter: blob:none
          fetch-tags: false
          lfs: false
      - uses: dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772 # master
        with:
          toolchain: 1.97.1
      - name: Prove the gate refuses absent provisioning
        run: python scripts/doctor-provisioned-windows-gate.py --self-test
      - name: Run the confinement primitive's hostile-input corpus
        run: cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys --lib doctor::windows_confinement
```

`scripts/doctor-provisioned-windows-gate.py` does not exist yet; it should be
authored analogous to `scripts/doctor-provisioned-linux-gate.py`'s
`--self-test`/pure-decision-logic pattern (fail-closed, never a skip, and
self-testable on any host including this one) by whoever implements the
primitive above, since its exact precondition list depends on which
filesystem-confinement design (restricted ACL vs AppContainer) is chosen.
Neither the workflow file nor the gate script is added by this document; both
are specified here as the exact delta a maintainer should land.

## Acceptance criteria status

See the coordinating session's report for the full criterion-by-criterion
table. In summary: the versioned contract this document is now exists and is
cross-referenced from `DOCTOR-PRODUCTION-PROVISIONER-V1`; the confinement
primitive and its hostile-input tests do **not** yet exist in the owning
crate (`HUMAN_BLOCKED: needs a Windows host and toolchain`, and a
`Cargo.toml` feature addition outside this session's lease for the
AppContainer route if chosen); the gate is authored here as a proposal only
and has never been run; and no completion-matrix promotion is claimed.

## Nonclaims

This contract does not: implement or compile any of the primitives it
describes; claim the existing ordinary-probe job-object confinement in
`windows.rs` as evidence of production-grade sandboxing (it confines process
*lifetime*, not filesystem or network access, and was not designed as a
security boundary); claim Linux or macOS evidence proves anything about
Windows; run on a cross-compiled or emulated target as a substitute for real
Windows execution; wire any new path into the CLI; or promote
`docs/COMPLETION-MATRIX.md` WP-05 for Windows. It records the design decisions
a Windows-capable session needs to implement this contract, and the exact
places (job-object limits, token restriction, filesystem confinement, sealed
input, settlement accounting, gate script) that implementation must land.
