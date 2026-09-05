# SEMAPRAX saved-source editor adapter

Experimental. A focused local Visual Studio Code Extension Host run passed for
exact subject `2888f84f123b7caa44aa6807388d98f851d4beaf`; the standalone
50-case controller suite remains separate. An additive 57-case controller and
real-host candidate-task scenario is authored but unrun. This zero-build CommonJS extension uses only
VS Code APIs and Node built-ins. No npm dependencies, bundling, telemetry,
webviews, language server, automatic process startup, or publication command.
It is not a packaged or marketplace release. The exact local execution claim is bounded by the evidence contract below.

## Syntax highlighting

Opening a `.spx` file gives SEMAPRAX highlighting, `//` comment toggling,
bracket matching, and auto-closing pairs without starting a session. This is
the declarative `languages` and `grammars` contribution in `package.json`
backed by `syntaxes/semaprax.tmLanguage.json` and
`language-configuration.json`; it runs no code and needs no compiler path.
The repository's documentation gate checks that the grammar names every
keyword the parser recognises, so the highlighting cannot silently lag the
language. The grammar itself provides no completion or navigation.

## Check on save

Saving a `.spx` file or `semaprax.toml` runs the compiler named by the
**user/machine** setting `semaprax.compilerPath` as
`check <subject> --json`, where the subject is the nearest `semaprax.toml`
walking up from the saved file, or the file alone when no manifest exists. Each
JSON diagnostic line becomes an editor diagnostic at its reported line and
column (a bare position is one character wide) with the message
`code: message` and the compiler's help on a following line; entries for files
the re-check no longer reports are cleared. `SEMAPRAX: Check Project` runs the
same check explicitly for the active file's project or the first workspace
folder, and names the setting to fill when `semaprax.compilerPath` is empty.
`semaprax.checkOnSave` (default `true`, machine scope) turns the save trigger
off. With an empty `semaprax.compilerPath` nothing runs. The binary is invoked
directly, never through a shell, never discovered, and never taken from
workspace settings; the child's combined output is capped at 4 MiB and its run
at 30 seconds, after which it is killed and the failure is written to the
`SEMAPRAX Check` output channel. `check` is read-only: this feature builds
nothing, publishes nothing, writes no file, and starts no saved-source session.
A run is published only when the adapter can classify it. `check` exits 0
after printing exactly one `{"status":"verified", …}` record and no error, and
exits 1 after printing at least one error diagnostic and no verified record.
Any other combination — a killed or unstartable child, a foreign exit status,
a line that is neither a diagnostic nor the verified record, an error with
status 0, or a verified record with status 1 — is a check failure: the
previously published diagnostics stay exactly as they were, the reason is
written to the `SEMAPRAX Check` output channel, and `SEMAPRAX: Check Project`
reports the failure and that the visible diagnostics may be stale. A check that
a newer check of the same subject superseded publishes nothing either. The
adapter never reports a clean project from output it could not read.
`test/diagnostics.test.js` covers manifest discovery, malformed-line skipping,
severity/range mapping, appended help, stale clearing, the exit-status and
output classification matrix, retention of the previous ledger across every
failure, and the byte and time bounds against a scripted child.

## Navigate by meaning

With `semaprax.compilerPath` set, three commands and one code-lens provider
read the saved active `.spx` file through the compiler's read-only
`query <file> --json` and `doc <file>` routes; nothing runs on a dirty buffer.
`SEMAPRAX: Go to Declaration by Stable ID` lists every declaration of the
module (name, kind, `@id`, canonical header) and moves the cursor to the chosen
declaration's name token, using the one-based line and column the compiler
reports. `SEMAPRAX: Show Callers of a Declaration` asks for a function or
method, then lists the declarations whose bodies call it, from the compiler's
persistent call index rather than a text search, and jumps to the chosen
caller. `SEMAPRAX: Show Module Documentation` opens the Markdown page
`semaprax doc` renders beside the source. Code lenses above each declaration
show its `@id` (or that the identity is automatic), its `uses { … }` effects
when it declares any, and its `requires`/`ensures` counts when it declares
contracts; `semaprax.codeLens` (default `true`, machine scope) turns them off.
`SEMAPRAX: Show Ownership, Contracts, and Effects` asks for a function or
method and opens the compiler's bounded `context` document for it (depth one,
the `contracts`, `ownership`, and `effects` facets, an 8 KiB budget) beside the
source, so parameter and result ownership modes, contract clauses, and effect
sets are read from the checked graph rather than inferred from text.
`SEMAPRAX: Inspect Agent Definition` runs `agent inspect` on the saved active
AgentDefinition v1 `.json` file and opens its AgentGraph v1 beside it.
`SEMAPRAX: Safe Rename by Stable ID` asks for a function or method and a new
lowercase name, authors the one-line semantic patch `base <revision>` /
`rename <id> to <name>` in a temporary file, shows the compiler's `impact`
analysis (how many declarations change and which consumers), and only on
confirmation lets the compiler's replay-checked `patch` route rewrite the
saved file; the stable identity never changes and the temporary patch is
removed afterwards. `SEMAPRAX: Show Cleanup Plan` opens the canonical cleanup
plan the module graph records for a chosen function, exactly as `graph`
emits it, so cleanup order is read rather than inferred. `SEMAPRAX: Run Agent
Transcript (Trace/Evidence)` takes the saved active AgentDefinition v1 file,
asks for a task and a transcript document, and opens the scripted run's
trace, evidence, or receipt from `agent run`; the run has no provider, tool,
or network authority.
Every run is bounded exactly like check-on-save (4 MiB, 30 seconds, direct
spawn without a shell, never workspace settings) and its failure is written to
the `SEMAPRAX Check` output channel. `test/navigation.test.js` covers the
argument vectors, result validation, source-ordered items, zero-based ranges,
lens titles, and the byte and time bounds against a scripted child.

## Saved-source session

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
The existing host-policy v1–v7 parser remains authoritative. Prefer a policy with
candidate preparation enabled and builds and Git commit disabled. Candidate
interpreter tests are unavailable unless startup policy selects fixed test limits. Enable
diagnostics in the host policy only if you want the optional attempt workflow
below. The adapter cannot widen policy and its own fixed allowlist excludes
builds, direct synchronous tests, commit approval, source publication and archive
restoration even if the supplied policy grants them.

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

**Run Candidate Interpreter Tests** uses Semaprax's explicit
`candidate/test-task-*` methods when all four are in the startup-selected tool
catalogue. These are Semaprax tools, not MCP standard task augmentation. The
start response is queued behind a one-shot gate; polling releases the bounded
interpreter worker. **Cancel Candidate Interpreter Tests** and the cancellable
VS Code progress notification request cooperative cancellation. Cancellation is
sticky, but completion may win if it was already terminal. Source drift, refresh,
finish or stop invalidates the editor handle, requests cancellation and discards
late results. A completed report is accepted only after exact revision, authority,
blind-spot, schema, pagination and digest checks. It claims no native or Wasm
runtime, deployment, generated-artifact, external API, runtime-environment or
external-consumer coverage. Builds and commits remain non-cancellable and absent.

For diagnostic recovery, **Try Active Typed Intent with Diagnostics** preserves
a rejected attempt separately from the valid candidate. **Show Rejected Attempt
Summary** and **Show Retained Attempt Diagnostics** inspect it; the latter
verifies the bounded report's exact bytes and displays diagnostic locations as
descriptions, never source navigation. **Show Compiler-Admitted Repair Catalog**
displays available proposals. **Select and Apply Exact Diagnostic Repair** asks
you to select a proposal, then sends only its exact repair ID and attempt
revision. Displayed intentions and potentially rounded numbers are never
resubmitted as repairs. The accepted candidate can use the existing source diff.
**Discard Diagnostic Attempt** releases the attempt without changing source.
These commands require the existing host policy to expose diagnostics; ordinary
Apply remains fail-fast. There is no automatic repair, policy change or retry.

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
compiler catalogue. Once a draft exists, choices come from that draft's current
last-valid state, so more holes can be opened after earlier fills. **Select Pending
Hole**, **Show Descriptive Hole Summary**, **Show Hole Facet Page or Next Page**
and **Show Full Hole Context (Unbundled)** inspect the current draft without
exposing it as valid source. Facet pages are
expanded explicitly and remain bound to that exact hole context. **Show Typed
Hole Constructor Schemas** displays the compiler's recursive expression grammar
without fetching schema references or claiming semantic admission.

**New Hole Fill Scratch** creates a typed-expression JSON document bound to the
selected draft revision and hole. **Choose Checked Hole Fill for Scratch** can
instead request bounded compiler suggestions and copy one explicitly selected
expression into the same kind of scratch. The command requires the host's
`hole/fill-suggestions` method. It shows accepted/considered counts and whether
the finite search was exhausted; empty results do not prove no valid fill exists.
Suggestions passed source replay, not tests or proof of the desired behavior.
Selecting one never adopts its preview digest or fills the hole automatically.
You can inspect or edit it before **Fill Selected Hole from Active Scratch**
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

Only one protocol request can be pending. The Cancel command can mark the active
test controller while its current request is pending; the controller sends the
explicit cancellation request sequentially. Requests are capped at 128 KiB outer
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
context/reference binding, failed fills and explicit completion. Suggestion
controller cases cover exact summary/report bindings, bounded expression
grammar, stale and asynchronous failures, and no implicit preview adoption. Repair cases
use schema-shaped mock responses to cover exact selectors, bound raw diagnostic
reports, malformed responses and failed handle retirement. Task-controller cases
cover exact queued and terminal states, sticky cancellation, bounded report
chunks, digest binding, authority and blind spots. Verification can use
`node --test test/*.test.js`; no VS Code or compiler process is started by those tests.
The separate `scripts/graph-operational-vscode-host-evidence.py` v2 runner
provisions an actual Extension Host plus compiler task-cancellation scenario and
must be reported only after it succeeds on its exact clean subject.

Implementation references: [VS Code workspace trust](https://code.visualstudio.com/api/extension-guides/workspace-trust),
[virtual documents](https://code.visualstudio.com/api/extension-guides/virtual-documents),
and [configuration contributions](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration).
