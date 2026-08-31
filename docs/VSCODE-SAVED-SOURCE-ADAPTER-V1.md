# VS Code saved-source adapter v1

Status: implementation and regression evidence authored, unrun; not published.

Audience: editor users, extension integrators and compiler contributors.

The optional extension in [editors/vscode](../editors/vscode/README.md) connects
an explicitly started local editor session to the existing
[MCP stdio adapter](IMAGE-MCP-ADAPTER-V1.md). It provides stable-ID selection,
compiler-derived change discovery, typed-intention submission and read-only
source diffs for complete immutable candidates. It does not make editor buffers
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
leaves the prior candidate unchanged. This adapter does not implement typed
holes, all diagnostic-repair interactions, branching recovery or a complete
graph browser; those protocol surfaces remain separate.

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

Only one editor command and one protocol request can be pending. Responses and
virtual views remain bound to the session generation; a source change during
an asynchronous operation invalidates its result. Transport errors, malformed
responses and timeouts end the client session without automatic retry. Stop or
restart releases retained review text. The extension does not promise process
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
constructor UI, holes and diagnostic workflows, asynchronous cancellation,
durable candidate recovery and task-level measurements remain open. This is an
optional local adapter, not a marketplace release or full programme completion.
