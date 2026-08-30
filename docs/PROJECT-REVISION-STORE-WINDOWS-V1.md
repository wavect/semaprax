# Project Revision Store Windows-entry v1

Status: **authored, locally unrun, unpublished, and unpromoted**. This is a
separately selected Windows authority, not a supported-platform declaration.

## Additive API and bytes

`project_revision_store::identify_windows`, `persist_windows`, and
`load_windows` have the same parameters and result types as ordinary
`identify`, `persist`, and `load`. Identification is authority-free and
platform-independent. Windows persistence/loading reject unsupported hosts;
ordinary v1 persistence/loading retain their existing Unix-only admission.

The new schema is `semaprax.project-revision-store-windows-entry.v1`; its entry
digest domain is `semaprax.project-revision-store-windows.entry-digest.v1\0`.
Length framing and the exact entry tree, field order, Project/source bindings,
limits, and complete replay follow [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md).
Only two ordered nonclaims change in the new profile:

- `requires_trusted_exclusive_current_euid_root` becomes
  `requires_trusted_exclusive_effective_sid_root`.
- `no_windows_store_support` becomes
  `no_windows_network_remote_or_non_ntfs_store_support`.

All ordinary v1 schema/domain/nonclaim bytes remain unchanged. The profiles
reject each other's entries; there is no implicit conversion, migration,
adoption, replacement, or platform-dependent identity. A Windows locator can
resolve an uncertain publication only through `load_windows`, never by itself.
Project Manifest v1-v10, Transport v2-v5, Workspace, graph, diagnostics, and
target contracts are not widened.

## Admitted host authority

The injected root must already exist on a fixed local NTFS volume, with a
normalized drive-absolute path. UNC, device, relative, mapped/remote,
non-NTFS, dot/dotdot, alternate-separator, stream, reserved-device, trailing
dot/space, reparse, and short-alias spellings reject. The root and its complete
ancestor chain must be free of short aliases; short-name generation must not
create aliases for the fixed long stage/entry filenames. The implementation
does not change volume settings or clear metadata to obtain admission.

The root's owner is the effective thread-token SID, or the process-token SID
only when no thread token exists. The protected, non-null DACL has exactly two
explicit allow ACEs: that SID and LocalSystem, with the exact admitted access
mask. Inherited, group, Everyone, Administrator, deny, callback, object, or
extra ACEs do not substitute for this policy. LocalSystem as the effective
caller is not the distinct two-principal profile. Effective token identity and
mutation state are rechecked throughout the invocation.

The unpublished `semaprax-project-revision-store-windows-sys` crate quarantines
unsafe FFI. The compiler crate remains unsafe-free. The safe boundary exposes
opaque held root/directory/file handles and authenticated facts, not raw
handles or reusable publication receipts. Drive-anchor and component-relative
opens reject reparses. Facts bind volume identity, 128-bit file identity,
attributes, regular-file link count, and size. Alternate data streams and
8.3 aliases reject. Records, names, reads, and inventory growth are bounded.

A root-identity-derived nonblocking named mutex serializes cooperating callers.
Its owner and protected DACL are independently authenticated using mutex-specific
rights. Busy or squatted names reject. Mutex ownership remains thread-affine;
this is not an exclusion boundary against another same-principal process.
The host must exclude uncooperative namespace/content mutation of the root,
ancestors, and owned stage for the invocation.

## Publication and settlement

Canonical carrier preparation and subject-fact replay precede filesystem
publication authority. The Windows route then authenticates the held root and bounded
retained inventory, creates one exact new stage, and creates the fixed entry
tree relative to retained created parents. Created objects receive the explicit
protected descriptor. No later write substitutes a freshly reopened parent
path for its retained creation authority.

Created files are read through retained handles and exact-compared with the
prepared bytes. Inventory and complete Project replay are checked before
publication. File/directory flushes and explicit handle settlement are checked;
failures are never silently treated as synchronization success. Root/path and
inventory rechecks precede the single same-root handle-relative
`FileRenameInformationEx` operation. Its flags are zero: no replacement, POSIX
semantics, fallback, or retry.

After a successful rename, the published name must reopen as the retained
stage identity. Exact bytes, inventory, full Project replay, root binding,
absence of the old stage name, and final handle/mutex settlement precede
success. `SPX-I216` denotes post-pivot or rename uncertainty; the possibly
visible entry is never deleted. Earlier failures preserve any inert stage.
No cleanup, rollback, repair, eviction, recovery, or adoption route is added.

Read-only loading may quarantine one exact inert-stage top identity. Unlike
the Unix `stat`-based check, Windows opens its top directory handle solely to
authenticate owner/DACL/type/identity and then closes it. It does not enumerate
children, read stage files, adopt, delete, repair, or publish the stage.

Flush calls do not establish a power-loss durability or filesystem-wide
synchronization guarantee. There is no volume-wide flush, privilege elevation,
network, process/tool, build, target execution, or daemon authority.

## Evidence and promotion

Evidence is authored and unrun. The quarantine's focused tests cover closed
path/name grammar and protected-DACL admission; shared profile tests bind
distinct Windows identities and preserved legacy bytes. Physical NTFS fixtures
must run under their explicit private-root/short-name prerequisites before
any Windows support claim. Missing prerequisites must fail visibly when the
explicit physical gate runs, not turn a skipped assertion into a passing
support gate. Provisioned-host tests are marked ignored by default so ordinary
workspace checks do not pretend to provision the host or exercise this gate.

The quarantine fixtures require `SEMAPRAX_WINDOWS_REVISION_STORE_TEST_PARENT`;
the compiler-level calculator Project round trip requires a separately
provisioned empty `SEMAPRAX_WINDOWS_REVISION_STORE_TEST_ROOT`. Both must meet
the host contract above. Fixtures retain their exact owned files for inspection;
neither test setup nor the store changes volume settings or recursively cleans
the host. On that provisioned Windows host, explicitly run:

```text
cargo test --locked -p semaprax-project-revision-store-windows-sys --lib -- --ignored --test-threads=1
cargo test --locked -p semaprax --all-features --test project_revision_store_windows_v1 -- --ignored --test-threads=1
```

Promotion requires real create/read/publish/load round trips, every admitted
Project profile, same-byte substitution, foreign-byte preservation,
hard-link/reparse/ADS/short-alias rejection, token drift, mutex contention and
squatting, exact count/read boundaries, pre-pivot flush/close failures,
post-pivot uncertainty, and legacy v1 byte preservation on required hosts.
Static review and formatting alone do not satisfy that gate.
