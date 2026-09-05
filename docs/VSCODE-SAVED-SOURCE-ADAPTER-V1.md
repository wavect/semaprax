# VS Code saved-source adapter v1

Status: focused local Extension Host evidence executed for exact subject `2888f84f123b7caa44aa6807388d98f851d4beaf`; an additive real-host candidate-task scenario is authored and unrun; extension remains experimental and unpublished.

Audience: editor users, extension integrators and compiler contributors.

The optional extension in [editors/vscode](../editors/vscode/README.md) connects
an explicitly started local editor session to the existing
[MCP stdio adapter](IMAGE-MCP-ADAPTER-V1.md). It provides stable-ID selection,
compiler-derived change discovery, typed-intention submission and read-only
source diffs for complete immutable candidates, plus ephemeral typed-hole
planning, fills and explicitly selected compiler repairs. It does not make editor buffers
canonical, install a compiler or publish source.

## Startup and authority

The user configures absolute compiler, manifest and existing host-policy paths
in machine settings, then runs `SEMAPRAX: Start Saved-Source Session`. The
extension requires a trusted workspace and rejects workspace-level path
overrides. These choices follow the VS Code
[trust](https://code.visualstudio.com/api/extension-guides/workspace-trust) and
[configuration scope](https://code.visualstudio.com/api/references/contribution-points#contributes.configuration)
boundaries. Opening a repository does not start a compiler process.

Additively, saving a `.spx` file or manifest, or running `SEMAPRAX: Check
Project`, invokes the same user-selected binary directly as
`check <nearest semaprax.toml or file> --json` and shows its diagnostics in
the editor. The route is read-only and bounded (4 MiB of output, 30 seconds),
starts no session, is disabled by an empty `semaprax.compilerPath`, and can be
switched off with the machine setting `semaprax.checkOnSave`. The extension's
[README](../editors/vscode/README.md#check-on-save) owns the behavior.

Also additively, `SEMAPRAX: Go to Declaration by Stable ID`, `SEMAPRAX: Show
Callers of a Declaration`, `SEMAPRAX: Show Module Documentation`, `SEMAPRAX:
Show Ownership, Contracts, and Effects`, `SEMAPRAX: Inspect Agent Definition`,
and the declaration code lenses run the same binary's read-only
`query <file> --json`, `doc <file>`, `context`, and `agent inspect`
([Unified CLI v1](UNIFIED-CLI-V1.md),
[Documentation Projection v1](DOC-PROJECTION-V1.md)) over the saved active
file, with the same bounds and no session. The
[README](../editors/vscode/README.md#navigate-by-meaning) owns the behavior.

Startup invokes the selected executable directly with
`serve-workspace-mcp <manifest> <host-policy>` and no shell. The executable and
policy are host choices, not values inferred from source or tool responses.
The ordinary host-policy loader still decides what the server can do. The
extension intersects discovered tools with its own small allowlist; even a
broader host policy cannot enable editor build, test, approval or commit calls.
Prefer a candidate-only policy for ordinary changes. The optional diagnostic
workflow also requires diagnostics in the already selected host policy; the
editor does not edit that policy or enable missing methods.

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
leaves the prior candidate unchanged. Branching recovery and a complete graph
browser remain separate protocol surfaces.

## Diagnostic attempts and explicit repair

The ordinary Apply command remains fail-fast. A separate diagnostic-attempt
command submits the tracked typed-intention scratch through `candidate/attempt`
only when the host exposes the diagnostic lifecycle. An accepted outcome must
contain an ordinary candidate handle and no attempt. A rejected outcome must
contain a rejected attempt summary and no candidate; the selected predecessor
remains unchanged. A rejected record cannot become a candidate, typed-hole draft
or source-review subject.

The controller binds the attempt to the exact image, held Project revision,
predecessor candidate and predecessor Project revision. It tracks the original
candidate base separately: that base is not necessarily the predecessor's
current Project revision after earlier changes. Closed response variants,
revision digests, byte/count bounds and no-authority fields are checked before
adopting state.

Diagnostic details are shown as read-only report text. `attempt/query` chunks
must preserve their exact attempt selector, byte offsets, total size and UTF-8
progress. The controller bounds the assembled report to 2 MiB and verifies the
attempt digest over its exact raw bytes, including the canonical final LF.
Parsed values are not reserialized to verify this digest. Diagnostic paths and
spans describe rejected constructor input or uncommitted candidate source;
the editor does not open those paths or jump to them as verified source spans.

Repair discovery displays only the compiler's current bounded catalogue, its
availability reason and the advertised proposal metadata. The user explicitly
chooses a proposal. Application sends only the retained `attempt_revision` and
exact `repair_id`, alongside the image revision. The editor never reconstructs
or submits a proposal's displayed change or `semantic_change_intent`. Thus
displayed integers outside JavaScript's safe range cannot silently become
rounded replacement intentions. The returned candidate must match the selected
proposal's advertised validated candidate revision and original base binding.
Ordinary compiler re-derivation and full candidate admission remain authoritative.

Only one attempt is selected locally. Replacing it retires the old server
attempt only after the new outcome has been validated; repeated identical
attempt handles are not accidentally discarded. Applying a repair validates the
returned candidate before retiring the rejected attempt. Failed retirement
ends the session and reports that the preceding mutation may already have
succeeded; it does not claim rollback or automatically retry. Ordinary semantic
rejection preserves the current selection for correction. Source drift,
malformed responses and transport uncertainty invalidate editor state.

Active typed-hole drafts block attempt submission and repair application.
Candidate replacement, target or draft transitions invalidate prior repair
views and selections. An asynchronous picker cannot apply a selection after
its image, candidate, attempt or editor generation changes. A repaired candidate
can use the existing source-diff preview; repair selection grants no source
publication, approval, build, test or external execution authority.

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

`Choose Checked Hole Fill for Scratch` adds an optional entry point to that
same scratch workflow. It first requires the already discovered
`hole/fill-suggestions` method, obtains a current compact summary, and requests
the [bounded source-checked report](PROJECT-HOLE-FILL-SUGGESTIONS-V1.md).
The controller binds its exact draft, hole, context, last-valid revision and
expected type to its own retained summary. It checks the closed report fields,
safe counters, 64 KiB limit, fixed no-authority/nonclaim fields and finite
place/direct-call expression grammar before returning a defensive copy.
It never hashes reparsed full context or treats lexical scope as liveness.

The command presents the admitted proposals in compiler order with considered
counts and finite-search exhaustion status. Labels may be shortened for display;
the exact selected expression goes into the scratch document. No proposal is
automatically chosen, filled or completed. Its `preview_draft_revision` remains
descriptive text and never becomes the editor's draft or a query handle. Editing
the scratch is allowed; the ordinary explicit Fill command always revalidates
the submitted expression rather than trusting the earlier preview.

Source/dirty-buffer, epoch, controller, draft and selected-hole checks follow
every asynchronous query, choice and scratch-opening step. Cancelling the chooser
creates no scratch or server mutation. Stale views cannot create a current scratch
binding, and a failed final display removes any provisional binding. Missing
host support fails before RPC without enabling a method or broadening policy.
The editor allowlist does not acquire parallel batching, execution or publication.

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

`editors/vscode/test/repairs.test.js` adds authored, unrun controller cases for
accepted/rejected variants, exact predecessor and proposal bindings, raw report
hashes, explicit selection, stale state and uncertain retirement. Its mocked
responses establish intended adapter behavior, not executed compiler or editor
conformance.

Real editor-host integration, accessibility and platform evidence, richer typed
constructor UI, broader diagnostic workflows, durable candidate recovery and
task-level measurements remain open. The additive candidate-test task controller
now supplies explicit Run/Cancel commands through VS Code cancellable progress,
but real Extension Host execution of that path remains open. This is an
optional local adapter, not a marketplace release or full programme completion.

`editors/vscode/test/holes-suggestions.test.js` adds authored, unrun mock
controller cases for summary binding, malformed or excessive proposals,
ordinary semantic rejection, busy/delayed requests and no implicit fill or
preview adoption. These mocks do not exercise the actual VS Code quick pick,
document-opening lifecycle or compiler source replay; those integration gates
remain outstanding.

## Focused Extension Host evidence

[VS Code Host Execution Evidence v1](GRAPH-OPERATIONAL-VSCODE-HOST-EXECUTION-EVIDENCE-V1.md) owns a separate, exact-subject local scenario using a selected provisioned Visual Studio Code Extension Host and freshly built compiler. Its test-only seam is enabled only by `ExtensionMode.Test` and contributes no production command or authority. The exact local subject `2888f84f123b7caa44aa6807388d98f851d4beaf` passed the 50-case standalone controller selection plus the actual Extension Host/compiler typed-rename, verified-diff, and dirty-buffer invalidation scenario. [VS Code Host Execution Evidence v2](GRAPH-OPERATIONAL-VSCODE-HOST-EXECUTION-EVIDENCE-V2.md) adds an authored, unrun exact scenario for startup-selected interpreter limits, real MCP task cancellation, and pending-task dirty-buffer invalidation. Packaging, manual UI, hosted/cross-platform, typed-hole and diagnostic-repair host execution remain open.

## Candidate test task control

When startup discovery already contains the four host-selected
`candidate__test-task-*` tools, the extension exposes Run Candidate Tests and
Cancel Candidate Tests. `tasks.js` validates exact image/project/candidate/task
bindings, all-false authority, the six blind spots, closed states, report digest,
and bounded 512 KiB pages before adopting any result. One controller task exists
per image. Dirty buffers, file/config changes, refresh, stop, epoch change, and
VS Code progress cancellation invalidate local adoption; explicit cancellation
still reaches the compiler task before the editor retires it. Late results cannot
replace current UI state.

The extension cannot enable `candidate_test`, change its policy, schedule build
or commit, or treat a passing reference report as target/runtime evidence. This
is the explicit Semaprax lifecycle from
[Candidate Test Tasks v1](IMAGE-CANDIDATE-TEST-TASKS-V1.md), not the optional MCP
Tasks capability and not `notifications/cancelled`. The focused Node controller
suite is local evidence. The v2 real Extension Host scenario now authors both
explicit cancellation and dirty-buffer invalidation against an actual compiler
child, but it remains unrun. Manual UI, accessibility, hosted/cross-platform,
and actual current-subject Extension Host cancellation evidence remain open.
