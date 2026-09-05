# Universal Semantic Workflow CLI v1

Status: additive read-only one-shot adapter; five focused integration cases
passed locally on 2026-09-05.

Audience: compiler contributors, coding agents, CLI users, and reviewers of the
first simplified semantic workflow surface.

Universal Semantic Workflow CLI v1 exposes the existing Universal Semantic
Query v1 and Universal Semantic Transaction v1 cores through two familiar
commands. It is deliberately a thin adapter: every successful query prints the
exact canonical `SemanticQueryResult` bytes, and every change preview prints an
exact existing transaction artifact. It introduces no competing query,
candidate, impact, review, or result schema.

This badge is one-shot and read-only. Each invocation authenticates one Project,
derives one process-local semantic service, performs one operation, rechecks the
held Project inputs, prints the result, and exits. It is not the persistent
shared service, a transaction-input protocol, or a source commit command.

## Command surface

The universal query forms are selected only when the second positional operand
is one of the five operation names below:

```text
semaprax query <project> declarations [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>] [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--offset N] [--limit N] [--revision <digest>]
semaprax query <project> symbol <stable-id> [--revision <digest>]
semaprax query <project> context <declaration|capability> <target> [--direction <forward|reverse|both>] [--depth N] [--max-bytes N] [--max-nodes N] [--revision <digest>]
semaprax query <project> impact <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N] [--revision <digest>]
semaprax query <project> available-operations <stable-id> [--revision <digest>]
```

`<project>` is a Project directory or `semaprax.toml`. If `--revision` is
omitted, the adapter binds the query to the canonical workspace revision it
just derived from the authenticated Project. If supplied, it is passed to the
core as the exact expected revision; stale or malformed values retain Universal
Semantic Query diagnostics. Paging, filters, analysis options, operation
eligibility, limits, canonical rendering, digests, and result schemas remain
owned by Universal Semantic Query v1 and its delegated cores.

The existing declaration-search form remains unchanged. In particular,
`semaprax query <file|project> [legacy filters] [--json]` retains its previous
text/JSON selection and bytes. A source file is never interpreted as the
Project-only universal form.

The first change form is:

```text
semaprax change preview <project> rename-display-name <stable-id> <new-name> [--revision <digest>] [--evidence]
```

It admits only the operation already owned by Universal Semantic Transaction
v1. The adapter reads the selected function's current display name from the
same authenticated Project generation and uses it as the exact old-value
precondition. It does not let the request select source paths, replacement
source bytes, validation requirements, invariants, or authority. The optional
revision is the transaction's expected canonical workspace revision; omission
selects the freshly derived current revision.

Without `--evidence`, successful preview prints the exact bytes returned by
`SemanticTransactionArtifacts::result()`. With `--evidence`, it prints the
exact bytes returned by `SemanticTransactionArtifacts::evidence()`. Both
already contain the deterministic candidate and source-review projection owned
by the transaction kernel. The CLI adds no wrapper, receipt, digest, or claim.

## Authentication and lifetime

The adapter resolves the Project operand through the existing Project CLI
rules, then performs its complete operation inside `with_authenticated_project`.
That owner authenticates and retains the manifest, declared sources, and
dependency subjects and performs a final held-object recheck regardless of
whether the operation succeeds. The process-local `SemanticWorkspaceService`
is built only from the retained immutable `ProjectRevision`.

Queries execute one canonical `SemanticQuery` against that service. Change
preview constructs one canonical `SemanticTransaction`, then delegates to the
service's non-mutating transaction validation. No caller JSON is decoded as
HIR, no serialized graph becomes trusted state, and no result is emitted after
an undetected held-input drift.

The one-shot service and its semantic cache are dropped when the command exits.
The adapter does not retain snapshots, cursors, cache entries, transaction
history, or process state between invocations. `--revision` is a fail-closed
precondition, not a request to retrieve an old revision.

## Diagnostics, bounds, and output

Malformed CLI grammar, duplicate options, noncanonical integers, missing
operands, and unknown options exit with status 2 and the ordinary scoped-help
hint. Once a typed core operation is constructed, its existing diagnostics and
precedence are preserved:

- Universal Semantic Query v1 uses `SPX-G531`, `SPX-G532`, and `SPX-G533`;
- Universal Semantic Transaction v1 uses `SPX-G525`, `SPX-G526`, and
  `SPX-G527`; and
- Project authentication, parsing, verification, image, context, and impact
  failures retain their owning diagnostics.

The adapter creates no new wire schema or data-size limit. Exact core request,
result, payload, transaction, and artifact bounds apply before output. Success
is written once to standard output and already includes its required terminal
LF. Diagnostics go to standard error. The adapter does not accept stdin,
transaction files, response files, output paths, or request-selected byte
buffers in v1.

## Authority, compatibility, and nonclaims

Both commands are read-only. They do not write `.spx`, the manifest, a lockfile,
a cache, a receipt, a temporary proposal, a managed Workspace generation, or
Git state. Change preview validates and renders a candidate but never adopts,
commits, stages, applies, or publishes it. Query results and transaction
artifacts carry no approval or reusable authority.

The adapter owns no filesystem write, process execution, network, watcher,
socket, build, test, signing, deployment, payment, or publication authority.
Its Project reads are the ordinary explicitly selected local input authority,
not ambient home-directory or repository discovery.

This badge does not provide a daemon, persistent CLI session, shared service
transport, MCP, LSP, editor adapter, generated SDK, streaming, subscriptions,
multi-operation transaction algebra, natural-language intent, source commit,
managed `ACTIVE` pivot, Git commit, or approval workflow. It does not claim
that the five queries cover all semantic questions or that display rename is a
complete change language.

The change is additive. Existing legacy `query` behavior and all Project,
candidate, canonical workspace, semantic service, query, transaction, image,
and managed Workspace bytes remain owned by their existing versions. Project
Agent Transport v5 methods, discovery, schemas, clients, diagnostics, and bytes
are frozen and unchanged; these CLI commands are not transport aliases.

## Focused evidence

The integration evidence is authored in
`tests/workspace/universal_semantic_workflow_cli.rs` as a module of the existing
Workspace harness. Its five cases cover exact direct-core parity and zero
writes across all five query operations; declaration paging and exact legacy
Project-query preservation; explicit stale-revision and malformed query
grammar rejection; exact default/evidence change projections plus missing and
comment-bearing target rejection; and malformed change grammar with unchanged
Project bytes. The frozen Project Agent Transport v5 boundary is an
implementation/compatibility constraint, not a claim that this focused file
re-executes the separate v5 conformance corpus.

The focused gate is:

```sh
CARGO_TARGET_DIR=target/universal-semantic-workflow-cli-v1 \
  cargo test --locked -p semaprax --test workspace \
  universal_semantic_workflow_cli --no-fail-fast
```

The command passed locally with five tests and no failures.
