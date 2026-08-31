# SEMAPRAX saved-source editor adapter

Experimental, authored and unrun. This zero-build CommonJS extension uses only
VS Code APIs and Node built-ins. No npm dependencies, bundling, telemetry,
webviews, language server, automatic process startup, or publication command.
It is not a packaged or marketplace release, and no editor execution is claimed.

Load this directory as a development extension using VS Code's extension
development host. Configure these **user/machine settings**, all absolute paths:

```json
{
  "semaprax.compilerPath": "/absolute/path/to/semaprax",
  "semaprax.manifestPath": "/absolute/project/semaprax.toml",
  "semaprax.hostPolicyPath": "/absolute/path/to/host-policy.json"
}
```

Workspace and folder overrides are rejected even if supplied manually. A trusted
local filesystem workspace and saved source/manifest buffers are required.
The explicit Start command invokes the selected binary directly, without a shell:
`serve-workspace-mcp <manifest> <host-policy>`. Nothing downloads or builds it.
The existing host-policy v1–v6 parser remains authoritative. Prefer a policy with
candidate preparation enabled and builds, tests, diagnostics and Git commit
disabled. The adapter cannot widen policy and its own fixed allowlist excludes
builds, tests, commit approval, source publication and archive restoration even
if the supplied policy grants them.

Use the command palette in this order:

1. **Start Saved-Source Session** negotiates MCP 2025-11-25, reads the paginated
   host-selected tool catalog and opens the held workspace image.
2. **Open Candidate**, then **Select Stable Target ID**. IDs are explicit inputs;
   the compiler's target catalog checks them. No display-name guessing occurs.
3. **Show Target Change Catalog**, or **New Typed Intent Scratch**. The latter
   shows the selected constructor descriptor and opens an untitled JSON buffer
   containing `kind` and `target`. Fill its required fields using the catalog.
4. **Apply Active Typed Intent** submits only that tracked scratch buffer to the
   exact selected candidate/target. The compiler independently checks and replays
   the complete intention; the extension is not a semantic verifier. Integers
   beyond JavaScript's safe range reject instead of being silently rounded.
5. **Preview Candidate Source Diff** reconstructs the bounded source-review
   report, verifies its canonical report digest and each source/diff digest,
   then displays the selected base/candidate text through read-only virtual
   documents. It performs no `WorkspaceEdit`, filesystem write or arbitrary path
   read. Source paths are validated labels, not filesystem access instructions.

Source or manifest edits invalidate candidate selection and visible previews.
Unsaved buffers must first be saved or reverted. **Preview and Explicitly Refresh
Saved Source** asks the server to authenticate a new snapshot, then requires an
explicit confirmation before replacing the held image. Open a new candidate
afterward. The adapter does not silently rebase old candidates. Watcher events
are only invalidation hints; every semantic call still uses exact server-bound
image/candidate revisions and server source authentication. Manifest configuration
changes that the host refuses require a new explicitly configured session.

For incomplete work, **Open Typed Hole** plans a body, body-expression or
contract-expression replacement. Choose expression identities from the
compiler catalogue and plan all holes before filling any. **Select Pending
Hole**, **Show Descriptive Hole Summary**, **Show Hole Facet Page or Next Page**
and **Show Full Hole Context (Unbundled)** inspect the current draft without
exposing it as valid source. Facet pages are
expanded explicitly and remain bound to that exact hole context. **Show Typed
Hole Constructor Schemas** displays the compiler's recursive expression grammar
without fetching schema references or claiming semantic admission.

**New Hole Fill Scratch** creates a typed-expression JSON document bound to the
selected draft revision and hole. **Fill Selected Hole from Active Scratch**
submits it through ordinary compiler admission. Rejected fills preserve the draft; successful
changes invalidate older fill scratches and navigation references. Select the
next pending hole and create a fresh scratch. Only **Complete Ready Draft as
Candidate** releases a candidate for source review after every hole is filled. **Discard
Typed-Hole Draft** returns to the original candidate without source writes.

An active draft, even one ready to complete, blocks ordinary candidate changes
and source-diff preview. Stop, source drift and refresh clear its editor state.
Superseded in-memory draft handles are released after successful transitions;
a failed release terminates the session without pretending that the preceding
operation was rolled back. There is no automatic retry or publication.

Only one command/request can be pending. Requests are capped at 128 KiB outer
MCP and 64 KiB inner v5, responses at 8 MiB, source reviews at 16 MiB and 16 files,
and virtual-document references at 64 and 32 MiB total per session. Requests time out after 30
seconds. Framing, identity, protocol or digest failures terminate the session;
there is no automatic restart or mutation retry. Ordinary rejected intentions
preserve the last valid candidate for correction. Source authentication or stale
image errors invalidate candidate UI and require explicit refresh. Stop remains available while a
request is pending. Session process errors never imply that a source transaction
was approved or published. This adapter never calls those transactions.

The generated virtual diff is a review of an immutable in-memory candidate. It
does not save that candidate into canonical source. Use separately authorized
SEMAPRAX tools for publication; this extension intentionally has no such route.

Authored Node tests live in `test/`. They use the built-in `node:test` runner and
mock processes to cover protocol bounds, exact inner envelopes, rejected tool
authority, timeouts, duplicate keys, canonical source-review digests and hostile
chunk/path inputs. Additional authored cases cover typed-hole lifecycle,
context/reference binding, failed fills and explicit completion. They were
**not run** during implementation. Later explicit verification can use
`node --test test/*.test.js`; no VS Code or compiler process
is started by those tests.

Implementation references: [VS Code workspace trust](https://code.visualstudio.com/api/extension-guides/workspace-trust),
[virtual documents](https://code.visualstudio.com/api/extension-guides/virtual-documents),
and [configuration contributions](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration).
