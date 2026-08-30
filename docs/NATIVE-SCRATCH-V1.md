# Native compiler scratch v1

Status: authored correction; executable regression gates remain unrun.

Audience: compiler contributors, CLI maintainers, and security reviewers.

## Scope

The ordinary native compilation helper writes an emitted C translation unit
for Clang. The legacy single-source `run` command additionally needs a temporary
executable. Neither operation may adopt, truncate, or delete a pre-existing
temporary file merely because its name contains this process's ID.

This correction changes temporary filesystem handling, not language semantics,
emitted C, command grammar, compiler flags, requested build-output behavior,
Project/npm/Rust package publication, or any descriptor/carrier schema. The
private shared implementation is `src/native_scratch.rs`, compiled separately
into the compiler and shared CLI driver without a new public API or dependency.

## Exclusive creation and ownership observations

One invocation resolves and retains an existing absolute temporary parent.
It creates a new direct child directory exclusively, with mode `0700` on Unix.
A bounded sequence of candidate names may encounter collisions; every occupied
candidate remains untouched. Names are allocation hints, not proof of ownership,
unpredictable secrets, or permission to remove another invocation's files.

Each scratch directory has one expected ordinary leaf. The compiler source is
written create-new and its identity is retained from that actual open file.
Executable scratch starts empty: a retained output handle must not obstruct
the linker on Windows. Only successful compilation permits adoption of the
expected plain executable, after checking the complete directory inventory.
The private executable basename includes the platform's executable suffix.

Checks reject a changed parent or directory identity, symlink/reparse leaves,
multiply linked files, unexpected inventory, and a file that no longer matches
its retained identity.
Retained handles overlap identity observations. These checks operate within a
trusted, quiescent temporary namespace; pathname observations are not atomic
protection against concurrent same-principal substitution. Windows identity
comparisons retain the existing `same-file` backend's filesystem limitations.
This is not a new protected Windows DACL or arbitrary-filesystem guarantee.

## Ordering, failure, and cleanup

The source-run route completes ordinary checking and C emission before allocating
executable scratch. A source or emission diagnostic therefore cannot delete an
old executable pathname or create a new executable directory.

Compilation and execution preserve their existing primary errors. Scratch
creation, writing, or sealing failures use the existing `SPX-I101` I/O category;
compiler start failure remains `SPX-B101`, compiler failure remains `SPX-B102`,
and child exit status handling is unchanged.

There is no deleting destructor. Failed compilation, failed child start/wait,
and non-successful child exit retain scratch for inspection. A successful direct
child exit permits an explicit cleanup attempt under the trusted-tool contract;
it is not proof that all descendants are quiescent.

Before deleting anything, cleanup validates the complete fixed inventory and
retained identities. It removes only the expected file, then the empty directory,
without recursive deletion. The file handle is released before the empty-directory
step so a Windows delete-pending file does not keep the directory occupied.
Any failed check or removal stops cleanup; an unexpected sidecar is never added
to the deletion inventory. Cleanup remains best effort after successful work
and does not replace the primary result. Residue is possible even on success.

The helper neither authenticates arbitrary native code nor restricts its process,
environment, filesystem, or network authority. Clang and the executed program
retain the ordinary native path's existing host authority. The separate held-tool
SDK builder and doctor lifecycle contracts are unchanged. No sandbox, secure
erasure, crash recovery, checked handle-close settlement, or hostile descendant
isolation is claimed.

## Required evidence

The private helper regressions must cover occupied file/directory candidates,
symlink and dangling-link candidates, exact successful cleanup, invalid leaf
names, changed file/directory/parent identity, extra inventory, and retained
unsealed or dropped scratch. Platform-specific cases require their actual hosts;
zero selected tests on another host are not evidence.

CLI regressions check that invalid source and rejected native emission leave
the former predictable executable sentinel's identity and bytes untouched.
Checking/emission before allocation is explicit implementation ordering, not
a filesystem-allocation trace measured by those CLI regressions. Private compiler
boundary tests inspect actual scratch paths, source bytes, and command arguments
with injected process results. They must preserve primary failures, retain
uncertain/partial output, and permit cleanup only through the success path;
injected success is not evidence of binary creation or real Clang execution.
Existing native output, command-mode, exit-status, generated-byte and release
smoke gates remain required.

Focused commands, documented but not executed:

```sh
cargo test --locked -p semaprax --lib codegen::native_emit::native_scratch
cargo test --locked -p semaprax --lib codegen::native_emit::scratch_tests
cargo test --locked -p semaprax --bin semaprax native_scratch
cargo test --locked -p semaprax-toolchain --bin semaprax-full native_scratch
```

These tests are authored, not executed. Formatting and adversarial static review
do not prove host execution, release readiness, or completion-matrix promotion.
