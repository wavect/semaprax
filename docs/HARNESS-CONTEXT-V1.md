# Harness context broker v1 (HP-05)

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
