# Project Rename Transaction v1

Status: locally implemented and bounded. Hosted promotion is not claimed.

Project Rename Transaction v1 is an explicit opt-in extension of
[Project Agent Transport v2](PROJECT-AGENT-TRANSPORT-V2.md). It changes the
reported protocol to `semaprax.agent-transport.v3` and adds exactly two
methods:

```sh
semapraxd --stdio --allow-project-rename \
  [--manifest-path semaprax.toml] \
  [--max-request-bytes N] [--max-response-bytes N]
```

The default command remains the byte-preserved, read-only v2 profile. A v2
session reports neither rename method, and attempts to call them receive
JSON-RPC `-32601` without changing a source.

## Admitted transaction

One transaction changes the display name of one monomorphic function selected
by the bound Project manifest's `web_exports`. The function must have an
explicit stable `@id`; the request names that identity and the exact current
and replacement display names. The source path and patch bytes are derived by
the server from authenticated retained meaning. Requests cannot supply a root,
path, source buffer, patch file, evidence file, or output location.

Both methods require the exact `project_revision` and `workspace_revision`
returned by `workspace/open`:

| Method | Additional parameters | Result |
| --- | --- | --- |
| `rename/preview` | `target_id`, `from`, `to` | One `semaprax.project-rename-preview.v1` artifact and digest |
| `rename/apply` | `preview_digest` | One receipt binding the accepted preview and refreshed Project, Workspace, source, and Project-graph revisions |

Preview validates the selected declaration, explicit identity, export
admission, exact `from` name, replacement grammar, and namespace conflicts. It
derives one canonical Semantic Patch v1 buffer, runs the bounded Project-module
patch parse/edit/canonicalization/revision preflight, then runs one complete
candidate Project build over the full declared source set. Standalone
executable verification is deliberately deferred because an imported Project
module need not define `main`; the complete Project build is the semantic
admission gate. Candidate validation includes scalar Project admission,
provider linkage, entry/test closures, Project graph/context inputs, and Web
export admission. Preview is read-only and moves the sequential session from
`open` to `prepared` only after its complete bounded response is representable.

The preview binds base and candidate Project and Workspace revisions, the
single base/candidate source facts, target identity/names/path, derived patch
schema/digest/byte count, candidate Project graph digest, limits, nonclaims,
and a domain-separated `preview_digest`. Apply accepts only that retained
digest. A mismatch is `-32602` and leaves both the source and prepared plan
unchanged.

## A0 authority handoff

Apply does not materialize a proposal file. While the authenticated Project
snapshot is still live, the daemon:

1. rechecks every held Project input;
2. acquires the ordinary single-file A0 sibling lock for the server-derived
   source path;
3. authenticates and retains the exact source identity, permissions, bytes,
   and revision;
4. parses and preflights the owned server-derived patch bytes against that
   retained snapshot; and
5. verifies the A0 base revision, candidate revision, and canonical candidate
   against the retained Project preview.

Only after this overlap is established does the daemon release the Project
snapshot's held handles. The owned, non-clone A0 authority is consumed once by
the unchanged create-new staging, two final source/stage identity-and-byte
checks, and atomic rename core. This is continuity between two existing
authorities, not a request-selected path capability or a new multi-file commit
protocol.

## Response preflight, reload, and uncertainty

Before acquiring commit authority, apply renders and bounds both its success
receipt and fixed uncertainty response. If the success response cannot fit,
the daemon returns terminal `-32001` before any write.

After A0 returns, the daemon reloads the complete bound Project from the
startup manifest. Success is reported only when the committed source revision
matches the preview and the reloaded Project revision, Workspace revision,
source fact, and complete Project graph match the candidate. The session then
returns to `open` with the new revisions, so graph, context, and test requests
operate on refreshed retained state.

If A0 rejects and reload proves the exact base still exists, the daemon reports
the failure and safely returns to `open`. Any other post-boundary combination
is `SPX-J110`: publication outcome is uncertain, the session becomes terminal,
and the caller must inspect the bound Project. The daemon never guesses,
retries, rolls back, or deletes an uncertain source.

Notifications remain silent and execute no semantic or mutation method.
Requests are sequential; `prepared` admits only the matching apply transaction
plus lifecycle inspection/shutdown behavior defined by the closed router.

## Evidence and nonclaims

Focused local evidence is:

```sh
cargo test --locked -p semaprax --all-features \
  --test project_agent_transport_rename_v1 -- --test-threads=1
```

It covers v2/v3 method isolation, a calculator `calculator.add` display rename
from `add` to `sum`, stable-ID preservation, refreshed revisions/graph/context/
test, unchanged stable-ID Web artifacts and Node consumer, stale subjects,
preview-digest mismatch, notification silence, response-cap termination, and
no-write failure inventories. The six black-box tests are complemented by four
session-boundary tests for exact success/uncertainty response minima, exact-base
recovery after commit rejection, correlated terminal `SPX-J110` after a
post-commit reload rejection, and same-byte target/foreign-source identity
drift on both sides of the A0 handoff. Planner/A0 units additionally prove that
the sealed plan admits a validated imported module without `main`, while the
general raw Patch A0 path remains standalone-strict. This is local evidence
only.

V3 provides no general source editing; multi-file transaction; import-alias,
type, member, case, automatic-ID, unexported-function, or identity rename;
client-supplied patch/evidence/source/path; managed Semantic Workspace
publication; build target; network/TLS/peer authentication; persistent cache;
incremental refresh; concurrency; request deduplication; exactly-once delivery;
recovery/rollback; provenance; approval; signature; or reusable authorization.
It does not change Project Manifest v1's scalar admission or any completion
matrix status.
