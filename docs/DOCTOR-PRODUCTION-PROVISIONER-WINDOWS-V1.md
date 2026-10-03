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
The subsequent 24-case native receipt establishes initial-open exclusion of
retained writable sections; the 26-case receipt adds authenticated child
request/bundle transport. The continuous sharing argument, two new concurrent
image cases, and three resource-limit cases are specified below. The authoring
host is macOS arm64; native Windows evidence remains attached to its exact
hosted revision.
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
  native Windows witnesses below; new concurrent cases await execution. See its
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
  reject*, not silently accommodate). When adding a process would exceed the
  bound, Windows terminates that new process and the association fails; the
  existing leader remains subject to the job.
- `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`, so a crashing tool cannot
  leave a debugger-attachable faulted process alive inside the job.
- `JOB_OBJECT_LIMIT_PROCESS_TIME` with a two-second
  `PerProcessUserTimeLimit`. The CPU-burn fixture must make the leader exit
  nonzero rather than relying on the caller's wall-clock deadline. Settlement
  preserves that raw `ExitCode`: aggregate job CPU accounting does not prove
  that the periodic Windows limit check caused a particular exit.
- `JOB_OBJECT_LIMIT_PROCESS_MEMORY` with a 256 MiB committed-memory cap. The
  hostile child allocates and touches 8 MiB chunks past that ceiling and must
  report an allocation refusal before settlement; reserving address space is
  not the hostile control.
- Two anonymous output pipes with 4 KiB pending buffers. The parent-only
  readers account stdout and stderr together and terminate the owned job once
  their total exceeds 64 KiB.
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

[DOCTOR-SEALED-INPUT-V1](DOCTOR-SEALED-INPUT-V1.md)'s `F_SEAL_*` memfd has no
Windows equivalent. The standalone primitive instead copies each authenticated
request and bundle into a separate unnamed paging-file mapping, releases its
only writable view and handle, and retains a `SECTION_MAP_READ` duplicate. The
kernel object has no lookup name and its surviving handle cannot create a
writable view. This is a bounded carrier mechanism, not an equivalence claim
for Linux seals or a general Windows immutability primitive.

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
The continuous no-write-sharing barrier and its native retained-section
controls are specified below.
Before token, job, scratch-root, or process effects, the standalone spawn also
copies the capsule's exact request and bundle slots into those two carriers.
It passes only the request-then-bundle pair with standard I/O through the
startup handle list, supplies their inherited handle values with the signed
selector and chosen image role in its closed child environment, and retains the
parent copies through settlement. This is an internal primitive handoff with the native receipt below and no
ordinary CLI route; it is not production support.

### Authenticated request/bundle handoff

`windows_confinement::carrier` accepts explicit bytes only when they match the
signed request or bundle `Artifact`'s exact nonzero bounded length and SHA-256
digest. The pair has one owner, so a caller cannot combine a request carrier
from one capsule with a bundle carrier from another. It copies each input to an
unnamed paging-file mapping, unmaps its sole writable view, duplicates only
`SECTION_MAP_READ` into an inheritable handle, drops the writable handle, and
rehashes from the retained read-only handle before returning.

Each section is created with the protected, deny-only DACL
`D:P(D;;GA;;;OW)`: no allow ACE grants a new handle access, and the OWNER RIGHTS
ACE suppresses the owner's implicit permission to rewrite the DACL. A merely
read-only duplicate with the creator's default DACL is insufficient because
[`DuplicateHandle`](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle)
can request greater access. A completely empty DACL also leaves the owner's
implicit `WRITE_DAC` permission; Microsoft's
[OWNER RIGHTS definition](https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/understand-security-identifiers)
specifies why an explicit owner-rights ACE is necessary. The creator's initial
handle fills the new section before it is reduced to read access; no later
permission transition is needed.

The new selected native cases reject same-length request and bundle substitutions
before process creation and verify a real restricted child can map exactly the
two inherited signed payloads read-only. The child checks its signed selector
and image role binding and cannot map either inherited carrier writable. The
same child also requires `ERROR_ACCESS_DENIED` when duplicating either section
with `FILE_MAP_WRITE`, `WRITE_DAC`, or `WRITE_OWNER`, covering direct write
escalation and owner-mediated permission changes while preserving its exact-byte
read control. These assertions extend the existing handoff case, keeping all
26 cases in the native `2c9695f5d` receipt below. The further two image-sharing
cases require a new native receipt.

### Signed image binding (#333): continuous sharing exclusion

`primitive/image.rs` bridges the pathname required by
[`CreateProcessAsUserW`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessasuserw)
to the authenticated file using one continuously held NTFS sharing barrier.
The security argument is based on
[`CreateFileW`'s `FILE_SHARE_WRITE` contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew),
which rejects an open without write sharing when the existing file has a
writable mapping. This includes the retained section capability before any
view is mapped. The native initial-open controls below test that exact
interpretation, independently of the subsequent oplock.

The production ordering is:

1. Open the selected image for data read with only `FILE_SHARE_READ`. Reject
   nonregular, reparse and multiply linked files and require NTFS. Existing
   write handles and writable mappings conflict at this first open. This is
   data-read access, not the metadata-only access whose sharing behavior is
   insufficient.
2. Retain the original open continuously. New data-write/append/delete opens
   conflict with it. Creating a writable section requires a writable file
   handle, as specified by
   [CreateFileMappingW](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-createfilemappingw).
   There is consequently no later transition from accepted image to writable
   section capability. Same-user DACL modification cannot relax the held
   handle's sharing restrictions. Unlike pagefile sections,
   [file-handle duplication](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle)
   cannot upgrade a read-only file handle to read/write access. The trusted
   kernel/administrator boundary remains explicit.
3. Acquire the normalized volume-GUID pathname, pin each ancestor without
   following reparse points or permitting write/delete sharing, and reopen
   the leaf through those pinned components. Compare its volume/file identity
   with the original. No signed byte read precedes this identity check.
   Keep every ancestor and both data-read file handles through child lifetime.
4. Hash the exact signed bounded raw-file length through the synchronous
   reader. Reject differing length, digest or role with
   `Capsule(ArtifactBinding)` before token, job, scratch or process creation.
   There is no close/reopen interval between authentication and loading.
5. Create the child suspended using that pinned pathname and verify its native
   image name before resume. A redirected image, including IFEO debugger
   redirection, or a query failure selects refusal and terminates/observes the
   suspended leader. The name check identifies the pinned file; the sharing
   barrier establishes the stability of its authenticated bytes.
6. Retain the complete image guard through settlement/drop, so deferred loader
   reads remain covered. Release it only after the leader/job settlement path.

The read oplock, its event checks after hashing/before creation/before resume,
and its cancellation/drain discipline remain additional refusal checks. They
are not the exclusion mechanism: Microsoft's
[section-synchronization rules](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/fs-filter-acquire-for-section-synchronization2)
allow advisory writable-section oplock breaks without acknowledgment. The
prior review incorrectly treated that limitation as the only relevant
barrier after separately identifying the no-write-sharing contract. Its
outstanding empirical question was whether a retained no-view section is
rejected at the initial open. The native checkpoint receipt resolves that
question for the admitted NTFS host profile.

This design does not authenticate imported DLLs or defend against kernel,
volume-administrator or process-handle injection authority. It does not change
the Linux launcher, create a broker identity, add an ordinary Windows CLI
route, provision a production signing key or promote WP-05 Windows support.
The separately authenticated request/bundle carriers remain part of the same
standalone primitive's explicit five-handle child inventory.

### Native binding evidence and selected hostile cases

The initial-open observation immediately follows the first successful
`FILE_SHARE_READ` open, before metadata checks or oplock acquisition. Both a
retained writable section with no view and a surviving writable view whose
original handles have closed require `Capsule(ArtifactBinding)` with no
observation reached. A retained no-view section must still map writable after
refusal and observably change/restore a same-length byte of the valid PE
fixture. The clean fixture must subsequently acquire the complete image guard.
Unavailable prerequisites never count as passing skips.

Those tests, the exact five-checkpoint successful child launch, and the earlier
substitution/settlement corpus passed 24/24 at `d0b42ce3d` in
[run 36988402356](https://github.com/wavect/semaprax/actions/runs/36988402356).
The request/bundle child handoff and protected carrier duplication corpus then
passed 26/26 at `2c9695f5d` in
[run 36991582065](https://github.com/wavect/semaprax/actions/runs/36991582065).
These are native Windows Server 2025 receipts using deterministic test keys.
They substantiate the selected primitives and do not establish release trust.

Two further selected native cases in this revision require their own receipt:

- `windows_runtime_image_sharing_race_preserves_authenticated_bytes` runs a
  retained no-view section winner control, a clean image-guard winner control,
  and eight concurrent kernel-open races. Exactly one conflicting open may
  win. When the section wins, admission refuses and the section must
  observably mutate then restore the signed file. When the guard wins, the
  hostile writer must receive `ERROR_SHARING_VIOLATION` while the guard stays
  held. Every iteration requires successful authenticated reacquisition,
  empty scratch and the warmed parent handle count.
- `windows_runtime_image_sharing_excludes_writers_at_every_launch_boundary`
  synchronously joins another thread's hostile writer attempt at the initial
  open, guard acquisition, digest validation, pre-process and suspended-leader
  checkpoints. At each checkpoint a fresh read-only file handle must also
  refuse duplication with read/write access. Every writer open must return
  `ERROR_SHARING_VIOLATION`; the child
  must exhibit the authenticated fixture behavior and settle with exact exit
  37 and an empty scratch parent. No checkpoint may be omitted.

The concurrent cases exercise the implementation ordering; repeated race
success alone is not the proof. The proof combines the documented sharing and
mapping-access contracts, continuous handle ownership in the implementation,
and native no-view/surviving-view initial-open observations. The existing
length/digest/role mismatch, leaf deletion/write/rename/hardlink, ancestor
rename, fresh-read-handle mapping, image-handle settlement and undeclared
inherited-handle cases remain selected.

### Retired experiments and remaining issue boundary

The OWNER RIGHTS fresh-image experiment at `f9ddd4fdd` failed its
`SetSecurityInfo` transition with `ERROR_ACCESS_DENIED` in
[run 36983893062](https://github.com/wavect/semaprax/actions/runs/36983893062).
It was retired at `16e14488ed`; no accepted behavior depends on it. A
metadata-only bridge also remains rejected by the
[MS-FSA sharing algorithm](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/8c0e3f4f-0729-49f4-a14d-7f7add593819).
No ACL transition, copy/close/reopen scheme, image-section experiment or
byte-range lock is required by the continuous data-read sharing design.

The resource corpus adds a CPU burn that must settle as
`Failed(ExitCode(_))`, an allocation-and-touch fixture that must receive a
committed-memory refusal, and an alternating stdout/stderr flood with each
stream below 64 KiB but their total above it. The flood must settle as
`Failed(OutputLimit)` after terminating the owned job. Each case requires
job/handle/scratch settlement; inspecting job flags alone does not meet that
obligation. Preserve the production one-process limit and separately
authorized test-only descendant fixture. Ordinary provisioner integration and
production support remain separate from #333's signed binding and
hostile-runtime acceptance.

For this batch, run the Windows library/test type-check, both provisioned gate
self-tests and the complete native Windows selector on the changed revision.
The user has explicitly delegated the full quality profile to hosted CI; it
is not a local issue-closure prerequisite. This source revision adds five
selected tests and does not claim they have executed yet.

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
test is filtered or ignored, or the test summary does not report all thirty-one
selected cases as passed. It never treats an absent prerequisite or a zero-test
run as a skip/pass. The historical receipt below covers the earlier twenty-two
case selector only.

`scripts/doctor-provisioned-windows-gate.py --self-test` checks the gate's
refusal and libtest-result parsing on any host; it provides no Windows runtime
evidence. `--plan` prints the exact thirty-one-test selector. The live selection runs
`windows_runtime_launches_restricted_child_inside_acl_scratch_and_settles_it`
and `windows_runtime_timeout_terminates_the_confined_job_and_settles_cancellation`,
the CPU-time and committed-memory fixtures, and
`windows_runtime_combined_output_limit_terminates_and_settles_the_confined_job`,
plus `windows_runtime_timeout_terminates_an_actual_job_descendant`,
`windows_runtime_nonzero_exit_settles_failed_and_cleans_resources` and
`windows_runtime_scratch_refusal_closes_setup_handles`,
`windows_runtime_signed_test_key_capsule_refusals_and_launch_settle`,
`windows_runtime_missing_release_anchor_refuses_before_token_job_or_filesystem`,
`windows_runtime_bad_signature_refuses_before_token_job_or_filesystem`, and
`windows_runtime_signed_linux_architecture_capsule_refuses_before_token_job_or_filesystem`,
and `windows_runtime_protected_scratch_dacl_blocks_inherited_parent_ace`.
The nested `primitive::tests::binding` module adds the image-binding cases described in
[Signed image binding](#signed-image-binding-333-continuous-sharing-exclusion).
Its post-binding launch hook attempts a new hard link, writable open, and
writable section from a fresh read handle before process creation; each must
refuse before the authenticated child is allowed to run. Those attempts do not
model a retained writable section and do not establish exact image binding.
The request/bundle cases reject substitution before process effects, then make a
real child map only its two declared authenticated carriers read-only while
checking the signed selector and selected role. The existing independent
`carrier` cases continue to cover mapping refusal and handle settlement.
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
The earlier expanded selector passed 22/22 with none ignored at `e15c16202` in
[run 36919771375](https://github.com/wavect/semaprax/actions/runs/36919771375).
Its new retained-section and carrier cases establish refusal and handle/scratch
settlement only; they do not establish image-byte binding or child transport.
The later checkpoint and request/bundle handoff cases passed in the exact
24/26-case native receipts above. The two concurrent sharing cases and three
resource-limit cases extend the selector to thirty-one and require a new native
Windows receipt.

## Acceptance criteria status

| Criterion | State |
|---|---|
| Versioned Windows contract, cross-referenced from V1 | met |
| Confinement primitive exists in the owning crate | implemented in `doctor::windows_confinement::primitive`; hosted type-check at exact checkout `7cab8aa8` and historical five selected runtime tests passed at exact checkout `3d4220b6`; see [Nonclaims](#nonclaims) |
| Sealed-capsule consumption | standalone primitive calls shared `parse_signed` with the compile-time release-key input and requires native Windows code 3/4; it now carries exact signed request/bundle slots to a fixed child inventory, with a 26-case native receipt at `2c9695f5d`; two image-sharing and three resource-limit cases need a native receipt; ordinary Windows CLI transport remains open |
| Hostile-input tests for the host-independent parts | 29 tests across `capsule`, `refusal`, and `settlement` pass on this authoring host (macOS arm64); `cargo test -p semaprax-native-rust-interop-platform-sys --lib doctor::windows_confinement` |
| Runtime tests for the Win32 primitive itself | 26 selected cases passed in [run 36991582065](https://github.com/wavect/semaprax/actions/runs/36991582065) at `2c9695f5d`; two additional concurrent sharing cases and three resource-limit cases await native execution |
| Fail-closed gate authored and run | exact 26-case selector passed at `2c9695f5d`; the 31-case selector requires a new receipt |
| Linux, macOS, or existing job-object evidence never cited as Windows proof | met |
| `docs/COMPLETION-MATRIX.md` WP-05 promoted for Windows | not done; not claimed |

## Nonclaims

This contract does not claim that the thirty-one-test Windows selector is a
complete hostile corpus or production-support gate. The two-test run at
`c6bf9902` and five-test run at `3d4220b6` each bind only their exact checkout
and selected tests.
The hosted Windows compilation recorded above type-checks only exact checkout
`7cab8aa8`; by itself it establishes no execution behavior. The five runtime
tests give narrow observations only for their exact checkout and assertions.
Earlier hand-checking against vendored `windows-sys` was diligence, not
substitute execution evidence. The selector uses a
deterministic test-only signing key and does not establish release trust. The
continuous image-sharing argument and exact prior native receipts are recorded
above; the two new concurrent sharing cases still require execution. Ordinary
Windows transport remains absent. Independent hostile-corpus,
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
native validation of the new concurrent image-sharing and resource-limit cases,
ordinary Windows request/bundle transport, and a broader hostile corpus across
supported Windows runners.
