# Doctor sealed input v1

Status: authored, unrun private input-boundary implementation. This primitive
alone supplies no profile format, CLI activation, executable isolation, or WP-05 promotion.

Audience: CLI/platform contributors and reviewers.

## Purpose and authority

The future offline doctor backend needs to obtain bytes without discovering or
reading an arbitrary host filesystem first. The unpublished safe platform facade
exposes `DoctorOfflineInput::acquire(&File, max_bytes)`. Its caller provisions
and retains the input file; the acquisition never interprets a pathname, reads
an environment variable, or takes ownership of that file.

The returned opaque carrier owns only an immutable byte vector. Its contents are
untrusted and are not a parsed profile, authenticated selector, trusted tool,
loader closure, filesystem root, or permission to execute. There is no descriptor
accessor, mutable byte accessor, ambient lookup, registry, format serializer, or
publication operation. This is an input primitive, not an admitted `DoctorHost`.
The [real doctor CLI](DOCTOR-PROBE-V1.md) still reports unavailable profiles.

## Anonymous carrier creation

The separate `create_doctor_offline_input(bytes, max_bytes)` function accepts
explicit borrowed bytes and returns `(File, DoctorOfflineInput)`. It fills one
fresh anonymous memory file, seals it, then uses the existing `acquire` operation
and exact byte comparison before transferring either result. The snapshot is
not constructed directly from caller bytes. Returning both objects avoids
requiring the caller to reacquire the snapshot just to parse a bundle or derive
its request. The caller owns the returned file and its subsequent lifetime;
dropping that file does not invalidate the snapshot.

Creation validates a zero limit (`Invalid`), above-ceiling limit (`Limit`), empty
input (`Invalid`) and input above the selected limit (`Limit`), in that order,
before any OS operation. Only the same native64 Linux configurations as
acquisition are admitted; other hosts then return `Unsupported`. It creates
with `MFD_CLOEXEC | MFD_ALLOW_SEALING | MFD_NOEXEC_SEAL`. There is no retry with
weaker flags or filesystem fallback. `EINVAL` or `ENOSYS` from creation returns
`Unsupported`; other operational failures return `Io`.

The explicit non-executable flag creates a file without execute permission and
sets `F_SEAL_EXEC`, preventing later addition of execute permission. It does not
promise to prevent every executable mapping or use of copied content. See the
[Linux non-executable memory-file contract](https://cdn.kernel.org/doc/html/latest/userspace-api/mfd_noexec.html).
No sysctl or host policy is modified; kernels or policies that cannot supply the
mandatory property reject. This stronger creation prerequisite does not change
the existing borrowed acquisition's accepted files or kernel requirements.

Writes are positional, at most 8,192 bytes each, with exact counts required.
Short, zero, interrupted or failed writes reject without retry or partial
publication. The shared file offset stays zero. Before returning, creation
requires all four immutable seals from the acquisition contract, the executable
seal, a regular file with no execute bits, the exact size, and close-on-exec.
Disagreed properties return `Invalid`; failed property queries return `Io`.
Only after these checks does ordinary sealed acquisition authenticate storage
and copy the snapshot; its existing errors remain unchanged. An unequal snapshot
rejects without returning either object.

Only the newly created descriptor is owned during failure cleanup; no supplied
file, pathname or arbitrary inherited descriptor is touched. Ownership is
consumed before exactly one checked close. A negative close result terminates
the process; it is never retried or followed by another operation on that
descriptor number. This new factory deliberately treats uncertain closure as
fail-stop rather than inferring completion from an error. It does not change
the existing acquisition's no-close contract or the caller's ownership after a
successful return. Normal cleanup cannot replace the primary selected error.

The byte ceiling bounds the carrier and snapshot individually, not total
resident memory: caller bytes, memory-file storage and the acquired snapshot can
coexist. The bounds constrain application allocation and syscall counts, not
hard real-time latency, kernel/LSM/VM behavior or aggregate host resource use.
Creation does not authenticate content provenance, parse a profile, discover
dependencies, start a worker, configure namespaces, or activate the ordinary
CLI. Its returned file is transport storage, not execution authority.

## Executable image creation

The separate `create_doctor_offline_executable(bytes, max_bytes)` factory returns
the same `(File, DoctorOfflineInput)` shape for worker/collector image storage.
It does not change the non-executable factory or turn borrowed input acquisition
into an executable validator. The safe facade delegates to the existing sys
quarantine; both factories share one private creation, bounded-write, snapshot
comparison and checked failure-cleanup implementation.

Common zero-limit, above-ceiling, empty-input and input-length checks retain
their order. Unsupported platforms then return `Unsupported`. On supported
native64 Linux hosts, the existing minimum ELF validator checks the explicit
bytes against the current architecture before creating a descriptor. Scripts,
malformed framing and foreign images return `Invalid` without creation or
cleanup effects. This is the same structural validator used by the bundle and
launcher, not a second parser. A structurally admitted interpreter name is not
looked up and does not prove that the image can load.

The executable route requires `MFD_CLOEXEC | MFD_ALLOW_SEALING | MFD_EXEC`.
Immediately after acquiring ownership of the new descriptor it sets mode
`0500`, before writing image bytes. The already-open read/write description
is used for bounded positional writes; no path reopen or temporary disk file
is needed. It adds the four immutable seals and `F_SEAL_EXEC` together, then
requires their presence, a regular file, exact permission/special bits `0500`,
exact length and close-on-exec. Additional kernel seals are permitted. Existing
sealed acquisition and exact byte comparison precede transfer of either result.
The returned shared file offset is zero, and dropping the file leaves the
snapshot valid. Creation/query/write/cleanup errors retain the ordinary input
factory's error and one-shot ownership rules, including fail-stop uncertain
closure. There is no retry with weaker flags, executable-mode downgrade, sysctl
change or filesystem fallback when the host prohibits executable memory files.

Linux can reject explicit executable memfds under its namespace policy.
`F_SEAL_EXEC` prevents changing execute bits, not every permission or metadata
field; executable sealing may add further write-related seals. See the
[kernel API](https://docs.kernel.org/userspace-api/mfd_noexec.html) and
[seal implementation](https://raw.githubusercontent.com/torvalds/linux/master/mm/memfd.c).
Mode `0500` is checked at handoff, not promised immutable against later caller
metadata operations. No factory result attests a particular worker/collector
role, provenance, library/configuration closure, credential behavior or binfmt
policy. It does not execute the bytes or confine other code in the calling
process. The provisioner still owns those facts and aggregate resources;
the [launcher](DOCTOR-OFFLINE-LAUNCHER-V1.md) independently validates its actual
inherited descriptors before use. Ordinary CLI admission remains unavailable.

## Admission and ordering

1. Reject a zero caller limit with `Invalid`; reject a caller limit above the
   immutable 536,870,912-byte (512 MiB) ceiling with `Limit`, before any syscall.
   Callers may lower this ceiling, not widen it. The ceiling is a resource bound,
   not a promise that every Clang/Node/Rust distribution fits into one carrier.
2. Only native 64-bit little-endian Linux x86-64/AArch64 is admitted. Other hosts
   return `Unsupported` without querying the supplied file.
3. Borrow the still-owned file descriptor for the entire acquisition. Perform
   `F_GET_SEALS` **before metadata or content access**. Require all of
   `F_SEAL_WRITE`, `F_SEAL_GROW`, `F_SEAL_SHRINK`, and `F_SEAL_SEAL`. Other seal
   bits may coexist; `F_SEAL_FUTURE_WRITE` does not replace `F_SEAL_WRITE`.
4. Only after that query authenticates immutable memory-file storage, require
   `fstatfs` to report `TMPFS_MAGIC` (excluding hugetlb), then `fstat` to report a
   regular file with a nonnegative, nonempty length within the caller limit.
5. Reserve the bounded output storage fallibly. Copy with positional `pread`
   calls of exactly 8,192 bytes, except for the final remaining bytes. Any
   incomplete, zero, or failed read, including interruption, returns `Io` with
   no partial carrier and no retry. Successful acquisition needs exactly
   `ceil(length / 8192)` reads and never changes the shared file offset.
6. Return only the complete owned bytes. The caller's file is neither duplicated
   nor closed on success or failure. Dropping it afterward does not invalidate
   the returned bytes.

The seal query returning `EINVAL`, a missing required seal, or invalid
storage/type/empty length rejects as `Invalid`. Other seal-query errors,
including `EINTR` and `EBADF`, reject as `Io`. An admitted storage length above
the requested bound rejects as `Limit`; metadata/read/allocation failures
reject as `Io`. Unsupported hosts do
not try another path. No input bytes are allocated or read before the storage
and size checks. This bounds application allocations and syscall counts, not
hard real-time latency of the kernel or allocator.

## Why the ordering matters

Linux routes `F_GET_SEALS` directly to memory-file seal inspection, recognizing
shmem/hugetlb storage rather than calling an arbitrary filesystem's metadata or
read handlers. The subsequent tmpfs check excludes hugetlb storage. See the
[fcntl dispatch](https://raw.githubusercontent.com/torvalds/linux/master/fs/fcntl.c)
and [memory-file seal implementation](https://raw.githubusercontent.com/torvalds/linux/master/mm/memfd.c).

Duplicating an arbitrary input first is **not** harmless: closing the rejected
duplicate can call a filesystem flush handler even while the original is open.
The implementation therefore never duplicates or closes the borrowed input.
See the [kernel close path](https://raw.githubusercontent.com/torvalds/linux/master/fs/open.c).
The safe Rust borrow keeps the file owned throughout the call; foreign unsafe
code closing or replacing its raw descriptor violates that I/O-safety contract.

This reasoning excludes arbitrary supplied-filesystem metadata/read/flush
dispatch during acquisition. It does not confine provisioning before the call,
the kernel, LSM hooks, auditing, swap/paging, or other process activity. It does
not establish any property of executable bytes or of a future launched process.
In particular, seals prove immutable contents, not trusted provenance.

## Required evidence

The focused sys fixtures must use real sealed memory files and cover exact
binary bytes, unchanged caller position/flags/seals and continued handle use,
all missing mandatory seal combinations, future-write-only rejection, live
writable-map rejection, ordinary files/directories/pipes/sockets and O_PATH,
exact/plus-one limits, sparse oversize rejection before output allocation,
and failed/short/interrupted read non-publication. Per-invocation test-only
operation observations must be calibrated by successful physical acquisition
and prove that seal rejection never reaches metadata, allocation or reads.
Metadata and read-failure injections exercise the shared rejection paths; they
are simulations, not evidence of physical kernel faults or signal delivery.
The no-duplicate/no-close property additionally requires source review: leaving
the original descriptor usable alone would not detect a duplicate-close flush.

The facade retains `forbid(unsafe_code)` and only delegates to the existing OS
quarantine. Earlier CLI/profile and version-probe lifecycle fixtures are
unchanged and remain required. Unsupported-host behavior must be exercised
separately from Linux success. All new fixtures are authored and unrun.

Creator fixtures separately cover exact binary content and chunk boundaries,
zero initial offset, close-on-exec, immutable and executable seals, rejected
write/resize/shared-writable-map/execute-permission changes, and retained
snapshot lifetime. Private fault controls cover creation, writes, seals,
property queries and acquisition failures without exporting runtime injection.
They must prove one-shot cleanup and no later write or transfer after failure;
injected syscall outcomes do not establish physical kernel faults. A dedicated
subprocess exercises fail-stop closure without targeting foreign descriptors.
Native success fixtures require actual non-executable sealing support and must
not silently skip or downgrade when that prerequisite is missing.

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::offline_input::create
```

Executable-factory regressions retain the non-executable cases and add literal
native ELF framing, pre-effect malformed/foreign/script and limit rejection,
exact chunk boundaries, mode/offset/seal/content checks, blocked content/size/
execute-bit mutations, failure-prefix and one-shot cleanup observations. Private
fault controls exercise the shared native flow; they are not physical syscall
fault evidence. A subprocess-only forced-close case must terminate without a
later factory return. Unsupported-host cases do not substitute for Linux success.

Launcher admission tests pass the actual returned executable files, not rebuilt
copies, into both image roles. The ignored production-launcher fixtures use
factory-created healthy images alongside independently constructed hostile
images; malformed inputs must still reach launcher rejection tests rather than
being filtered out by the factory first. No fixture has been executed here.
Storage and structural acceptance do not prove executable startup; the real
launcher/worker/collector runs need the complete provisioned context described
in the launcher contract.

The collector's ignored `tests/support/created_handoff.rs` additionally passes
the actual production-created files to the existing trusted launch fixture,
without serializing or resealing their contents. Independent literal bundle
and request checks precede native/all worker-to-report observations and an
unrepaired request-digest rejection. Existing literal-sealing hostile fixtures
remain independent. This gate requires the full
[provisioned collector context](DOCTOR-OFFLINE-COLLECTOR-V1.md#evidence-and-non-claims),
not merely permission to create a memory file. All creator and handoff cases
remain authored and unrun.

The separate [offline bundle parser](DOCTOR-OFFLINE-BUNDLE-V1.md) now consumes
this input through a closed, bounded inventory; it still grants no execution
authority. Before any production profile can be admitted, separately implement
and review provisioning/bootstrap, complete tool and
loader/configuration input closure, OS filesystem/IPC/network restrictions,
descendant settlement and the explicit selector-to-admission binding. Physical
platform and real selected-tool compatibility evidence remains mandatory.
