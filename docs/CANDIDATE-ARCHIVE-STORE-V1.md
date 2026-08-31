# Candidate archive store v1

Status: additive Unix implementation and regressions authored; unrun and unverified.

Audience: embedding hosts, compiler contributors, and agent workflow integrators.

This store persists one complete, source-backed `ProjectCandidateArchive` as an
immutable file under an explicitly selected host root. It is separate from the
Project Revision Store and changes neither its format nor its authority. No
root is discovered from a manifest, current directory, environment, archive,
candidate, or protocol request. Stored bytes are recovery inputs, never trusted
HIR, source-commit approval, or evidence of current source admission.

The additive [typed-draft persistence](DRAFT-ARCHIVE-PERSISTENCE-V1.md) APIs
share this root format and its aggregate limits. A root may contain complete
candidate and draft archives, but each selected loader admits only its own
exact archive kind. Existing candidate APIs and replay rules are unchanged.

## Public API

```rust
pub fn persist(
    root: &Path,
    archive: &ProjectCandidateArchive,
) -> Result<CandidateArchiveStoreReceipt, Vec<Diagnostic>>;

pub fn load(
    root: &Path,
    expected_archive: &str,
    expected_candidate: &str,
) -> Result<ProjectCandidate, Vec<Diagnostic>>;
```

The functions are exposed through `semaprax::candidate_archive_store`.
`CandidateArchiveStoreReceipt` has private fields and only borrowed
`archive_digest()`, `candidate_digest()`, and `base_revision()` getters. It
contains no path, open handle, root, approval, or reusable authority; it is not
serde data, `Clone`, or `Default`. The archive's identities can be retained
before publication to resolve an uncertain outcome with ordinary `load`.

`persist` independently restores the typed archive and checks its original base
before opening a filesystem root. It prepares the receipt before publication.
`load` independently reconstructs the embedded canonical original Project
manifest and source inventory, replays complete candidate intentions, and
checks exact archive/candidate identities and canonical bytes through
`ProjectCandidateArchive::restore`. It does not read the original source paths.
Removal of those original files does not prevent archive restoration. A restored
candidate still requires a separately admitted current base and independent host
authority before any eventual source commit.

## Root authority and inventory

The root must already exist as an exact `0700` directory owned by the current
effective user. Its spelling must be absolute and normalized: no relative,
empty, filesystem-root-only, dot, parent, repeated-separator, or trailing-slash
spelling. Paths are bounded to 4,096 bytes and 64 normal components. Every
component is opened with directory and no-follow flags; each ancestor descriptor
and its identity remain held through the operation. Before returning or
publishing, the implementation checks held component identities, parent-child
links, and an independently reopened absolute chain.

The host must exclude uncooperative mutation of this root, its ancestors, and
the selected/staged file by another process acting as the same principal for
the whole invocation. Mode checks do not inspect ACLs or revoke other existing
handles. On hosts with ACL authority beyond Unix mode bits, the host guarantee
must exclude that authority as well. A nonblocking advisory lock on the held
root serializes cooperating store writers and readers. It is not protection
against a same-user peer ignoring the lock, filesystem administrator, malicious
mount replacement, or kernel compromise.

The complete root inventory consists only of:

```text
<64 lowercase archive-digest hex digits>.json    # 0–32 completed archives
.stage-<64 lowercase archive-digest hex digits>  # at most one inert failed stage
```

The `sha256:` prefix is omitted from filenames. Each entry must be a current
effective-user-owned regular file with exact `0600` permissions and exactly one
hard link. Symlinks, directories, device/FIFO/socket objects, unexpected names,
extra stages, and excess entries reject. Completed files must be nonempty. All
file sizes are bounded by `MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES` (128 MiB); an
inert stage may be empty or incomplete. Root enumeration and metadata checks
are bounded to 32 completed entries plus one stage.

Unselected completed entries are checked for name, identity, owner, mode,
single-link status, and bounded size only. Their content is **not** read,
hashed, parsed, or semantically authenticated. A successful selected load does
not attest to the contents of every file in the store. This distinction avoids
replaying up to 32 unrelated archives on each operation; it is not a full-store
integrity claim or a bound on all retained compiler memory.

## Publication and load behavior

Under the nonblocking exclusive root lock, `persist` rejects any existing stage
or selected destination. Even byte-identical existing destinations are not
adopted. It requires a free completed-entry slot, creates exactly one stage with
create-new/no-follow flags, and authenticates its empty held file and root
inventory before writing. There is no implicit root creation or repair of
permissions masked by the caller's umask.

It writes the archive's exact canonical bytes, syncs the file, reads back and
compares every selected byte, and checks the root chain, inventory, and selected
file identity again. After syncing the directory and repeating the selected
readback, a no-replace rename publishes the stage under the digest filename.
The store then syncs the root, checks the published path against the held stage
identity, rechecks exact bytes and inventory, and releases the lock. It never
overwrites, appends to, adopts, removes, or renames another completed entry.

`load` takes a nonblocking shared lock and permits at most one structurally
valid inert stage. It opens only the selected completed file with no-follow and
nonblocking flags, checks the held metadata, and reads at most its bounded
declared size plus one byte. The independent archive restoration runs while the
root lock and descriptors remain held. Before returning a candidate it checks
the root/inventory and rereads the selected bytes for exact equality. The
operation does not delete or resume a failed stage.

Before successful rename, failures leave any created stage as it stands. A stage
blocks later `persist` calls but does not hide otherwise valid completed entries
from `load`. After successful rename, every settlement, observation, readback,
or lock-release failure returns `SPX-I361`: publication may have occurred.
Neither branch performs cleanup, rollback, adoption, retry, eviction, or garbage
collection. An uncertain publication must be inspected through exact `load`
using the independently retained archive and candidate identities; callers must
not treat the error as permission to retry publication blindly.

Sync calls express the local write/rename ordering. They do not establish power
loss guarantees, network/NFS/overlay filesystem semantics, crash recovery, or
durability across every storage device. The implementation is enabled on the
same Unix target families as the existing handle-relative no-replace store
(Linux, Android, Apple, Redox). Other targets, including Windows, fail closed
without opening or mutating a store. This is not Windows store support.

## Diagnostics and authored evidence

| Code | Meaning |
| --- | --- |
| `SPX-G300` | Invalid digest syntax, root spelling, or fixed path bounds. |
| `SPX-G301` | Completed-entry or selected-file capacity exceeded. |
| `SPX-G302` | Root/file/inventory binding disagreement, busy lock, missing entry, failed-stage blocker, or existing destination. |
| `SPX-I360` | Filesystem failure before confirmed publication, or load/unlock failure. |
| `SPX-I361` | Failure after successful publication rename; outcome must be resolved explicitly. |

Independent archive/compiler diagnostics are preserved. They are not converted
to a claim that stored source or a candidate is valid.

`src/candidate_archive_store/tests.rs` authors real filesystem cases for exact
persistence/restoration after original-source removal; no adoption; wrong
candidate binding and same-length tampering; root permissions/spelling/links;
cooperative lock contention; selected symlinks/hard links and foreign entries;
bounded metadata-only inventory; retained failed stages; hostile pre-publication
stage substitution; and a post-publication observation failure resolved through
exact load. Fault callbacks are private test seams around real filesystem
operations. The post-pivot case performs the real rename before an injected
observation error; it does not claim a physical crash or fsync fault was induced.

All these regressions remain unrun in this change. No compiler gate, test,
interpreter, target runtime, or filesystem fixture was executed during authoring.
No semantic cache speedup, serialization of trusted HIR, draft persistence,
source publication, network, build, or approval authority is provided. The
graph-operational full goal remains Partial.
