# VS Code saved-source adapter v1

Status: implementation and regression evidence authored, unrun; not published.

Audience: editor users, extension integrators and compiler contributors.

The optional extension in [editors/vscode](../editors/vscode/README.md) connects
an explicitly started local editor session to the existing
[MCP stdio adapter](IMAGE-MCP-ADAPTER-V1.md). It provides stable-ID selection,
compiler-derived change discovery, typed-intention submission and read-only
source diffs for complete immutable candidates, plus ephemeral typed-hole
planning and fills. It does not make editor buffers
canonical, install a compiler or publish source.

## Startup and authority

The user configures absolute compiler, manifest and existing host-policy paths
in machine settings, then runs `SEMAPRAX: Start Saved-Source Session`. The
extension requires a trusted workspace and rejects workspace-level path
overrides. These choices follow the VS Code
[trust](https://code.visualstudio.com/api/extension-guides/workspace-trust) and
[configuration scope](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration)
boundaries. Opening a repository does not start a compiler process.

Startup invokes the selected executable directly with
`serve-workspace-mcp <manifest> <host-policy>` and no shell. The executable and
policy are host choices, not values inferred from source or tool responses.
The ordinary host-policy loader still decides what the server can do. The
extension intersects discovered tools with its own small allowlist; even a
broader host policy cannot enable editor build, test, approval or commit calls.
Prefer a candidate-only policy for this workflow.

The extension performs the pinned MCP initialize/initialized exchange and pages
through the selected tool inventory. It validates the outer response ID and the
complete inner v5 result envelope, including the fixed inner ID `0`, protocol
versions and revision digest fields. Compiler semantic errors remain errors;
they cannot become candidate handles or permission to retry publication.

## Workflow

1. Start the saved-source session and open an immutable candidate.
2. Select a declaration by its explicit stable ID.
3. Inspect its compiler-derived change catalogue.
4. Create a typed-intention JSON scratch document for a listed operation.
5. Fill the required fields and apply that scratch document to the selected
   candidate. Ordinary compiler admission determines whether it is valid.
6. Preview the candidate source diff and select a changed file.
7. When saved source changes, explicitly preview and refresh the held image,
   then open a new candidate selection.

The JSON scratch document is a request draft, not `.spx` source or checked HIR.
The extension accepts no arbitrary source patch and does not claim that a
catalogue entry makes every payload valid. Existing candidate failure behavior
leaves the prior candidate unchanged. Diagnostic-repair interactions, branching
recovery and a complete graph browser remain separate protocol surfaces.

## Typed-hole workflow

The editor can plan body, body-expression, and contract-expression holes on one
selected candidate. Expression choices come from the compiler's bound body or
contract catalogue; display names and editor source spans are not selectors.
The user chooses an explicit hole ID and can plan up to sixteen pending holes.
These are server-owned immutable drafts, never placeholders written into `.spx`.

More holes can be planned after successful fills. Before the first hole, the
editor uses the original candidate's expression catalogue. Once a draft exists,
[draft expression discovery](PROJECT-DRAFT-EXPRESSION-CATALOG-V1.md) selects
current identities and lexical scopes from its private last-valid state. Each
choice inventory binds the current draft and is invalidated by every successful
open or fill. A host without this method fails explicitly; the editor never
falls back to stale original-candidate selections. Existing pending holes still
use the compiler's ordinary selector rebinding and overlap checks.

Select a pending hole, inspect its compact summary, and choose scope, calls,
obligations or constructors for a bounded facet page. Additional pages require
an explicit editor action; the adapter does not automatically drain large
inventories. Responses must match the selected image, draft, hole, context and
facet reference, including exact progress and count bounds. Full context is
available separately for aggregate and builtin descriptors and prior proof
details. All these views are descriptive and read-only. They do not prove a
fill valid, grant runtime capabilities, or authorize source publication.
The constructor-schema command separately displays the four compiler-owned
structural documents, including the recursive expression grammar. It never
fetches references or substitutes a client schema check for compiler admission.
Full contexts and schemas can contain integers beyond JavaScript's exact range;
their displayed numeric values are descriptive, not an exact proof carrier.
The controller does not hash reparsed full-context JSON. Compact facet reference
bindings use only their exact string fields and canonical hash contract.

The fill command accepts only an extension-created JSON scratch document bound
to the current image, source candidate, draft revision and hole. It contains a
typed expression, not an ordinary intention or source fragment. Any successful
draft change invalidates previous fill scratches and navigation references;
the user requests fresh context and a fresh fill scratch for the next hole.
Malformed or rounded numeric JSON is rejected before submission. Ordinary
compiler rejection leaves the draft and scratch available for correction.
At most 64 fill-scratch references are tracked, and closing a document releases
its reference. Successful draft changes invalidate old hole view text as well
as scratch bindings. Only the most recently requested expression catalogue is
retained for a new expression selection.

An active draft blocks ordinary candidate replacement, intention submission and
source-diff preview, including after its final hole is filled. The user must
explicitly complete it. Only the validated `hole/complete` response becomes the
selected candidate and enables the existing independently verified source diff.
Discarding a draft returns to the originally selected candidate without saving
or publishing any source. Neither action grants build, test, archive-restore,
approval or commit authority.

The controller retires its superseded server draft only after a new open/fill
result has been validated and adopted. Completion similarly releases the final
draft after receiving the candidate handle. This keeps at most two of this
controller's draft handles live during a transition, within the server's
sixteen-draft registry. Failed fills never discard the current draft. A failed
retirement ends the session: a successful preceding operation is not described
as rolled back, and no mutation or retirement is automatically retried.

## Read-only review

Source diffs use the [closed source-review report](PROJECT-CANDIDATE-SOURCE-REVIEW-V1.md).
The adapter reassembles bounded UTF-8 chunks with exact image/candidate bindings,
checks the complete canonical report digest, then verifies each source and diff
digest. Relative paths are validated as report metadata; they are never opened
or written to obtain the review text.

The base and candidate texts enter a bounded in-memory virtual-document store
under opaque extension-owned URIs. VS Code's
[text-document content provider](https://code.visualstudio.com/api/extension-guides/virtual-documents)
renders them as read-only documents. The diff view compares those two retained
texts. There is no `WorkspaceEdit`, real-file replacement, source save command,
webview, shell command embedded in a report or source-publication action.
Reviewing a diff does not create an approval token.

## Drift and lifecycle

Dirty `.spx` or manifest documents block semantic commands. Editor and file
watcher events invalidate the current selection and visible review state;
watchers are hints only. Every semantic server invocation still performs its
ordinary held-source authentication. The extension never uploads an unsaved
buffer as authoritative source or silently refreshes a revision after drift.

Only one editor command and one protocol request can be pending. The hole
controller also serializes its own operations. Responses and
virtual views remain bound to the session generation; a source change during
an asynchronous operation invalidates its result. Transport errors, malformed
responses and timeouts end the client session without automatic retry. Stop or
restart releases retained review text, draft selection, fill scratches and
facet references. The extension does not promise process
durability, exactly-once execution or recovery of a response lost after a
candidate operation.

The adapter retains the MCP 128 KiB request and 8 MiB response bounds and the
underlying 64 KiB v5 request bound. Source-review reports are bounded to 16 MiB
and sixteen changed files. The virtual-document store holds at most 64
references and 32 MiB of text per session. These are local protocol/storage
limits, not a global memory or latency guarantee. A timeout is not proof that the server did
not complete the requested candidate operation.

## Evidence and remaining work

Compiler/library and v5 transport regressions are authored alongside pure Node
protocol/report-validator cases in the extension. None were executed. No
compiler, Node test runner, generated client, VS Code extension host, package
installation or local quality gate was run for this change.

Real editor-host integration, accessibility and platform evidence, richer typed
constructor UI, diagnostic workflows, asynchronous cancellation,
durable candidate recovery and task-level measurements remain open. This is an
optional local adapter, not a marketplace release or full programme completion.
