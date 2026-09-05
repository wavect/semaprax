# Project Candidate Git Publication v1

Audience: host integrators and compiler contributors.
Status: bounded Linux/macOS bare-SHA1-or-SHA256 source-publication route; the
focused current boundary and real-Git regressions pass locally on macOS. The
Linux held-descriptor cases are authored and remain unrun on this head. No
hosted, Windows, full-profile or completion-gate evidence is claimed.

This route writes actual canonical `.spx` blobs, trees and a commit into one
explicitly selected local Git repository, then publishes through one expected-old
branch-ref update. It does not update a checkout, index, raw Project source path,
managed Workspace `ACTIVE`, remote, or network service. Git-tree readers can
observe one complete commit; editors observing the original working tree see no
change. This is separate authority from candidate preparation, recovery capsules,
reports, test results, and the managed publication bridge.

## Explicit host interface

`CandidateGitProcessAuthority::open(executable, repository, max_commands,
timeout_ms)` admits a host-controlled bare SHA1 or SHA256 repository on Linux or
macOS. Both paths
must be absolute; the repository path must already be canonical. The explicitly
trusted Git executable may be a symlink, resolved to a held regular executable.
No executable, repository, branch, author, timestamp or message comes from the
candidate or recovery capsule. Windows and other process hosts fail closed.
Ordinary SHA1 repositories are supported for Git compatibility; they do not gain
a collision-resistance or collision-detection claim.

`CandidateGitTarget::new(repository_identity, reference, expected_base_commit,
project_prefix)` binds the exact adapter identity, a bounded `refs/heads/` name,
a 40-lowercase-hex SHA1 or 64-lowercase-hex SHA256 object ID, and an optional relative Project prefix.
The prefix cannot contain dot, parent, `.git`, empty, control or backslash
components. `CandidateGitCommitMetadata::new(name, email, unix_seconds, message)`
requires explicit author/committer identity and UTC time, and a bounded message
ending in one LF. No ambient Git identity, clock, signer or editor participates.

`apply_candidate_git_publication(candidate, approved_candidate_digest,
project_manifest, target, metadata, authority)` is the publication API. Its
`CandidateGitAuthority` interface permits injected trusted hosts; the supplied
process adapter performs real Git object writes and ref mutation. Implementing
the interface is a host authority decision, not a claim that arbitrary providers
are trustworthy. The manifest is the exact absolute authenticated Project path.
A complete candidate and independently supplied exact approval digest are
required. Drafts and rejected attempts cannot publish.

The CLI is an explicit host operation:

```
semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>
```

The host policy file selects authority independently from the replayed recovery
capsule. This does not grant source publication to any Image Agent Protocol
profile. Invoking the CLI grants its selected local repository/ref operation;
there is no automatic push or original-checkout update.

## Replay and Git transaction

1. Load all declared Project inputs with held identities, require the original
   candidate base and unchanged manifest/source inventory, and independently
   replay the complete ordered candidate history through ordinary admission.
2. Authenticate the selected current branch against the exact host-selected base
   commit. Read and independently hash the commit, traversed trees, canonical
   manifest, and **every original declared source blob**, including unchanged
   sources. Git blob contents must equal the authenticated canonical original
   Project contents. Source paths must be ordinary blob entries, never symlinks
   or gitlinks. A candidate with no changed source is rejected.
3. Preserve every unrelated tree entry, object identity, mode and name. Replace
   changed canonical source blobs only, preserving their regular-file modes.
   Build binary repository-format trees and one explicit commit with the selected original
   commit as its sole parent. Unrelated histories and signatures are not copied
   into the new commit. Git tree object names occupy 20 binary bytes for SHA1 and 32 for SHA256.
   [Git object-format transition](https://git-scm.com/docs/hash-function-transition/2.52.0.html).
4. Bound and render the complete receipt before writing objects. Recheck held
   Project inputs, write content-addressed immutable objects, recheck inputs and
   repository, then call `update-ref --no-deref` once with the exact old object ID.
   Git owns the atomic expected-old ref update; this implementation does not
   emulate it with loose-ref file writes.
   [Git update-ref](https://git-scm.com/docs/git-update-ref.html).
5. Recheck Project inputs and observe the ref after a successful pivot. Any
   uncertain update or disagreement after publication returns `SPX-G267`, with
   the prepared commit and ref. The caller must inspect the ref before retrying.

The receipt schema is `semaprax.project-candidate-git-publication.v1`; it binds
repository/ref, previous/published commit, result tree, approved candidate digest,
original/result Project revisions and changed paths. It explicitly says raw
working-tree files and managed `ACTIVE` were not changed and tests were not run.
Git commit content identity is not a signature or an approval service.

## Process and filesystem boundary

The process adapter holds an exclusive permanent
`.semaprax-git-publication.lock` file in the repository. Dropping the adapter
explicitly unlocks its lease, including admission-error and unwind paths; the
lock file is never deleted. Close-on-exec remains set, but closing the owning
descriptor alone must not let a concurrent fork's pre-exec duplicate extend the
lease beyond the adapter's lifetime. This coordinates cooperating
publication hosts. Ordinary concurrent Git writers remain subject to Git's ref
CAS. The repository and executable are **host-controlled inputs**: neither this
lease nor repeated pathname checks protects against a malicious same-UID process
racing namespace/content mutations between checks. Such adversarial shared
storage is outside this adapter's authority model.

Admission accepts only a minimal bare config: `core.bare=true`, version 0
without extensions for ordinary SHA1, or version 1 with absent/explicit `sha1`
object format or explicit `extensions.objectformat=sha256`. Optional boolean
`core.filemode` and `core.logallrefupdates` remain accepted. Version 0 with an
object-format extension is rejected rather than guessing whether Git ignores it. Unknown/duplicate settings, includes, alternate object
stores, common-directory indirection, attached worktrees, shallow history and
grafts are rejected. Before each process call, held repository/config/executable
identities are checked, config bytes are compared, and a bounded recursive scan
rejects nested symlinks, special files and multiply linked regular files anywhere
in the repository, including refs, reflogs, packs and loose objects. Git executable
device, inode, size, mode and modification timestamp are held and rechecked. These checks do
not confer trust on an untrusted binary selected by a host.

Linux launches every process from the executable image and repository directory
held by the authority, using handle-relative working-directory selection and
descriptor execution rather than reopening either authenticated pathname.
macOS derives the launch path from the held executable, starts the child
suspended with a handle-relative working directory, attests its executable vnode
and working directory against the held objects, and resumes it only after
agreement. A namespace substitution that prevents exact Darwin vnode agreement
fails closed before resume. The child receives only its exact
standard-pipe inventory; every other descriptor is closed. It has an empty
inherited environment and explicit null global/system config, no replacement
objects, no lazy fetching, no terminal prompts and no optional locks. A macOS
safety entry pre-seeds CoreFoundation's user-text-encoding key with the process
UID and fixed encoding fields, preventing CoreFoundation's fallback from reading
the user-home encoding file and rewriting the child environment. A fixed
command-line `protocol.allow=never` forbids transport;
there are no network commands or request-selected command names. Hashing uses
`--no-filters`. Fixed `core.hooksPath=/dev/null` disables hooks, including
`reference-transaction`, which otherwise executes during ref operations.
[Git hooks](https://git-scm.com/docs/githooks). Shells, aliases, signing, filters,
credential helpers and inherited SSH/secret variables are not used.

The syscall implementation is the root registry crate's only locally allowed
unsafe module. The root manifest denies unsafe code everywhere else, and
`root_unsafe_quarantine` source-locks that single exception and its private API.
This packaging-preserving quarantine is narrower than exposing a generic safe
process facade but is not a claim that arbitrary unsafe additions are accepted.

Only `symbolic-ref`, `show-ref`, raw `cat-file`, raw `hash-object` and
`update-ref` are invoked. Synchronous bounded pipe handling, a fresh process
group, one host-selected total adapter deadline (1–60,000 ms), and one command
budget (1–4,096) cover input, output, and ordinary child execution. Success
or ordinary failure is returned only after the leader is reaped and the owned
process group is quiescent. Timeout, output overflow, pipe failure, or any other
post-spawn failure selects a sticky error, kills the owned group, drains bounded
output while the command is active, closes the pipes, and settles it. Settlement
has a separate fixed 30-second fail-stop allowance after the operation deadline;
inability to prove settlement aborts rather than returning a recoverable error.
The operation deadline begins at adapter opening and includes time
spent by the caller before publication. No claim is made that a trusted executable
can be physically preempted inside an uninterruptible kernel operation.

Core limits include 64 MiB/object, 8 MiB/tree, 256 MiB total read bodies,
256 MiB staged bodies, 4,096 objects, 65,536 visited tree entries, 64 path
components and 60 seconds of checked orchestration work. Adapter limits also
include 64 KiB config/stderr, 512 MiB conservatively reserved total process I/O,
and 65,536 filesystem entries/depth 64 per scan. Limits apply to this bounded
route, not all Git repositories or total HIR memory.

## Failure semantics and authored evidence

`SPX-G263` rejects host/grammar/object-shape constraints; `SPX-G264` reports core
capacity; `SPX-G265` rejects stale source/ref/blob identity; `SPX-G266` reports
pre-pivot host failure; `SPX-G267` reports an uncertain or already-observed pivot.
Existing Project/candidate diagnostics remain authoritative for admission.
A rejected pre-pivot operation leaves the ref unchanged but may leave unreachable
objects and the permanent host lock file. Once an update was attempted, process
failure is conservatively uncertain even when Git returned nonzero. There is no
rollback that might overwrite a concurrent writer's ref.

`tests/project_candidate/git_publication.rs` exercises real bare-SHA256 Git
publication, unrelated entry/mode preservation, unchanged raw sources, disabled
ref hooks, stale-ref/original-blob rejection, unsafe-config rejection and nested
object-store symlink rejection. It also checks live-host contention, explicit
lease release, rejected-admission recovery, unchanged source/ref state and lock-file
retention and directory substitution. Private held-runner tests cover executable
substitution, expired authority deadlines, descriptor/environment isolation,
bounded stdout/stderr, and child-group settlement.
`SEMAPRAX_TEST_GIT` can select the trusted fixture binary; otherwise the
Unix fixture selects `/usr/bin/git`. On this current macOS head, the eight active
held-runner cases, seven real-Git cases, root quarantine contract, and four
integrated SHA1/SHA256 workflow cases pass locally; strict root Clippy also
passes. Linux descriptor-execution cases remain authored/unrun. The later exact-subject
[graph-workflow bundle](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md) is local
Darwin evidence only. Neither result is current-head hosted or Windows evidence,
and the complete quality profile was not rerun for this tranche.

## Additive legacy SHA1 compatibility

The old public constructors and `CandidateGitRepository` fields remain source
compatible. `GitObjectFormat::{Sha1,Sha256}` names the admitted algorithms;
`repository.object_format()` maps its existing `sha256` flag, with false now
explicitly meaning SHA1. The process adapter also exposes `object_format()`.
Target OID width is checked against the held host format before object reads.
Tree parsing, hashing, staged writes and CAS all use that same format. Existing
SHA256 success receipts retain their previous field set and exact serialization.

A small private streaming SHA1 implementation hashes Git's exact object framing
for compatibility; no dependency download or SHA1 collision-detection library is
introduced. It is never used for semantic revision identity or approval digests.
Original Project blobs still require exact byte equality against independently
admitted canonical sources. Every staged SHA1 object is reread before the ref
pivot and compared byte-for-byte with its prepared body, rejecting an existing
object that has the same SHA1 name but different bytes (`SPX-G276`). The usual
read/output/object/fuel bounds apply to these additional reads.

SHA1 receipts additionally contain `sha256_object_content_binding`: a SHA256
binding over the exact sequence of authenticated read objects followed by staged
objects. The first domain is `semaprax.git-publication.read-objects.v1` plus NUL;
the second segment starts with `semaprax.git-publication.staged-objects.v1` plus
NUL. Each object contributes its OID text length as big-endian u64, OID text,
Git's `kind length` header plus NUL, then exact body bytes. Traversal/staging order
is deterministic. Post-write rereads are excluded from the receipt binding. This
binds observed and prepared content, not a signature, a proof that the whole Git
history is collision-free, or authentication of unrelated untraversed objects.
The existing candidate/Project SHA256 identities remain separate bindings.

`sha1_security` explicitly reports
`legacy_git_compatibility_no_collision_detection_or_collision_resistance_claim`.
The adapter does not implement SHA1DC, promise rejection of every known collision
family, or promote SHA1 to a modern integrity primitive. A Git executable's own
collision defenses remain its responsibility. Host-controlled storage and the
previous cooperative-race limits still apply. SHA256 is the format for hosts that
require Git object naming with the stronger hash.

Regressions cover actual SHA1 bare publication and byte comparison,
format/width mismatch rejection, strict version/extension config admission,
standard SHA1 known answers (empty, `abc`, the 56-byte vector and one million
`a` bytes), split-update padding checks, and known Git empty blob/tree OIDs. No
compiler, tests, or Git adapter execution was run for this extension.
