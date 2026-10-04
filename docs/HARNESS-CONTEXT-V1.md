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
  `in` (project-relative path prefix on search/references). Page size is
  `max_items`, never `limit`. `auto` rebuilds a provider index that is behind the
  working tree; `never` answers `stale`.
- result: `metadata` (at most 8 KiB, scalars or one level of scalar-valued objects;
  still scanned for authority-like members, `SPX-HPA036`), `coverage.extraction_errors`
  `[{path, reason}]` (surfaced by the broker as skipped entries), item `edges`
  `[{target <= 1024, relation <= 64, provenance structural|inferred}]`. An edge can
  never be `compiler-verified`.

Digest convention: an item's `digest` is `sha256:<hex>` of lines `start..=end` joined
by LF with no trailing terminator, the form the broker re-hashes. A provider using
another convention has every item reported `stale-digest` and unverified.

Interpreter-shebang upstreams need their runtime to be probed: `adopt` adds the
directories of `HARNESS_NODE`/`HARNESS_PYTHON` to the probe's `PATH`.

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

