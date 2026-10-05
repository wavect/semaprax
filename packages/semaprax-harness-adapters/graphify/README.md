# Graphify context provider (`com.graphify-labs/graphify-context`)

A `context.repository/v1` adapter (operations `orient`, `search`, `skeleton`,
`references`) over [Graphify](https://github.com/Graphify-Labs/graphify),
speaking `semaprax.harness-rpc.v1` on stdio. Python 3, standard library plus
`../sdk/python/semaprax_harness_adapter.py`. Nothing here enters Cargo or the
compiler build. Outcome of the ADR 0001 re-evaluation: see
[EVIDENCE.md](EVIDENCE.md); adapter availability is opt-in only.

## Adoption journey

1. Install Graphify yourself, outside Cargo: `uv tool install graphifyy==0.9.75` (0.9.25 also tested)
   (the PyPI package is `graphifyy`, two y's; the adapter checks identity, see below).
2. `semaprax harness adopt packages/semaprax-harness-adapters/graphify/harness-provider.json --upstream <abs path to graphify>`,
   then `semaprax harness trust com.graphify-labs/graphify-context`.
3. In `semaprax.harness.toml` pin
   `[capability."context.repository"] provider = "com.graphify-labs/graphify-context"`.
   Switching between Graft and Graphify is this one line; no source or compiler change.
4. The first query lazily runs a local code-only extraction into the host cache dir;
   later queries read `graph.json` directly.

## Local-only guarantees

* The host supplies `SEMAPRAX_HARNESS_UPSTREAM` (absolute graphify path),
  `SEMAPRAX_HARNESS_PROJECT_ROOT` and `SEMAPRAX_HARNESS_CACHE_DIR`. Graph output
  goes only to `<cache>/graphify-index/<version>` (`--out`, staged then published atomically under a cache lock); the project tree is never written
  (a test compares the file tree before and after). Child cwd is the cache dir.
* Extraction is always `graphify extract <root> --code-only --out ...`: local
  tree-sitter AST, no model call, no docs/PDF/media ingestion. The adapter never
  passes `--backend`, `--model`, `--mode deep`, `--global`, `--postgres`, `--cargo`
  or any `install`/hook/MCP/`add` command.
* The child environment is an **allowlist**, so nothing else is inherited:
  `LANG`, `LC_ALL`, `LC_CTYPE`, `TMPDIR`, plus fixed `PATH=/usr/bin:/bin`,
  `HOME=<cache>/home` (so `~/.graphify` and agent config dirs are never read or
  written), `PYTHONDONTWRITEBYTECODE=1`, `GRAPHIFY_NO_TIPS=1`. Stripped by
  construction, and explicitly covered by the planted-variable test: every
  variable below found in graphify 0.9.25 source, and any other.
  * Provider keys: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`,
    `GOOGLE_API_KEY`, `MOONSHOT_API_KEY`, `DEEPSEEK_API_KEY`,
    `AZURE_OPENAI_API_KEY`, `OLLAMA_API_KEY`, `GRAPHIFY_API_KEY`.
  * Endpoints/models: `OPENAI_BASE_URL`, `OPENAI_MODEL`, `ANTHROPIC_BASE_URL`,
    `ANTHROPIC_MODEL`, `GEMINI_BASE_URL`, `KIMI_BASE_URL`, `DEEPSEEK_BASE_URL`,
    `OLLAMA_BASE_URL`, `OLLAMA_HOST`, `OLLAMA_MODEL`, `AZURE_OPENAI_ENDPOINT`,
    `AZURE_OPENAI_DEPLOYMENT`, `AZURE_OPENAI_API_VERSION`, `GRAPHIFY_AZURE_MODEL`,
    `GRAPHIFY_CLAUDE_CLI_MODEL`, `GRAPHIFY_TRIAGE_BACKEND`, `GRAPHIFY_TRIAGE_MODEL`,
    `GRAPHIFY_ALLOW_LOCAL_PROVIDERS`, other `GRAPHIFY_*` tuning
    (`GRAPHIFY_OUT`, `GRAPHIFY_REPO_ROOT`, `GRAPHIFY_FORCE`, `GRAPHIFY_MAX_WORKERS`, ...).
  * Cloud/db credentials read by other graphify paths: `AWS_*`, `NEO4J_PASSWORD`,
    `FALKORDB_PASSWORD`, `CLAUDE_CONFIG_DIR`, `CLAUDE_PROJECT_DIR`, and every
    `*_API_KEY` / `*_TOKEN` / `*_SECRET`.
* Identity is verified, not assumed: the adapter reads
  `lib/python*/site-packages/graphifyy-*.dist-info/METADATA` next to the resolved
  upstream executable and requires `Name: graphifyy`, a Graphify-Labs/graphify
  repository URL and a tested version. Anything else returns `unsupported`
  (`SPX-HPG001`) before any extraction.

## Behaviour

Request payloads (contract names): `search {query, max_items?}`, `skeleton {path}`,
`references {symbol, max_items?}`, `orient {max_items}`; every operation also accepts
`refresh: "auto"|"rebuild"|"never"` (default `auto`: rebuild a graph that is behind the
tree; `never` answers `stale`). Digests are `sha256:<hex>` of one line without terminator.

* The adapter reads `graphify-out/graph.json` and validates an explicit schema profile
  per tested version (`PROFILES` in `adapter.py`: 0.9.25 and 0.9.75; any other version is
  `unsupported`, there is no guessing). The 0.9.75 profile additionally admits the optional
  boolean node keys `_callable` and `_callable_class` and nothing else new. The shape: top-level `nodes` and `links` lists (NetworkX node-link; `edges` is the
  `--no-cluster` raw layout and is refused), node keys `id,label,file_type,source_file`
  with `source_location` as `L<line>` (start line only, so spans are
  `start_line == end_line`), edge keys `source,target,relation,confidence,source_file`
  with `confidence` in `EXTRACTED|INFERRED|AMBIGUOUS`. Neither version records an end
  line; upstream ranges are never invented. Any deviation yields
  `unsupported` (`SPX-HPG003`). It never parses `graphify query` output.
* Provenance: `EXTRACTED` edge -> `structural`; `INFERRED`/`AMBIGUOUS` -> `inferred`;
  an edge to an unresolved stub target (no `source_file`) -> `inferred`. Nodes are
  `structural`. Never `compiler-verified`; graphify confidence is not translated
  into compiler certainty.
* Coverage: every walked project file that graphify did not index is listed with a
  reason (`.spx`, `.spatch`, manifests, documents/media skipped by `--code-only`,
  unsupported language). Extraction error lines from the graphify log go to
  `coverage.extraction_errors` (an additive member; the host contract must admit it).
  `complete` is false if anything was skipped or errored, and the status is then
  `partial`. `references` is exhaustive only when coverage is complete, and an empty
  result always carries a diagnostic that absence is not proof of no callers.
  Mixed `.spx` projects are therefore never exhaustive.
* Staleness: a SHA-256 over the sorted (path, content-hash) set of all non-hidden
  files except unconsumed media (`png jpg jpeg gif svg webp mp3 mp4 wav mov webm`, which
  code-only extraction never opens) is recorded at build. Media are never read or hashed
  to validate the index; their additions, deletions and edits do not re-extract, and the
  skipped-coverage list is recomputed from the current tree on every lookup. Documents,
  manifests and other files stay in the identity. A changed set returns `stale` (`SPX-HPG002`) or, with
  `refresh: "rebuild"`, rebuilds. A new adapter process reuses the on-disk index
  only if root, upstream version and digest all match; otherwise it rebuilds.
* Results are truncated to 80% of `budget.max_result_bytes` and marked incomplete.

* Cache identity: the index lives in `graphify-index/<extractor version>/`, and
  `adapter-meta.json` (schema v2) records version, profile id, root, source digest and the
  graph.json SHA-256. Reuse requires every field to match, so output cached by 0.9.25 is
  never read as the 0.9.75 schema (and an old unversioned index is ignored). Concurrent
  adapter processes serialize on `<cache>/graphify-index.lock`; builds stage privately.
* Spans: a node is `[definition]` only when a version-bound source resolver proved its
  range (Python `ast` `end_lineno`, recorded as `span_resolver` in metadata); every other
  node is `[start-line]` (one line, not the whole definition). The closed item shape has no
  span-kind member, so the kind leads `text` and `metadata.span_kinds` counts them. Digests
  are the host's rule (LF-joined lines), so the host rehashes them independently.
* Call resolution (`references`): every edge is `resolved`, `ambiguous` or `unsupported`
  (edge relation `calls:<status>`, text, `metadata.resolution`). Graphify `EXTRACTED` is a
  parse fact, not a binding proof: a call whose name matches several methods is
  `ambiguous` (provenance `inferred`) unless the 0.9.75 profile can justify it (same-class
  call with no subclass override, or `super` call with exactly one defining ancestor).
  Stub targets are `unsupported`. Dynamic dispatch, untyped receivers and `getattr` leave
  no edge (`SPX-HPG012`), so references are never exhaustive.

## Tested and unsupported

Tested: graphifyy 0.9.25 and 0.9.75, macOS arm64, Python 3.12, including two and three
processes sharing one cache dir. Other versions, Linux (untested: no local Linux image) and
Windows are unsupported.
Unsupported: `.spx`/`.spatch` content (use the native compiler context), documents,
PDFs, media, model-backed extraction, `--global` graphs, MCP/serve, exhaustive call resolution (see EVIDENCE.md), end-line spans for non-Python languages.

## Tests

```sh
cd test && python3 -m unittest -v test_adapter
```

Uses the real graphify (`SEMAPRAX_HARNESS_UPSTREAM`, default `~/.local/bin/graphify`;
`SEMAPRAX_HARNESS_UPSTREAM_NEW` enables the 0.9.75 classes)
over a temporary Python + TypeScript + `.spx` project.
