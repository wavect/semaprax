# Harness context broker v1 (HP-05)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

Audience: toolchain contributors and harness adapter authors.

`semaprax harness context <project> <query> [--max-bytes N] [--symbol S] [--references] [--purge-cache] [--json]`
composes compiler facts with one external `context.repository/v1` provider.
Owner: `crates/semaprax-harness/src/context/`. Diagnostics `SPX-HPE`.

## Output

Document `semaprax.harness-context.v1`, canonical JSON, budget unit `byte-v1`
(output bytes, never model tokens). Every byte of the document, metadata and
handles included, fits `--max-bytes`.

- `native`: items with `provenance: compiler-verified`. `text` is the exact
  standalone `semaprax context` stdout (project mode when `semaprax.toml`
  exists, else file mode with the compiler's `--filters`). Mandatory: if they
  and the metadata exceed the budget the request is refused (`SPX-HPE001`).
- `external`: whole items only, `structural` or `inferred`. A provider claim of
  `compiler-verified` is demoted (`SPX-HPE050`). Rank is provider-local and
  never compared across providers.
- `omitted`: count plus retrieval handles `ctx:<provider>:<path>#<a>-<b>@<digest>`.
- `references` (reference queries): `exhaustive` and `definitive_absence` are
  true only for a complete, exhaustive, fully verified, scope-unrestricted
  provider answer with nothing omitted by budget and an explicit
  `no_references`. Otherwise a source-search recommendation is given.

## Revision binding (MN-05)

The capture's per-file digests are the only evidence of the captured revision.
Every source read the broker renders from (`Snapshot::read_bound`) hashes the
exact bytes it returns and compares the whole-file digest with the capture, so a
span digest is only ever compared against bytes of the captured revision. A
matching span in a file that changed elsewhere is therefore still unverified.
Outcomes, reported as the item's `omission_reason` (item `verified=false`,
`complete=false`, provenance `inferred`, revision unchanged and never relabelled):

| Source state | Reason |
| --- | --- |
| unchanged, span matches | verified |
| unchanged, span differs | `stale-digest` |
| edited (same size or not) or replaced by another file kind | `source-changed` |
| deleted, renamed, or not in the capture | `deleted-or-renamed` |
| read failure or non-UTF-8 | `unreadable` |

`.spx` declaration scanning uses the same bound read, so a changed `.spx` file
contributes no declarations. A native item whose declaring file no longer matches
the capture is also reported unverified, incomplete and not `compiler-verified`;
the compiler's facts keep their own `provider_id` and are never merged into the
external tier. A provider response is written to the cache only after every
returned item verified and a fresh capture still names the same revision; a
failed binding is never cached. A mutable path is not an immutable snapshot:
this is a point-in-time proof at each read, not protection against a later edit.

## External `.spx` references

Each external item's span digest is re-hashed against the current working
tree. A match inside an `@id` declaration whose identity the compiler confirms
is `resolved`; a mismatch is `stale`, otherwise `unmappable`. All stay hints:
only compiler-verified, verified items authorize edits. A resolved reference to
a fact already in `native` is not repeated.

## Selection

One provider per scope. Federation is explicit (`Broker::enable_federation`)
and every provider needs a non-empty, pairwise-disjoint scope (`SPX-HPE040/041`).
Per-provider budgets split the space left after native facts.

## Cache

Stores provider responses under `<harness home>/cache/context`, never in the
project. Key: project + worktree identity, content digest of the working tree
(edits, deletions, renames), provider/descriptor/upstream identity, config and
permission-scope digest, query, output contract. Authority
(`profile::check_grant_current`) is rechecked on every read, hits included; a
failed recheck purges that provider's entries. Bounded by entries, bytes, TTL;
a newer revision supersedes older entries of the same query. `--purge-cache`
purges everything.

## Real providers (HP-06, HP-07)

`packages/semaprax-harness-adapters/graft` (Graft 0.18.0) and `.../graphify`
(Graphify 0.9.25) implement `context.repository/v1` over real tools; evidence in
their `EVIDENCE.md`, selection by `[capability."context.repository"] provider = ...`
only.

Additive optional contract members (v1 stays closed otherwise; validated, bounded):

- request: `refresh` (`auto` | `rebuild` | `never`), `exhaustive` (references),
  `in` (project-relative path; component-aware, so `allowed` admits `allowed` and
  `allowed/...`, never `allowed-sibling/...`; applied by the adapter before ranking
  and `max_items`, and again by the broker, on top of the configured provider scope,
  so an item outside it is never admitted and an out-of-scope drop forfeits
  exhaustiveness). Page size is
  `max_items`, never `limit`. `auto` rebuilds a provider index that is behind the
  working tree; `never` answers `stale`.
- result: the whole serialized envelope fits the requested `max_result_bytes`: adapters
  trim items first, then skipped-file detail, metadata and diagnostics, count every
  omission, never upgrade coverage, and send a bounded refusal when nothing fits.
  `metadata` (at most 8 KiB, scalars or one level of scalar-valued objects;
  still scanned for authority-like members, `SPX-HPA036`), `coverage.extraction_errors`
  `[{path, reason}]` (surfaced by the broker as skipped entries), item `edges`
  `[{target <= 1024, relation <= 64, provenance structural|inferred, resolution?}]`
  (an edge can never be `compiler-verified`), item `span_kind` (`definition` = the span
  is the whole definition, `start-line` = only its first line) and edge `resolution`
  (`resolved|ambiguous|unsupported`, the provider's own claim). Both are carried
  through the broker, the cache and the rendered item unchanged.

Digest convention: an item's `digest` is `sha256:<hex>` of lines `start..=end` joined
by LF with no trailing terminator, the form the broker re-hashes. A provider using
another convention has every item reported `stale-digest` and unverified.

Interpreter-shebang upstreams need their runtime to be probed: `adopt` adds the
directories of `HARNESS_NODE`/`HARNESS_PYTHON` to the probe's `PATH`.

## Task-relevant planning (HN-13)

Owner: `context/plan.rs`, `workflow/{pipeline,broker_stage}.rs`. Native graph
completeness is not the retrieval criterion; the task is. `plan::plan(project,
goal, seed, diagnostics)` derives `EvidenceNeeds`:

- selected symbols (seed plus `.spx` ids/names the goal mentions);
- affected foreign-language boundaries: language words in the goal, named files
  (`web/app.ts`), and a manifest `web_exports` symbol when the goal changes the
  interface (rename, signature, export, abi ...);
- configuration words (`config`, `toml`, `yaml`, `env` ...);
- unresolved references quoted by "unresolved/unknown/not found" diagnostics
  that no `.spx` declaration owns;
- an exhaustive-reference request ("all callers", "every usage" ...).

Routing. `.spx` meaning always comes from compiler queries. The one
project-selected provider is consulted only when a need is foreign, a
configuration, an unresolved name or an exhaustive reference request. A task
that only names `.spx` symbols is native-only: zero provider calls. `external_context`
`never` still forbids the provider (and the plan reports the resulting unknown);
`always` forces the legacy one-shot query when no concrete need exists.

Bounded plan. The first step is the smallest relevant query (symbol ids/names,
named files, unresolved names, config words; `.spx` items excluded because the
compiler owns them, counted as `SPX-HPE070`). A step that surfaces none of the
needed languages gets at most one goal-worded retry; one failed candidate
diagnostic gets at most one focused follow-up (`ContextStage::follow_up`)
whose query is only the identifiers the failure names and the prior query did
not ask (`max_items <= 8`). `MAX_PROVIDER_CALLS` is 3 and nothing is recursive.
Workflow wiring of `follow_up` into a retry loop belongs to the session owner;
`pipeline::follow_up_context` is the merge-and-budget entry point.

Dedup and mandatory data. Exact `path:span` + provenance + text is forwarded once
(provider duplicates and cross-packet repeats); native facts (contracts, effects,
ownership, types filters) are mandatory and budgeted before any optional item.

Honesty. `context.plan.retrieval` reports `ranked`, `exhaustive`,
`absence_provable`, `coverage_complete`, `omitted_items`, `unknowns[]` and
`continuation[]`. Ranked top-N output is `ranked: true, absence_provable: false`;
an exhaustive request either establishes scoped complete coverage (provider
complete + exhaustive + nothing omitted + every slice verified + nothing
filtered) or states `exhaustive reference coverage declined`. A provider that is
unavailable or stale keeps the native facts (marked incomplete) and adds an
unknown ("repository provider failed ..."); it is never retried. Goal text never
enters a report: steps carry a query digest and term count only.

Continuation. Omitted items keep `ctx:` handles (at most 8). `ContextStage::expand`
turns one handle into its slice with no provider call: the slice is read from the
working tree only if it still hashes to the handle digest (`SPX-HPE071` stale or
drifted), fits the remaining budget (`SPX-HPE072`) and stays inside the project
(`SPX-HPE073`); unknown handles are `SPX-HPD130`. The workflow budget refuses a
slice that does not fit (`SPX-HPD131`).

Cache. `BrokerContext::with_cache(root)` (opt-in) reuses the broker cache; the key
already binds project + worktree, the content digest of the working tree,
provider/descriptor/upstream identity, the lock digest (added to the provider
identity), configuration, permission scope and the exact query, so a changed
plan step, edit, worktree or lock is a miss. A hit is not a provider invocation.

## Index adoption (HN-10)

Owner: `crates/semaprax-harness/src/context/index_adoption.rs`; diagnostics `SPX-HPF`. Reusing an installed
executable is not reusing the user's index. Adoption is generic, opt-in per provider, and never blind.

An `IndexDescriptor` (`semaprax.harness-index-adoption.v1`) names provider, upstream version, index schema, canonical
source root, worktree id, a configuration digest (parser/extractor identity and indexing options), the content digest of
every indexed input, coverage and the ownership mode `read-only` or `copied-snapshot`. `verify(descriptor, expected)`
returns every mismatch: `SPX-HPF001` provider/version/schema, `002` another root or worktree, `003` changed parser or
configuration, `004` a private/excluded path in the index, `005` stale content (digest differs or file gone; length and
Git revision are never evidence), `006` a current file the index lacks. Only an empty result lets the index answer. The
user's index is read in place or copied once into an immutable snapshot (`copy_snapshot`); it is never overwritten,
refreshed or upgraded, and an adopted index may not carry content from a richer mode than the project's policy allows.
Any refusal falls back to an owned cache or native context and is reported, never silent.

Semaprax-owned caches use `GenerationStore`: a `mkdir` single-flight `RefreshLock` (dead holders reclaimed,
`SPX-HPF010` on wait timeout), `begin` into `gen/g<N>.partial`, `publish` by directory rename then `CURRENT` rename, and
pruning that keeps the previous generation. A reader sees a complete old or a complete new generation. The broker's
`ResultCache` writes each entry through a unique temp file, so concurrent writers cannot cross entries.

`Outcome` reports what happened: `reused-user-index`, `copied-validated-index`, `incremental-refresh`, `rebuilt`,
`incompatible`, with `Work` counters (files verified, bytes hashed, bytes copied, files indexed) kept apart from
index-construction time. Shared blobs, if any, are keyed by content, version and configuration; working-tree state,
locks, permissions and query results stay per worktree, and no cache crosses a project trust boundary. The Graft
adapter implements the same rules in Node (`packages/semaprax-harness-adapters/graft/lib/adopt.mjs`,
`generation.mjs`); the Graphify adapter reuses this contract.

### Adapter configuration plumbing (HN-10)

`[capability."context.repository".config]` in `semaprax.harness.toml` supplies the descriptor's `config.fields`.
The host validates each value against the selected descriptor (declared field, type, no secret-looking value, no
absolute/home/`..` path) and forwards it as host-set environment `SEMAPRAX_HARNESS_CFG_<FIELD>` (upper-cased;
string lists comma-joined). The prefix is host-reserved, so an adapter can trust it; defaults are not forwarded.
The config (and therefore its digest) is part of every cache key. Graft reads `adopt_index` (`read-only` |
`copied-snapshot`) and `user_index` (default `graft`); the older `SEMAPRAX_GRAFT_*` names remain aliases.
Graphify gains the same opt-in with `user_index` defaulting to `graphify-out`: the graph must satisfy the
installed version's profile, carry that version's per-version AST cache, be code-only (`_origin: ast`), name this
worktree (`.graphify_root`) and bind to the source (graphify's own content key where it records one, otherwise its
recorded size and nanosecond mtime, a weaker binding). Any refusal serves from the owned cache and reports
`index_adoption`/`served_by` metadata; the user's files are never written.
