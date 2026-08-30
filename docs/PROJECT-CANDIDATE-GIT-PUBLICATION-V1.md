# Project Candidate Git Publication v1

Audience: host integrators and compiler contributors.
Status: authored bounded Unix/bare-SHA256 source-publication route; focused
regressions are authored and **not executed locally**. No passing platform or
completion-gate evidence is claimed.

This route writes actual canonical `.spx` blobs, trees and a commit into one
explicitly selected local Git repository, then publishes through one expected-old
branch-ref update. It does not update a checkout, index, raw Project source path,
managed Workspace `ACTIVE`, remote, or network service. Git-tree readers can
observe one complete commit; editors observing the original working tree see no
change. This is separate authority from candidate preparation, recovery capsules,
reports, test results, and the managed publication bridge.

## Explicit host interface

`CandidateGitProcessAuthority::open(executable, repository, max_commands,
timeout_ms)` admits a host-controlled bare SHA256 repository on Unix. Both paths
must be absolute; the repository path must already be canonical. The explicitly
trusted Git executable may be a symlink, resolved to a held regular executable.
No executable, repository, branch, author, timestamp or message comes from the
candidate or recovery capsule. Windows and other non-Unix process hosts fail
closed. SHA1 repositories are outside this first version.

`CandidateGitTarget::new(repository_identity, reference, expected_base_commit,
project_prefix)` binds the exact adapter identity, a bounded `refs/heads/` name,
a 64-lowercase-hex Git SHA256 object ID, and an optional relative Project prefix.
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
   Build binary SHA256 trees and one explicit commit with the selected original
   commit as its sole parent. Unrelated histories and signatures are not copied
   into the new commit. Git tree SHA256 object names occupy 32 binary bytes.
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
releases its lease; the lock file is never deleted. This coordinates cooperating
publication hosts. Ordinary concurrent Git writers remain subject to Git's ref
CAS. The repository and executable are **host-controlled inputs**: neither this
lease nor repeated pathname checks protects against a malicious same-UID process
racing namespace/content mutations between checks. Such adversarial shared
storage is outside this adapter's authority model.

Admission accepts only a minimal format-1 bare SHA256 config: required
`core.repositoryformatversion=1`, `core.bare=true`,
`extensions.objectformat=sha256`, plus optional boolean `core.filemode` and
`core.logallrefupdates`. Unknown/duplicate settings, includes, alternate object
stores, common-directory indirection, attached worktrees, shallow history and
grafts are rejected. Before each process call, held repository/config/executable
identities are checked, config bytes are compared, and a bounded recursive scan
rejects nested symlinks, special files and multiply linked regular files anywhere
in the repository, including refs, reflogs, packs and loose objects. Git executable
size and modification/change timestamps are held and rechecked. These checks do
not confer trust on an untrusted binary selected by a host.

Every process has an empty inherited environment and explicit null global/system
config, no replacement objects, no lazy fetching, no terminal prompts and no
optional locks. A fixed command-line `protocol.allow=never` forbids transport;
there are no network commands or request-selected command names. Hashing uses
`--no-filters`. Fixed `core.hooksPath=/dev/null` disables hooks, including
`reference-transaction`, which otherwise executes during ref operations.
[Git hooks](https://git-scm.com/docs/githooks). Shells, aliases, signing, filters,
credential helpers and inherited SSH/secret variables are not used.

Only `symbolic-ref`, `show-ref`, raw `cat-file`, raw `hash-object` and
`update-ref` are invoked. Reads/writes use bounded pipes, a fresh process group,
and one host-selected total adapter deadline (1–60,000 ms) and command budget
(1–4,096). Worker pipes are capped; timeout/output failure kills the process group
and reports failure. The deadline begins at adapter opening and includes time
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

`tests/project_candidate_git_publication_v1.rs` authors a real bare-SHA256 Git
publication, unrelated entry/mode preservation, unchanged raw sources, disabled
ref hooks, stale-ref/original-blob rejection, unsafe-config rejection and nested
object-store symlink rejection. `SEMAPRAX_TEST_GIT` can select the trusted fixture
binary; otherwise the Unix fixture selects `/usr/bin/git`. These tests have not
been run in this development batch. No compiler, test suite or Git adapter process
was executed as local validation.
