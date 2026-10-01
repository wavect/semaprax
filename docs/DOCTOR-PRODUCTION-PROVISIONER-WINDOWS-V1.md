# Windows doctor confinement and settlement contract v1

Audience: release engineers, platform maintainers, and security reviewers
with access to a real Windows host or a Windows CI runner.

Status: the `#[cfg(windows)]` primitive has hosted type-check evidence and a
historical five-test runtime witness on exact checkout `3d4220b6`, extending the earlier
two-test witness at `c6bf9902`. The historical signed-capsule nine-case exact selector
(six runtime cases plus three admission refusals) passed on exact checkout
`c608b8d8`. The historical ten-case selector added a hostile inheritable-parent-ACE
DACL case and passed on exact checkout `f4d3291f`. The prior eighteen-case
selector preserves those ten and adds signed-image substitution, writable
mapping, handle-settlement, and read-only request/bundle-carrier regressions;
all eighteen passed on the provisioned Windows runner at exact checkout
`06c0090d9` in [run 36911767583](https://github.com/wavect/semaprax/actions/runs/36911767583).
The expanded twenty-two-case selector adds retained-section refusal settlement
and anonymous carrier handle-lifetime cases. All 22 passed, with none ignored,
at `e15c16202` in [run 36919771375](https://github.com/wavect/semaprax/actions/runs/36919771375).
Exact exclusion of retained writable-section mutation remains unresolved. The
authoring host remains macOS arm64; the hosted run is the native runtime witness.
No cross-compilation or emulated substitute is treated as native execution
evidence. Host-independent capsule, admission-ordering,
and settlement logic remains separately testable on non-Windows hosts. See
[Hosted Windows compilation evidence](#hosted-windows-compilation-evidence-type-check-only),
[Hosted Windows runtime evidence](#hosted-windows-runtime-evidence), and
[Windows runtime gate](#windows-runtime-gate) for the exact evidence ceiling.

The separate [macOS contract](DOCTOR-PRODUCTION-PROVISIONER-MACOS-V1.md)
has real local execution evidence; it is not Windows evidence.

## Hosted Windows compilation evidence (type-check only)

The earlier "never compiled" statement is no longer accurate. Hosted
[run 35462242188](https://github.com/wavect/semaprax/actions/runs/35462242188)
checked out `7cab8aa8fa67d412fa82643ab8139cbd9eb00b43`, which includes the
Windows confinement module and `94adc21b`'s later
`GetCurrentProcess` import correction to `primitive.rs`. Its overall workflow
conclusion was failure for unrelated jobs, but the successful
[Public Native Rust SDK v1 (windows-latest) job 105948054658](https://github.com/wavect/semaprax/actions/runs/35462242188/job/105948054658)
compiled `semaprax-native-rust-interop-platform-sys` on Windows Server 2025
while running this exact command:

```text
cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys --lib tests::windows_archive::windows_real_brepro_archive_round_trips_through_exact_admission -- --exact --nocapture --test-threads=1
```

The selected `tests::windows_archive` test caused successful library
compilation, which type-checks the `#[cfg(windows)]`
`doctor::windows_confinement::primitive` source present in that checkout. It
executed no `doctor::windows_confinement` function: it did **not** create a
restricted token, start a confined child, assign a job, apply an ACL, or
observe settlement. It is compilation evidence only, not a confinement or
hostile-input execution witness. `7cab8aa8` is an ancestor of the tree audited
for this update; no claim is made about a later unrun commit.

## Hosted Windows runtime evidence

Hosted [run 35986090171](https://github.com/wavect/semaprax/actions/runs/35986090171)
executed the original two-test Windows runtime selector successfully (2
passed, 0 failed) on exact checkout
`c6bf9902966f5c1fda0c8e70687c261ffacf37c4`. It covers the restricted-token,
ACL/job success and timeout-settlement cases described below. Hosted
[run 35988348061](https://github.com/wavect/semaprax/actions/runs/35988348061),
Windows Server 2025 job `107596231695`, then ran the expanded exact selector
on `3d4220b633283e76c277f6042550275c8f1c3327`: **5 passed, 0 failed,
0 ignored, 113 filtered**. Its three added cases exercised an actual
test-admitted job descendant during timeout, nonzero-exit classification and
cleanup, and repeated filesystem-stage refusal with stable handle count.
Those earlier runs do not establish signed-capsule admission. Hosted
[run 35992373373](https://github.com/wavect/semaprax/actions/runs/35992373373),
Windows Server 2025 job `107609259218`, executed the signed-capsule selector
on exact checkout `c608b8d8920816497c84be72ea9b0b5dc05dd253`:
**9 passed, 0 failed, 0 ignored, 114 filtered**. Six runtime cases used a
deterministic test-only signing key; three admission cases refused a missing
anchor, a bad signature, and both signed Linux architecture codes before
token/job/filesystem effects. The preceding run `35992165688` at `3b790471`
failed at compilation in a Windows-only test fixture; it executed no runtime
case. Neither result establishes release trust, artifact binding, or general
Windows support.
Hosted [run 35993882814](https://github.com/wavect/semaprax/actions/runs/35993882814),
Windows Server 2025 job `107614131433`, executed the exact ten-case selector
on `f4d3291f1f8a261d5c86f0d6c6aaa70ca8779ad2`: **10 passed, 0 failed,
0 ignored, 114 filtered**. The new case put a verified broad inheritable ACE
on a private parent and observed a protected child scratch DACL with only its
one intended explicit ACE. This is that fixture's observation, not a general
filesystem-isolation or production-support claim.

## Why the first revision had no accompanying code, and why this one does

[DOCTOR-PRODUCTION-PROVISIONER-V1](DOCTOR-PRODUCTION-PROVISIONER-V1.md)
states that "macOS and Windows need separate native confinement and
settlement contracts" and that "Linux evidence never promotes those hosts." A
prior session on this repository already established the adjacent principle
this document holds to: it "correctly refused to substitute a mingw
cross-compile for real Windows evidence, reasoning it would be 'weaker
evidence dressed as stronger'." The document's first revision extended that
same refusal one step earlier, to authorship, on the reasoning that shipping
unverified `unsafe` FFI into `windows.rs`/`windows/launch.rs` -- files that
already compile and pass on real Windows CI today
(`.github/workflows/ci.yml`'s `windows-2025` matrix legs) -- risked silently
breaking a currently-green Windows build with code nobody in that session
could check.

This revision reaches a different conclusion for the same host constraint,
for two reasons the coordinating session judged sufficient to proceed:

1. The new Win32 wiring lands in a **new, standalone module**
   (`doctor::windows_confinement`, mirroring `doctor::darwin_confinement`'s
   existing shape), not in `windows.rs`/`windows/launch.rs` themselves. Those
   two files, and the ordinary `--version` probe they implement, are
   byte-for-byte unchanged by this revision -- the currently-green Windows CI
   legs that build and run them are not touched by anything this revision
   adds.
2. Every symbol this revision's Win32 code calls -- every function
   signature, struct field, and constant -- was cross-checked against the
   exact vendored `windows-sys = "=0.61.2"` source this crate's `Cargo.toml`
   already pins, using only the `windows-sys` features that `Cargo.toml`
   (outside this session's lease) already enables. That is real diligence,
   raised confidence before a Windows toolchain compiled it. The later hosted
   compilation witness above now proves type-checking for that exact checkout;
   it remains **not** a substitute for executing the confinement primitive.

The `Cargo.toml` constraint from the first revision still holds: the
AppContainer filesystem-confinement route needs `Win32_Security_Isolation`,
not enabled today and outside every session's lease so far, so this revision
implements the restricted-ACL scratch-root alternative instead -- see
[Filesystem confinement](#3-filesystem-confinement).

## Current state (unchanged by the first revision; extended by this one)

`crates/semaprax-native-rust-interop-platform-sys/src/doctor/windows.rs` (296
lines) and `windows/launch.rs` (221 lines) still only implement the *ordinary
probe's* launch path, exactly as the first revision described: a process
created suspended (`CREATE_SUSPENDED`), assigned to a fresh,
non-breakaway-eligible job object (`CreateJobObjectW` +
`SetInformationJobObject` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) via
`AssignProcessToJobObject` *before* `ResumeThread` lets any target code run,
with settlement observed through
`QueryInformationJobObject(JobObjectBasicAccountingInformation)`'s
`ActiveProcesses` field reaching zero. Neither file is modified by this
revision.

Alongside them, `crates/semaprax-native-rust-interop-platform-sys/src/doctor/windows_confinement/`
now exists, reusing that same job-object *shape* without importing
`windows.rs`'s private types (exactly as `doctor::darwin_confinement` does not
import `doctor::unix::launch::darwin`'s private types):

- `capsule.rs` -- host-independent structural hostile-input corpus plus
  Windows runtime delegation to the shared signed-capsule parser and release
  trust anchor. Structural parsing is not runtime admission evidence.
- `refusal.rs` -- host-independent, order-enforcing admission classifier (no
  Win32 call; compiles and its ordering tests run on every host).
- `settlement.rs` -- host-independent sticky settlement state machine (no
  Win32 call; compiles and its tests run on every host).
- `primitive.rs` -- `#[cfg(windows)]` restricted token, tightened job object,
  ACL'd scratch root, sealed-capsule-gated suspended spawn, and settlement
  observation. Never compiled or executed on this authoring host; it has the
  historical Windows witnesses above; the new image binding is unexecuted. See its
  own module documentation for the exact simplifications it makes and the
  specific claims it does and does not make about itself.

This is still a **standalone confinement primitive**, not the production
provisioner: it is not wired into any ordinary CLI route or into
`provisioned_doctor_*`, per the issue's explicit request.

## Confinement primitive (implemented; hosted type-checked, limited runtime evidence)

Windows has no namespace or cgroup-v2 equivalent. The three building blocks
this contract proposed, all already partially present in `windows-sys`'
enabled feature set (`Win32_Foundation`, `Win32_Security`,
`Win32_Storage_FileSystem`, `Win32_System_JobObjects`,
`Win32_System_Threading`, `Wdk_Foundation`, `Wdk_Storage_FileSystem`) or one
feature away from it, are now implemented as described below in
`crates/semaprax-native-rust-interop-platform-sys/src/doctor/windows_confinement/primitive.rs`.
The image-binding continuation adds `Win32_System_Ioctl` for the read-oplock
preflight and `Win32_System_Memory` for its hostile writable-mapping fixture.
No dependency version changes.

### 1. Job-object limits, tightened

Implemented as `primitive::tightened_job`, which builds its own job object
(distinct from `windows/launch.rs`'s, per the "standalone primitive" scope
above) and sets:

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
already an enabled feature; no `Cargo.toml` change was needed.

### 2. A restricted token

Implemented as `primitive::restricted_token`, narrower than first proposed:
it calls `CreateRestrictedToken` with `DISABLE_MAX_PRIVILEGE` and empty
disable/delete/restrict lists (not the fuller "also disable the caller's own
logon SID" refinement this document originally proposed). That fuller
refinement needs walking the calling token's `TokenGroups` to find the
`SE_GROUP_LOGON_ID` entry -- a variable-length structure this session judged
too easy to get subtly wrong before any Windows compilation, let alone runtime
execution. `DISABLE_MAX_PRIVILEGE` alone is still a real, meaningful restriction
(the resulting token holds no privileges at all), and the logon-SID
refinement is left as an explicit follow-up for a Windows-capable session,
recorded in `primitive.rs`'s own module documentation. `Win32_Security` is
already an enabled feature; no `Cargo.toml` change was needed.

The primitive uses `CreateProcessAsUserW`, not `CreateProcessWithTokenW`: the
former was judged to have the more directly verifiable calling convention
against the vendored source for this exact use (a token this process itself
just created, not one obtained through a separate logon).

### 3. Filesystem confinement

This session picked the **restricted-ACL scratch root** design, not an
AppContainer profile, exactly along the line the first revision drew:
`Win32_Security_Isolation` (needed for `CreateAppContainerProfile`) is still
not an enabled `windows-sys` feature, and `Cargo.toml` is still outside every
session's lease so far. Implemented as `primitive::confined_scratch_root`:
one fresh directory per invocation, with a DACL built from `InitializeAcl` +
`AddAccessAllowedAceEx` granting only the restricted token's own user SID
(read via `GetTokenInformation(TokenUser)`) `FILE_GENERIC_READ |
FILE_GENERIC_WRITE | DELETE`; no other principal is listed, which is an
implicit deny under Windows DACL evaluation. The tool's working directory and
its `TEMP`/`TMP` environment variables are confined to this directory
(`primitive::forced_environment`). This needs no `windows-sys` feature beyond
`Win32_Security`, already enabled.

The new `windows_runtime_protected_scratch_dacl_blocks_inherited_parent_ace`
case creates a private parent with one `FILE_ALL_ACCESS` ACE marked
inheritable to files and directories, verifies that fixture, then checks that
the actual scratch child DACL is protected and contains only its one explicit
restricted-token-user ACE with no inherited flags. This selected case tests
that specific inheritance boundary; it does not prove general Windows
filesystem isolation. Output capture remains file-based (two fixed-name log
files inside the same ACL'd directory) rather than the pipe-based,
attribute-list-restricted handle inheritance
`windows/launch.rs` uses for the ordinary probe -- a deliberate simplification
to keep the new unsafe surface reviewable without a toolchain, recorded in
`primitive.rs`.

## Sealed input and process handoff

[DOCTOR-SEALED-INPUT-V1](DOCTOR-SEALED-INPUT-V1.md)'s sealed-carrier concept
(`F_SEAL_*` on a Linux memfd) has no Windows equivalent primitive at the OS
level; the closest analog remains an anonymous, unnamed file mapping
(`CreateFileMappingW` with `NULL` name and no `FILE_MAP_WRITE` reopen path
after initial population), and this revision does not implement it -- that
half of the sealed-input contract (the immutable carrier itself) is still
open, exactly as the first revision left it.

Production Windows admission uses shared `semaprax-doctor-capsule::parse_signed`
with the compile-time `SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX` anchor, then
requires native Windows architecture code 3 (x86-64) or 4 (AArch64) before
token, job, or filesystem effects. Missing or invalid trust material and
signature failure refuse closed. The shared v1 architecture mapping is
specified in [Linux production provisioner v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md#signed-release-capsule)
and [sealed input v1](DOCTOR-SEALED-INPUT-V1.md#capsule-wire-architecture).
The structural decoder remains only as a non-authoritative malformed-wire test
utility; production does not fall back to it. The deterministic test key is
not a release anchor. The standalone spawn now requires an explicit `ImageRole`
(launcher, worker, collector), checks the selected signed length and SHA-256 against held executable
bytes, and retains file/path handles and an advisory oplock through settlement.
Exact binding under retained writable-section mutation remains unresolved.
Request/bundle carrier transport, selector/role handoff and Windows production transport remain unimplemented;
this is not complete signed-carrier admission or production support.

### Anonymous request/bundle carrier experiment

`windows_confinement::carrier` is an isolated Windows-only experiment. It
accepts explicit bytes only when they match a signed capsule `Artifact`'s exact
nonzero bounded length and SHA-256 digest. It copies the authenticated bytes to
an unnamed paging-file mapping, unmaps its sole writable view, duplicates only
`SECTION_MAP_READ` into an inheritable handle, drops the writable handle, and
rehashes from the retained read-only handle before returning. The mapping name
is `NULL`; no path, named-object lookup, child input selection, process launch,
or production transport is involved.

The selected native case makes both request-shaped and bundle-shaped bytes,
rejects a forged artifact digest, asks the kernel for `FILE_MAP_WRITE` through
each retained inheritable handle, and requires that request to fail while a
read-only rehash still succeeds. It also requires exact warmed handle-count
settlement. This shows that an explicit future handle-list entry can carry
read-only authenticated bytes; it does not show that any child inherited the
handle, received a fixed carrier role, or executed an authenticated image.

### Signed image binding continuation (#333; partial native runtime evidence)

`primitive/image.rs` implements a bounded pathname bridge because
[`CreateProcessAsUserW`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw)
requires an application name. After signed capsule/OS admission and before any
token, job, scratch creation or process effect, it:

1. Opens the selected image without write/delete sharing, rejects nonregular,
   reparse or multiply linked files, and requires NTFS.
2. Acquires its normalized volume-GUID path, avoiding drive-letter lookup, and
   holds every ancestor open without write/delete sharing or reparse traversal.
   It reopens the leaf through those pinned components and compares the held
   volume/file identity to the original before reading any image bytes.
3. Requires a Read oplock grant on the original asynchronous handle. Microsoft's
   [grant conditions](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/granting-oplocks)
   exclude writable user-mapped sections at grant time. The pending request,
   original file, event and stable boxed buffers now stay owned through
   suspended process creation and child settlement. The guard checks for an
   observed break after hashing, before creation and before resume. Drop
   cancels and drains that exact request before releasing its buffers;
   cancellation alone is never treated as I/O completion. Unsupported
   filesystems, absent oplock support, or an observed break refuse with
   `Capsule(ArtifactBinding)`. An unexpected wait/handle error without observed
   I/O completion terminates the host process instead of returning with live
   kernel pointers into released buffers.
4. Streams exactly the signed bounded length through SHA-256 from the held
   synchronous reader. Mismatched length/digest refuses with the same stable
   class. The final application name is the pinned GUID path, and the image and
   ancestor handles remain held until the child's settlement/drop completes.
5. Checks the still-suspended child's `QueryFullProcessImageNameW` native name
   against the admitted held file's native name before `ResumeThread`. A
   redirected image (including a host IFEO debugger policy) or query failure
   causes job termination and observed leader exit before return. This check
   follows suspended process creation but precedes target code execution; it
   does not reopen or trust the child's reported pathname.

**Exact race-resistant image binding remains unresolved.** A retained writable
section object whose file handle has closed needs separate admission and race
coverage, including the state before any writable view exists. Microsoft's
[section-synchronization rules](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/fs-filter-acquire-for-section-synchronization2)
say writable-section operations can break Read/Read-Handle/Read-Write oplocks
without acknowledgment and proceed immediately. Keeping the request alive and
checking its event therefore provides detection and defense in depth, not an
atomic exclusion proof. Writable-section mutation can still race these checks;
#333's exact binding acceptance remains open. The initial continuation's
cancel-before-hash/pre-spawn design did not establish this guarantee either.

These partial checks rely on the ordinary NTFS sharing and oplock contract and
the trusted Windows kernel/volume namespace. It does not authenticate imported DLLs or
protect against administrator/kernel mutation. It adds no ordinary CLI route
or Windows request/bundle carrier transport, and does not modify the Linux sealed-file
launcher. The deterministic test capsule signs the actual test image's bytes;
its other slots remain fixture-only inputs and are not transport evidence.

The seven new selected cases exercise signed length/digest/role mismatch before
launch, denied post-binding leaf deletion/write/rename/hardlink creation and
ancestor rename, a failed post-binding writable-section request from a newly
opened read handle, pre-existing writable handles and hardlinks, a writable
mapping whose handles have both closed, a retained PAGE_READWRITE section
without any view, drop-time process/image handle settlement, and an explicitly
inheritable delete-on-close sentinel that must not reach the child outside its
three standard handles. The post-binding section request only demonstrates
that a new read-only handle cannot mint writable-section access; it does not
address an already retained writable section. The section-without-view case
requires pre-spawn refusal; that selected behavior passed on Windows at
`06c0090d9`.
Success controls require NTFS/oplock acquisition to work; no unavailable
prerequisite can pass by skipping. All eighteen selected native cases passed
at `06c0090d9`; later source revisions need their own execution receipt.

Local verification on 30 September 2026: the initial `64472c71b` continuation
and the retained-oplock correction both passed the Windows-target check below
(the correction additionally used `--offline`):
`cargo check --locked -p semaprax-native-rust-interop-platform-sys --tests
--target x86_64-pc-windows-msvc` passed on macOS using the preinstalled target
standard library. This type-checks the Windows library and test source
but executes no Windows code. The corrected sixteen-case Windows gate's
`--self-test` passed. The initial Linux gate's `--self-test` passed 137/137 checks, including the explicit exclusion
of the new Windows-only test module. This earlier verification predates the
eighteen-case [native run](https://github.com/wavect/semaprax/actions/runs/36911767583).


### Binding decision and next implementation boundary (#333)

Source/API review on 30 September 2026 retains the current partial primitive
and leaves exact binding unaccepted. This is a design decision and work plan,
not a new execution receipt or a claim that the race has been reproduced.
The affected completion row is WP-05. Its Windows production boundary stays
unpromoted; the historical ten-case evidence remains attached to its original
revision. The eighteen-case selector passed at exact checkout `06c0090d9`;
that narrows the runtime-evidence gap but does not prove atomic exclusion of a
retained writable section or promote WP-05.

The proposed repair must establish one continuous invariant: from the first
authenticated byte read until the loader has consumed the admitted image,
no untrusted holder can change those bytes or substitute the consumed object.
The proof must cover an already-created writable section with no current view,
not just open writers and existing mapped views. A matching native pathname
and a clean event observation do not establish that invariant.

The following shortcuts are rejected by this review:

- Changing to a Read-Handle, Read-Write or Read-Write-Handle oplock does not
  supply a writable-section barrier: the documented section-synchronization
  rule above applies to all four request types and requires no acknowledgment.
- Adding more hashes or event checks leaves an interval after the last check.
  Checking a suspended child's name also establishes no byte identity.
- A byte-range lock cannot close this gap: Microsoft's
  [LockFileEx contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex)
  explicitly permits access through mapped views despite a file lock.
- A fresh copy followed by closing its writer and reopening read-only needs
  a separate authority argument for that transition. Keeping the writer open
  while calling [ReOpenFile](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-reopenfile)
  with no write sharing conflicts with the existing write access. Random names
  or an ACL granting the same effective SID are not themselves a proof against
  another process with that SID. A new broker identity would change the
  authority boundary and must be specified and authorized separately.

The next bounded batch is a **native mechanism experiment**, in the existing
`primitive::tests::binding` harness. It must precede a production guard change:

1. Extend the existing no-view writable-section fixture with bounded event
   handshakes. Retain the section while closing the original writer; attempt
   view creation and mutation at each explicit boundary: after image guard
   acquisition, after digest validation, before process creation, and while
   the leader remains suspended before its final check/resume. Admission
   refusal must occur before the mutation hook; if admission succeeds, the
   test must actually reach the hook and attempt the mutation. Record which
   path occurred. Do not turn an unavailable setup into a passing skip.
2. Evaluate a separately owned image-section guard created from the held file
   with `CreateFileMappingW(PAGE_READONLY | SEC_IMAGE)` and an owned mapped
   view, retained through settlement. This is an experiment, not an approved
   repair: the [documented mapping API](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-createfilemappingw)
   describes PE-derived page protections, but does not by itself prove that a
   later pathname launch consumes those exact authenticated bytes. Establish
   exclusion of retained writable sections, file-cache/image-section
   coherence, loader object identity and all view/handle lifetimes before
   proposing `HeldImage` integration. Hash the signed raw file layout; hashing
   relocated image pages is not the capsule's artifact digest.
3. Use a valid PE fixture with a known bounded, same-length mutation location;
   verify the hostile writer changes the intended bytes in an unguarded
   control. A malformed PE rejected by the loader is not binding evidence.
   For any successful protected launch, require the authenticated fixture's
   behavior, observed leader exit, zero job members, exact scratch cleanup,
   and the warmed parent handle count. For a refusal, require the stable
   `Capsule(ArtifactBinding)` class and no resumed leader. Retain separate
   assertions for pre-process refusal and suspended-process settlement.
4. Add the exact cases to `EXPECTED_TESTS` in
   `scripts/doctor-provisioned-windows-gate.py`, preserve its existing twenty-two
   names, and update its count/parser controls and the Linux gate's explicit
   Windows-only exclusion if a new test submodule is added. Keep experiment
   results and release acceptance separate; repeated stress success alone
   cannot prove the invariant.

This continuation binds `bInheritHandles` to an explicit three-handle startup
list and adds a child probe of an unrelated inheritable delete-on-close
sentinel. Its selected native case passed at `06c0090d9` but checks only a
single capability boundary. Inspecting job flags does not exercise resource exhaustion:
CPU, committed-memory and output limits still need specified bounds, actual
violating children, selected failure classes, and post-failure job/handle
settlement. The current `tightened_job` sets an active process limit and
kill-on-close behavior, not CPU or memory bounds. Add those bounds through this
owning contract before claiming the resource corpus is complete; preserve the
existing production one-process and test-only two-process descendant
distinction.

Finally, `confined_spawn_after_binding` currently receives an executable
pathname and arguments, not held request/bundle carriers. Image repair cannot
satisfy the ticket's request/bundle substitution acceptance on its own. A
follow-on entry point must consume authenticated bounded carrier objects,
bind their signed slots and selector/role, and expose only the declared child
handle inventory. Specify that transport and its refusal ordering before
adding a production route.

Verification required for the implementation batch: the Windows target
library/test type-check, both provisioned gate self-tests, the complete native
Windows selector on the changed revision, and the repository full profile.
This design-only review ran no Cargo, build, test, or native Windows command;
none of those gates gains new evidence from this section.

## Settlement contract

The same four-outcome shape this repository's macOS contract uses is now
implemented in
`crates/semaprax-native-rust-interop-platform-sys/src/doctor/windows_confinement/settlement.rs`,
for consistency across both non-Linux platforms, though the two do not share
an implementation:

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
timeout route gives a successful `TerminateJobObject` one fixed five-second
grace to observe the leader signaled, then allows one additional fixed
five-second interval for repeated job accounting to reach zero. A leader that
remains live is `Uncertain(KillWaitTimedOut)`; if the job remains nonempty
after the second bound, settlement is `Uncertain(ActiveProcessesNonZero)`, not
settled cancellation.
The contract also requires the *sticky, four-way* outcome
distinction: the current ordinary probe only distinguishes "settled" from
"abort the whole harness process" (`std::process::abort()` on any observation
failure), which is correct for a bounded developer-machine probe but is not
the same as recording `Completed` vs `Failed` vs `Cancelled` vs `Uncertain` as
data a caller can inspect and act on differently, per this repository's
non-negotiable invariant that "a settlement or concurrency model is proof
data, not permission to perform a physical finalizer." A production
provisioner needs to report *which* of the four happened, not merely succeed
or abort the whole process.

Failure selection is sticky here exactly as macOS's `StickySettlement`
enforces, via the same `select`/`resolve` shape in `settlement.rs`'s own
`StickySettlement`: once `Failed`, `Cancelled`, or `Uncertain` is selected, no
later observation -- including a job-object accounting reread ostensibly
proving `ActiveProcesses == 0` -- may replace it with `Completed`. This
state-machine logic has no Win32 dependency and its sticky-selection tests run
on every host this crate builds on; `primitive::settle_confined`
(`#[cfg(windows)]`, unexecuted here) is the code that actually observes a real
job object and drives it.

## Windows runtime gate

The dispatch-only
[`.github/workflows/doctor-provisioned-windows.yml`](../.github/workflows/doctor-provisioned-windows.yml)
uses an ephemeral `windows-2025` runner and creates a fresh, explicit scratch
parent under `RUNNER_TEMP`. The gate fails when the host is not 64-bit Windows,
the parent is missing, nonempty, or a reparse point, Cargo fails, any named
test is filtered or ignored, or the test summary does not report all twenty-two
selected cases as passed. It never treats an absent prerequisite or a zero-test
run as a skip/pass.

`scripts/doctor-provisioned-windows-gate.py --self-test` checks the gate's
refusal and libtest-result parsing on any host; it provides no Windows runtime
evidence. `--plan` prints the exact twenty-two-test selector. The live selection runs
`windows_runtime_launches_restricted_child_inside_acl_scratch_and_settles_it`
and `windows_runtime_timeout_terminates_the_confined_job_and_settles_cancellation`,
plus `windows_runtime_timeout_terminates_an_actual_job_descendant`,
`windows_runtime_nonzero_exit_settles_failed_and_cleans_resources` and
`windows_runtime_scratch_refusal_closes_setup_handles`,
`windows_runtime_signed_test_key_capsule_refusals_and_launch_settle`,
`windows_runtime_missing_release_anchor_refuses_before_token_job_or_filesystem`,
`windows_runtime_bad_signature_refuses_before_token_job_or_filesystem`, and
`windows_runtime_signed_linux_architecture_capsule_refuses_before_token_job_or_filesystem`,
and `windows_runtime_protected_scratch_dacl_blocks_inherited_parent_ace`.
The nested `primitive::tests::binding` module adds the seven cases described in
[Signed image binding](#signed-image-binding-continuation-333-partial-native-runtime-evidence).
Its post-binding launch hook attempts a new hard link, writable open, and
writable section from a fresh read handle before process creation; each must
refuse before the authenticated child is allowed to run. Those attempts do not
model a retained writable section and do not establish exact image binding.
The independent `carrier` case supplies the bounded anonymous mapping experiment
above; it is not a request/bundle child transport case.
The child-launch cases traverse `confined_spawn_using`; capsule verification
uses the production release-key path or the explicit test-only key seam. The success case inspects
the child's disabled privilege set and job membership/limits, reads back the
scratch DACL and SID, exercises the one-process job limit, and observes
successful settlement. The other launch cases observe timeout cancellation or
an exact nonzero exit status.
The nonzero-exit case requires exact `Failed(ExitCode(37))` plus cleanup. The
scratch-refusal case repeats an attempt against an absent scratch parent,
requires the filesystem-stage refusal without creating that parent, and checks
that the current process handle count returns to its warmed baseline after each
attempt. The production-configured active-process-limit case verifies
descendant launch refusal. In the separate timeout-descendant test only, the
test first asserts the job limit is one, holds the child at a file handshake,
then raises that test-owned job's limit to two so one exact-filter grandchild
can start. It observes exactly two active job processes before timing out and
requires empty-job cancellation settlement and scratch cleanup. This isolated
test override does not change the primitive's production job limit or weaken
the one-process refusal test. A live assertion-failure guard terminates its
owned job, waits for the job to empty, and removes only the exact marker,
marker temporary, and permit files before scratch teardown.
Each child-launch test captures then removes its exact marker before
settlement, and requires the per-invocation scratch directory and provisioned
parent to be empty afterward. The refusal case requires no scratch entry. If
the 600-second gate timeout fires on Windows, the gate
uses `taskkill /T /F` on Cargo's PID, waits for pipe/process settlement, and
verifies Cargo's PID is absent; inability to verify the direct process exit is
itself a gate failure. Descendants reparented before the post-kill process-list
check are not independently enumerated, so this is not a general descendant
quiescence proof.

The original two-test dispatch selector passed at `c6bf9902`; the historical
five-test selector passed at exact checkout `3d4220b6`. Those runs predate
signed-capsule admission. The historical nine-case selector passed at exact
checkout `c608b8d8`: six live runtime cases (including deterministic test-only
signed-key launch/settlement) and three signed-admission refusal cases. The
historical ten-case source added the hostile inheritable-parent-ACE scratch-DACL
case and passed at `f4d3291f`. The test key is not release trust. The prior
eighteen-case source, including the held-image probes and carrier experiment,
passed in [run 36911767583](https://github.com/wavect/semaprax/actions/runs/36911767583)
at `06c0090d9` (18/18 selected, none ignored). This does not establish exact
launched-image binding against a retained writable section.
The expanded selector passed 22/22 with none ignored at `e15c16202` in
[run 36919771375](https://github.com/wavect/semaprax/actions/runs/36919771375).
Its new retained-section and carrier cases establish refusal and handle/scratch
settlement only; they do not establish image-byte binding or child transport.

## Acceptance criteria status

| Criterion | State |
|---|---|
| Versioned Windows contract, cross-referenced from V1 | met |
| Confinement primitive exists in the owning crate | implemented in `doctor::windows_confinement::primitive`; hosted type-check at exact checkout `7cab8aa8` and historical five selected runtime tests passed at exact checkout `3d4220b6`; see [Nonclaims](#nonclaims) |
| Sealed-capsule consumption | production path calls shared `parse_signed` with the compile-time release-key input and requires native Windows code 3/4; twenty-two selected native cases passed at `e15c16202`; mapped-section race exclusion and request/bundle child transport remain open |
| Hostile-input tests for the host-independent parts | 29 tests across `capsule`, `refusal`, and `settlement` pass on this authoring host (macOS arm64); `cargo test -p semaprax-native-rust-interop-platform-sys --lib doctor::windows_confinement` |
| Runtime tests for the Win32 primitive itself | twenty-two selected cases, including held-image probes and carrier handle settlement, passed in [run 36919771375](https://github.com/wavect/semaprax/actions/runs/36919771375) on `e15c16202` |
| Fail-closed gate authored and run | script self-test and exact twenty-two-test selector passed at `e15c16202` |
| Linux, macOS, or existing job-object evidence never cited as Windows proof | met |
| `docs/COMPLETION-MATRIX.md` WP-05 promoted for Windows | not done; not claimed |

## Nonclaims

This contract does not claim that the twenty-two-test Windows selector is a
complete hostile corpus or production-support gate. The two-test run at
`c6bf9902` and five-test run at `3d4220b6` each bind only their exact checkout
and selected tests.
The hosted Windows compilation recorded above type-checks only exact checkout
`7cab8aa8`; by itself it establishes no execution behavior. The five runtime
tests give narrow observations only for their exact checkout and assertions.
Earlier hand-checking against vendored `windows-sys` was diligence, not
substitute execution evidence. The selector uses a
deterministic test-only signing key and does not establish release trust. The
partial image checks have no atomic exclusion proof for retained writable
sections despite their selected native pass, and Windows request/bundle carrier
transport remains absent. Independent hostile-corpus,
general descendant-tree, and production-support requirements remain open. Do not claim the existing ordinary-probe
job-object confinement in `windows.rs` as evidence of production-grade
sandboxing (it confines process *lifetime*, not filesystem or network access,
and was not designed as a security boundary); claim Linux or macOS evidence
proves anything about Windows; claim that this revision's own host-independent
test pass (`capsule`, `refusal`, `settlement`, run on macOS arm64) is Windows
execution evidence of any kind -- it proves only that logic with no Win32
dependency behaves as designed, nothing about the primitive that actually
touches a job object, a token, or the filesystem; run on a cross-compiled or
emulated target as a substitute for real Windows execution; wire any new path
into the CLI; or promote `docs/COMPLETION-MATRIX.md` WP-05 for Windows.

The five historical live tests ran on Windows at `3d4220b6`, providing bounded
observations of the restricted token, protected DACL, production job limits,
descendant launch refusal, test-owned descendant timeout, normal/nonzero
settlement, cancellation, and repeated filesystem-stage refusal/handle cleanup.
The remaining work includes restricted-token refinement,
closing the retained writable-section mutation race, native validation of the
selected-image checks, Windows request/bundle transport, and a broader hostile/resource corpus across supported Windows runners.
