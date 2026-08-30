# Doctor offline root materialization v1

Status: authored, unrun internal Linux component. No production launcher,
profile provisioning, CLI activation, or WP-05 promotion.

## Ownership and entry boundary

The sys quarantine now owns the single [offline bundle parser](DOCTOR-OFFLINE-BUNDLE-V1.md).
Its unsafe-free parser module retains the sealed input and validated indexes;
the safe platform crate delegates through an opaque wrapper. File-view accessors
return slices with the retained bundle's lifetime, not the temporary view's
lifetime. No caller can construct a bundle from arbitrary file vectors or obtain
mutable input, a descriptor, or a publication operation through that facade.

The separate private `doctor/offline_root` component prepares and materializes
that inventory. It is compiled only for native64 little-endian Linux x86-64 and
AArch64, and has no public entry point. A narrowly scoped dead-code allowance
records that the production execution route is deliberately not connected.
These functions are implementation components, not an admitted doctor host.

The materializer's caller must already own a controlled child with mapped
private user/mount namespace authority, exclusive access to its newly created
descriptors and tree, and a no-unwind setup path. It does **not** authenticate or
establish that context. It neither forks nor changes the current root/cwd,
mounts onto an existing path, closes inherited descriptors, launches a tool,
or drops capabilities. Calling it in an arbitrary embedding process is not a
substitute for the missing bootstrap protocol.

## Effect-free preparation

`Plan::prepare` borrows only an opaque parsed bundle. Preparation, including all
allocation and sorting, must finish before any child boundary. It derives every
slash prefix, sorts and deduplicates those directory paths, and prepares exact
NUL-terminated relative names. Parents precede descendants; file order stays
canonical. Paths are not repaired or resolved against a host filesystem.
Payloads remain borrowed from the retained snapshot.

All vectors and C-string storage reserve fallibly. Before allocation, the plan
charges at most 4,096 files, 126,976 directory-prefix occurrences, and 33,685,504
path bytes including terminators. This conservatively counts repeated prefixes
before deduplication, within the bundle's existing file/depth/path limits.
Invalid page configuration, arithmetic/storage limits and allocation failure
are distinct internal errors. No partially prepared plan is returned.

The explicit page size must be a power of two from 4,096 through 65,536.
The materializer independently checks the actual tmpfs type, block size and
configured block/inode quotas before creating directories or files. Each
nonempty file is rounded up independently to a whole page; empty files consume
no data pages. The overall byte quota is never zero. The inode quota is exactly
one root, unique directories and all files. Decimal mount values are prepared
without formatting or allocation inside the child.

This bounds filesystem data pages and inodes, not total resident memory,
allocator overhead, kernel metadata or syscall latency. The input, plan and
materialized data coexist. Kernel/LSM and VM/swap activity remain trusted.
Tmpfs treats zero quotas as unlimited and may charge extended attributes against
inode space; allocation can fail rather than silently increasing limits. See
the [kernel tmpfs contract](https://docs.kernel.org/filesystems/tmpfs.html).
Automatic LSM labels (for example SELinux security attributes) can therefore
make a nominally fitting inventory fail closed on that host; this component
does not disable labeling or promise compatibility with every LSM policy.

## Detached materialization

The syscall path uses only the prepared plan and stack storage:

1. Create a fresh `tmpfs` context with `fsopen`; configure explicit byte/inode
   quotas and root mode `0700` through `fsconfig`.
2. Create the filesystem and obtain a detached `fsmount` root with close-on-exec,
   nosuid and nodev attributes. No host mountpoint is opened or modified and no
   source directory is copied or bind-mounted.
3. Authenticate filesystem type, block size and quotas. Create only the derived
   directories, setting exact `0700` permissions independently of umask.
4. Exclusively create regular files beneath that root using `O_EXCL`,
   `O_NOFOLLOW` and `O_CLOEXEC`. Write exact borrowed content in chunks of at most
   8,192 bytes. Errors, interruptions, zero and short writes reject without retry.
5. Set file mode to `0400` for data or `0500` for executable intent. Close every
   writable descriptor before applying read-only/nosuid/nodev attributes with
   `mount_setattr(AT_EMPTY_PATH)`. Verify the resulting filesystem flags and
   quotas before returning the private root owner.

The fresh tree has no symlinks or external mutators. This private ownership is
required for the root-relative operations; they are not a general hostile-path
traversal API. Read-only mount flags do not constrain a future privileged tool
that can remount it. Capability removal, syscall confinement and retained-handle
exclusion remain separate mandatory launch gates.

Ordinary failures close only newly created context/root/file descriptors. There
is no host publication or recursive path cleanup. Each close is attempted once;
uncertain close ownership terminates the controlled child with `_exit(126)` and
cannot return a root or continue setup. The private root requires explicit
consuming closure rather than Rust `Drop`. Its future owner must close it or
terminate without unwinding; the future supervisor still owes exact process
settlement before publishing a report.

## Unresolved launch boundary

The child of an ordinary fork inherits host descriptors. Closing a copied
descriptor can invoke a filesystem flush callback even while the parent retains
its original reference; see the [kernel close path](https://raw.githubusercontent.com/torvalds/linux/master/fs/open.c).
Thus `close_range`, close-on-exec and failure `_exit` are not automatically
harmless no-network bootstrap operations. This component does not reuse the
retained installed-tool launcher.

Kernel filesystem-module autoload and usermode-helper policy are also outside
this component. An unavailable filesystem can trigger that policy during
acquisition; a future offline bootstrap must review its provisioned kernel and
helper boundary rather than treating every rejected mount as effect-free.

Production integration still requires independently reviewed clean-descriptor
provisioning; namespace creation and UID/GID-map authority; exact process
ownership, deadlines and namespace-descendant settlement; root/cwd entry and
capability removal; native-only execution, loader/configuration closure and
syscall/IPC/network restrictions. Windows and macOS need their own physical
routes. The real CLI continues to report unavailable profiles without fallback.

## Authored evidence

Default Linux preparation fixtures enter through real sealed memory input and
the production parser. They inspect exact directory order, deduplication,
payload bytes/borrow addresses, per-file page rounding, empty-file accounting,
supported/invalid page sizes and 4,096 tiny files. Pure helpers cover exact and
plus-one path accounting, overflow, decimal framing and reservation failure.
These are structural ELF byte samples, not runnable tool distributions.

Physical cases inspect actual reopened bytes and modes, exact quotas, absent
unlisted paths, read-only write rejection and retained-file lifetime after root
closure. Calibrated setup/write failures return no root; metadata mutations feed
the production predicates rather than returning a test-specific error. A
controlled self-reexecution fixture exercises uncertain-close termination.
Injected errors are branch evidence, not delivered kernel-fault evidence.

Physical materializer fixtures require an externally provisioned private mapped
user/mount namespace and run serially. Capability/syscall absence fails rather
than skipping or falling back. The environment acknowledgement below records a
provisioner precondition, not namespace authentication. Only in that context:

```sh
SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT=private-user-mount-v1 cargo test --locked \
  -p semaprax-native-rust-interop-platform-sys doctor::offline_root::linux::tests \
  -- --ignored --test-threads=1
```

All new fixtures and the unchanged input/parser/CLI/lower-probe gates remain
unrun. No completion status or supported-platform claim changes in this batch.
