# Graphify context provider (`com.graphify-labs/graphify-context`)

A `context.repository/v1` adapter (operations `orient`, `search`, `skeleton`,
`references`) over [Graphify](https://github.com/Graphify-Labs/graphify),
speaking `semaprax.harness-rpc.v1` on stdio. Python 3, standard library plus
`../sdk/python/semaprax_harness_adapter.py`. Nothing here enters Cargo or the
compiler build. Outcome of the ADR 0001 re-evaluation: see
[EVIDENCE.md](EVIDENCE.md); adapter availability is opt-in only.

## Adoption journey

1. Install Graphify yourself, outside Cargo: `uv tool install graphifyy==0.9.25`
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
  goes only to `<cache>/graphify-index` (`--out`); the project tree is never written
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

Request payloads: `search {query, limit?}`, `skeleton {path}`,
`references {symbol, limit?}`, `orient {limit?}`; every operation also accepts
`refresh: "stale"|"rebuild"` (default `stale`, the descriptor's `refresh` config).

* The adapter reads `graphify-out/graph.json` and validates the pinned 0.9.25
  schema: top-level `nodes` and `links` lists (NetworkX node-link; `edges` is the
  `--no-cluster` raw layout and is refused), node keys `id,label,file_type,source_file`
  with `source_location` as `L<line>` (start line only, so spans are
  `start_line == end_line`), edge keys `source,target,relation,confidence,source_file`
  with `confidence` in `EXTRACTED|INFERRED|AMBIGUOUS`. Any deviation yields
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
  files is recorded at build. A changed set returns `stale` (`SPX-HPG002`) or, with
  `refresh: "rebuild"`, rebuilds. A new adapter process reuses the on-disk index
  only if root, upstream version and digest all match; otherwise it rebuilds.
* Results are truncated to 80% of `budget.max_result_bytes` and marked incomplete.

## Tested and unsupported

Tested: graphifyy 0.9.25, macOS arm64, Python 3.12. Other versions, Linux, Windows
and concurrent adapter processes sharing one cache dir are untested/unsupported.
Unsupported: `.spx`/`.spatch` content (use the native compiler context), documents,
PDFs, media, model-backed extraction, `--global` graphs, MCP/serve, call-resolution
completeness (graphify resolves calls by name; same-named symbols across files
collide, see EVIDENCE.md), end-line spans.

## Tests

```sh
cd test && python3 -m unittest -v test_adapter
```

Uses the real graphify (`SEMAPRAX_HARNESS_UPSTREAM`, default `~/.local/bin/graphify`)
over a temporary Python + TypeScript + `.spx` project.
