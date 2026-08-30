# Candidate Git publication CLI v1

Status: authored, unrun; no actual candidate publication or target execution evidence.

Audience: local host operators and agent integration authors.

The explicit command is:

```text
semaprax project-candidate-git-publish <manifest> <capsule.json> <approved-candidate-digest> <host-policy.json>
```

The host selects this command and policy independently of the candidate. The
capsule is an ordinary complete-candidate recovery capsule; it cannot select the
repository, reference, author, executable or approval. The digest operand must
match the restored candidate exactly. Unresolved drafts have no recovery capsule
or publication route.

The policy is a regular, bounded JSON file with exactly these fields:

```json
{
  "schema": "semaprax.candidate-git-host-policy.v1",
  "git_executable": "/absolute/path/to/git",
  "repository": "/absolute/canonical/path/to/bare-repository",
  "reference": "refs/heads/approved",
  "base_commit": "<40 SHA1 or 64 SHA256 lowercase hexadecimal digits of the exact commit>",
  "project_prefix": "",
  "author_name": "Project Host",
  "author_email": "host@example.invalid",
  "unix_seconds": 0,
  "message": "Apply the explicitly approved semantic candidate.\n",
  "max_commands": 512,
  "timeout_ms": 60000
}
```

`project_prefix` selects the existing Project directory within the Git tree;
empty means the tree root. Author and committer metadata use the supplied name,
email and UTC timestamp. The message requires a final LF. No ambient user
identity, clock, editor or signing service is consulted. Policy input is limited
to 64 KiB, commands to 1–4096 and adapter lifetime to 1–60000 milliseconds.

After restoring the candidate against independently authenticated source, the
command acquires the explicit Git host adapter and invokes
[Candidate Git Publication v1](PROJECT-CANDIDATE-GIT-PUBLICATION-V1.md). That
authority authenticates the expected Git source tree and full candidate again,
writes immutable Git objects and attempts one old-commit-checked reference
update. The adapter supports its documented Unix bare SHA1 and SHA256 repository
profiles. The exact base OID width must match the independently validated host
repository format; no new policy field selects an algorithm. Ordinary active
checkouts, unsupported hosts and broader repository config remain rejected.

Success prints a receipt naming the previous/new commits, tree, exact candidate
and changed canonical source paths. This is a local Git branch publication,
not a remote push or raw working-tree rewrite. SHA256 receipt bytes retain their
existing format. SHA1 additionally reports a SHA256 observed/prepared-object
content binding and rereads staged objects for exact byte equality before the
ref pivot; it does not claim SHA1 collision detection or collision resistance.
Existing managed `ACTIVE` state
is untouched. Failed object staging can leave unreachable Git objects. Errors
after the reference update is attempted explicitly report possible publication;
the caller must inspect the reported ref/commit instead of assuming rollback.

Policy/capsule input validation precedes process authority. The read-only
candidate restore finishes before publication begins, so a generic outer input
recheck cannot mask the publication API's explicit post-update uncertainty.
No image or candidate NDJSON request can select this command or widen its own
capability. All tests and compiler/executable gates remain unrun in this work.
