# Doctor sealed input v1

Status: authored, unrun private input-boundary implementation. This primitive
alone supplies no profile format, CLI activation, executable isolation, or WP-05 promotion.

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

The separate [offline bundle parser](DOCTOR-OFFLINE-BUNDLE-V1.md) now consumes
this input through a closed, bounded inventory; it still grants no execution
authority. Before any production profile can be admitted, separately implement
and review provisioning/bootstrap, complete tool and
loader/configuration input closure, OS filesystem/IPC/network restrictions,
descendant settlement and the explicit selector-to-admission binding. Physical
platform and real selected-tool compatibility evidence remains mandatory.
