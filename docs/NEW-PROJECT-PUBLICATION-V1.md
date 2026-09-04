# Calculator project publication v1

Status: correction with local macOS/Linux evidence; Windows and hosted gates
remain required.

Audience: toolchain contributors, host integrators, and reviewers.

## Scope and unchanged interface

The unpublished full toolchain owns `semaprax-full new <destination>` and the
built-in calculator template. Tag archives expose that full CLI as `semaprax`;
the standalone registry compiler does not gain private-host dependencies.
See the [quickstart](QUICKSTART.md) for the user workflow and
[Project Manifest v1](PROJECT-MANIFEST-V1.md) for checked project semantics.
The separate [Public Project Scaffold Capsule v1](PROJECT-SCAFFOLD-V1.md)
derives and replays the same four file bytes without a destination or write
authority; it does not replace this held-parent publication protocol. The
standalone compiler's `new`, owned by [standalone project creation
v1](NEW-PROJECT-STANDALONE-V1.md), writes the same bytes through a bounded
create-new route without this protocol's staging or identity re-verification.

This correction changes publication verification, not the command grammar,
template names, Project schema, source semantics, or successful file bytes.
Only the existing optional `--name` and `--template calculator` are admitted.
The exact generated inventory is the [Public Project Scaffold Capsule
v2](PROJECT-SCAFFOLD-V2.md) inventory:

- `README.md`
- `AGENTS.md`
- `semaprax.toml`
- `src/app.spx`
- `src/tests.spx`

(`AGENTS.md` was added by scaffold v2; the root inventory the held-parent
authority authenticates and publishes grew from two files to three.)

There is no template discovery, arbitrary template input, network access,
dependency installation, Git initialization, recursive cleanup API, general
workspace mutation, or target process execution in the generator.

## Preparation and held authority

The toolchain renders only compiled-in templates, validates their complete
fixed inventory, and invokes ordinary Project semantic construction and the
bounded in-process test evaluator on those exact owned bytes before staging.
It does not reopen an ambient staging pathname as semantic input. Physical
held-file comparison and exact directory inventories subsequently bind the
staged files to the same checked bytes.

The destination is a fresh child of an existing parent. One absolute requested
destination spelling is captured at entry, without resolving symlinks; its
parent spelling and the parent's initial identity remain available for final
comparison. The existing canonical parent is still used to acquire lower
held authority. Final checks must preserve previously admitted relative and
parent-relative paths rather than sending `..` through a lower API that only
accepts normal absolute components.

The lower `NewProjectAuthority` retains the exact parent and created stage
handles, fixed child names, file inventories, and expected absolute parent and
published paths. Expected path storage is prepared before namespace creation.
Files are written create-new through retained directory authority. Source and
root inventories are authenticated before descendant settlement and rename.
No byte digest or pathname alone grants deletion or publication authority.

Generated staging names must be distinct from the requested destination.
The CLI skips an exact or ASCII-case-equivalent candidate within its existing
32-attempt budget, before acquiring an authority that creates directories.
The lower authority independently rejects that pair with `Invalid` before
namespace creation; an already-existing output retains `Exists` precedence.
This prevents `.semaprax-new-<pid>-<serial>` from becoming the final path while
the calculator is still being staged. It does not reject that CLI destination:
the next noncolliding staging candidate can still publish to it.

The generated names are ASCII, and Windows already limits these child names
to ASCII. The comparison conservatively excludes ASCII-case aliases on Unix
too; it is not a general Unicode normalization or filesystem-name equivalence
oracle. Arbitrary Unix names and unusual filesystem alias rules remain a
separate authority boundary, not a guarantee supplied by this comparison.

## Publication and success binding

Publication remains one same-parent no-replace directory rename. The shared
Windows primitive uses the existing extended rename attempts and legacy
fallback; their retry schedule and error/handle-close behavior are unchanged.
Before the legacy call, the complete shared flags word is explicitly reset to
zero. The extended API interprets that storage as flags, but the legacy API
interprets its first byte as `ReplaceIfExists`; reusing the extended POSIX flag
would request replacement. This follows Microsoft's
[FILE_RENAME_INFORMATION definition](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information).
It is a correction of requested authority, not evidence of a demonstrated
directory-overwrite exploit; Windows independently restricts replacement of
existing directories.

Immediately after rename succeeds, the authority latches publication before
any hook, reopen, content verification, or path comparison. Published state
never regains pre-publication cleanup authority.

The descendant `src` handle is released before the rename, preserving the
Windows publication prerequisite. A failed rename must return its selected
error before any `src` reopen. Reopening is verification-only after successful
publication, never a way to reacquire cleanup authority. Previously, an
unconditional reopen after failure could adopt an independently created `src`
replacement containing the original tracked files. Their matching file
identities did not make the replacement directory owned, yet subsequent cleanup
could delete it. Returning before the reopen preserves that ownership history
without changing successful publication or the platform rename implementation.

Success requires all of the following observations:

1. Reopened held files exactly match the checked template bytes and both
   directory inventories remain exact.
2. The retained parent still binds its expected absolute parent path.
3. The retained published stage still binds the expected destination path.
4. The CLI's captured original parent spelling still resolves to its initial
   identity, including when canonicalization originally traversed an alias.

These are checked observations, not a filesystem lock, permanent binding,
atomic visibility across multiple path queries, hostile same-principal
isolation, crash recovery, or power-loss durability. The host must continue
to control its selected parent and ancestry. A later caller can change a
pathname after any successful observation.

## Failure and preservation

Malformed invocation remains exit 2; creation/publication failures remain
exit 1. Lower errors retain the existing `Exists`, `StageExists`, `Changed`,
and `Invalid` vocabulary. No new wire schema or diagnostic alias is introduced.

Before publication, cleanup is limited to the exact held stage inventories.
An incomplete write, untracked file, changed identity, or foreign inventory
can leave inert staging residue. Failure does not promise that every staging
directory disappears; cleanup must not infer authority from a nonce prefix.

Once the original `src` handle has been released for publication, a failed
rename leaves the stage inert, including an ordinary output collision. Dropping
the authority performs no source or root discard. The original rename error
remains selected even if `src` is now absent or cannot be reopened. A later
publication call on that authority rejects before another rename because its
original source authority is gone. Earlier preparation failures retain their
existing exact-inventory cleanup behavior.

After successful rename, failed content or path binding reports failure and
retains the complete published tree, even if its original name has been moved.
It must not delete that tree or a foreign replacement at the requested path.
There is no post-publication rollback. Callers reconcile retained output and
current paths externally.

## Required evidence

Preserve existing deterministic templates, Project check/test/Web behavior,
CLI grammar, pre-write parent/stage substitution, inventory rejection, and
fresh-output tests in
`crates/semaprax-toolchain/tests/cli_new_project_v1.rs`.

The new-project destination and quickstart output-parent rejection fixtures
must create a real directory link before exercising rejection. They share one
test-only helper: Unix uses a symbolic link; Windows uses a directory junction
and verifies its reparse attribute and exact fixture-owned target. Creation
failure fails the test instead of silently omitting the hostile case when
Windows symbolic-link privilege is unavailable. The rejection assertions are
unconditional and retain the link, foreign sentinel bytes and exact target
inventory before explicit link cleanup. This is Windows junction evidence,
not evidence that privileged Windows symbolic-link creation was exercised.
Production path admission and successful template bytes are unchanged.

New regression evidence must cover successful relative and parent-relative
inputs; substitution after physical rename of the published directory and its
parent; original ancestor-alias displacement; unchanged foreign sentinels and
the displaced original inventory after error and drop; and partial/untracked
stage residue that is not adopted for cleanup.

Failed-rename evidence must also cover a real output collision after descendant
release, both with unchanged `src` and with a substituted source directory.
The Unix substitution case moves the original tracked files into an
independently created replacement directory: exact bytes and file identities
alone must not authorize deleting that directory. Retain directory identity
witnesses and require the original displaced directory, replacement directory,
tracked files, stage root and foreign output to survive error and drop. A
missing or linked `src` must not mask the primary collision error or authorize
another publication attempt. These cases are bounded namespace-substitution
observations, not hostile same-principal isolation or crash recovery.

Staging-name regressions must use invocation-local candidate selection rather
than racing or resetting the process-global serial. Cover exact and ASCII-case
collisions, the unchanged attempt ceiling, successful publication with exact
template bytes, final-path absence during staging/writes and injected failure,
and lower-level rejection without creating any child. Existing-output errors
and foreign bytes must remain unchanged. The CLI collision/binding cases passed
locally on macOS; lower-level and Windows-specific gates remain separate.

Windows-specific tests must force the extended-to-legacy transition and inspect
the actual submitted replacement field. They also exercise native legacy
success and existing regular-file/directory collision preservation. A physical
directory-collision test alone cannot detect the wrong flags field because the
OS can reject directory replacement independently.

The full `cli_new_project_v1` test binary passed 15 tests locally on macOS arm64
with Rust 1.98. The separate quickstart suite passed nine tests and the version
suite passed six. These results do not prove installation, release archives,
Windows execution, or hosted gates, and cannot promote WP-06 or any
completion-matrix row without its remaining required evidence.

The failed-rename replacement-directory regression was executed against the
old ordering on both macOS arm64/Rust 1.98 and Linux arm64/Rust 1.88: dropping
the failed authority actually removed the replacement source directory and
stage. The same unchanged regression passes after propagating the rename
failure before reopening. All nine lower project-publication tests and all
15 calculator CLI cases pass on both hosts; the complete lower package's
46 unit tests and warnings-denied package Clippy also pass on macOS. Linux
used the existing offline, capability-dropped container with read-only source.
The CLI's relative-path fixture requires a writable working directory: the
initial read-only-checkout run failed setup, then the exact Cargo-reported
test executable passed all 15 cases when launched from container `/tmp`.
This is not a source/test relaxation, Windows evidence or a full quality run.

Focused gates, to run on the required hosts before promotion:

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib platform::publish_tests
cargo test --locked -p semaprax-native-rust-owned-data-package --lib project_publication::tests
cargo test --locked -p semaprax-toolchain --test cli_new_project_v1
```

The first gate requires Windows; a zero-test result elsewhere is not evidence.
Physical post-rename displacement and ancestor-alias fixtures are Unix-only.
Common success, publication-latch unwind, residue and relative-path cases still
require execution on every supported host. Existing full quality and Project
gates remain independently required.
