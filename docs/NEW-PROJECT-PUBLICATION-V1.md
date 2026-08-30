# Calculator project publication v1

Status: reviewed correction contract; new executable evidence remains unrun.

Audience: toolchain contributors, host integrators, and reviewers.

## Scope and unchanged interface

The unpublished full toolchain owns `semaprax-full new <destination>` and the
built-in calculator template. Tag archives expose that full CLI as `semaprax`;
the standalone registry compiler does not gain private-host dependencies.
See the [quickstart](QUICKSTART.md) for the user workflow and
[Project Manifest v1](PROJECT-MANIFEST-V1.md) for checked project semantics.

This correction changes publication verification, not the command grammar,
template names, Project schema, source semantics, or successful file bytes.
Only the existing optional `--name` and `--template calculator` are admitted.
The exact generated inventory remains:

- `README.md`
- `semaprax.toml`
- `src/app.spx`
- `src/tests.spx`

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

After successful rename, failed content or path binding reports failure and
retains the complete published tree, even if its original name has been moved.
It must not delete that tree or a foreign replacement at the requested path.
There is no post-publication rollback. Callers reconcile retained output and
current paths externally.

## Required evidence

Preserve existing deterministic templates, Project check/test/Web behavior,
CLI grammar, pre-write parent/stage substitution, inventory rejection, and
fresh-output tests in `tests/cli_new_project_v1.rs`.

New regression evidence must cover successful relative and parent-relative
inputs; substitution after physical rename of the published directory and its
parent; original ancestor-alias displacement; unchanged foreign sentinels and
the displaced original inventory after error and drop; and partial/untracked
stage residue that is not adopted for cleanup.

Windows-specific tests must force the extended-to-legacy transition and inspect
the actual submitted replacement field. They also exercise native legacy
success and existing regular-file/directory collision preservation. A physical
directory-collision test alone cannot detect the wrong flags field because the
OS can reject directory replacement independently.

No new tests, builds, Windows execution, or hosted gates have run in this
correction batch. Static review and formatting cannot promote WP-06 or any
completion-matrix row.
