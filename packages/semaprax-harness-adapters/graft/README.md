# Graft repository-context adapter

Provider `org.nanonets/graft-context` for the `context.repository` v1 capability
([HARNESS-PROVIDER-V1](../../../docs/HARNESS-PROVIDER-V1.md)). It wraps an
**already installed** [Graft](https://github.com/trailhq/Graft) and answers four
operations: `orient`, `search`, `skeleton`, `references`. Node standard library only; no
Cargo or compiler dependency. Files: `harness-provider.json` (descriptor),
`adapter.mjs` + `lib/` (program), `test/` (real-graft and shim tests).

## Upstream identity

| | |
| --- | --- |
| npm package | `@nanonets/graft` (license MIT, read from the installed `package.json`) |
| Canonical repository | `https://github.com/trailhq/Graft` |
| Redirect | `https://github.com/NanoNets/Graft` answers `301` to the canonical URL; the npm package metadata still names `NanoNets/context-graph-engine`. Not to be confused with similarly named "graft" projects. |
| Tested version | **0.18.0** (macOS arm64, Node 24.3.0). Any other version is `unsupported` until re-tested. |

The adapter verifies the executable before use: the realpath's `package.json` must be
`@nanonets/graft`, and `graft --version` must equal that package's version and be in the
tested set. A look-alike package is `refused`.

## Adopting an existing installation

Nothing is installed or upgraded by Semaprax.

1. Have graft 0.18.0 installed yourself (`npm i -g @nanonets/graft@0.18.0`); note its absolute path.
2. `semaprax harness adopt harness-provider.json --upstream <abs path to graft>` then
   `semaprax harness trust org.nanonets/graft-context` (HP-02).
3. Set `[capability."context.repository"] mode = "auto"` (or pin `provider`) in `semaprax.harness.toml`.
   With no approved provider, the host falls back to native/source context.

The host launches the adapter with three explicit variables (nothing is discovered from
`PATH`/`HOME`): `SEMAPRAX_HARNESS_UPSTREAM` (absolute graft path),
`SEMAPRAX_HARNESS_PROJECT_ROOT`, `SEMAPRAX_HARNESS_CACHE_DIR`; optionally
`SEMAPRAX_HARNESS_GIT` (default `/usr/bin/git` when present). Missing values give
`unavailable` (`graft.config-missing`). A cache root overlapping the project is refused.

## Local-only operation

- The index is built **code-only** (`graft build`, never `--deep`, no model, no key) into
  `<cache>/graft-context/<sha256(realpath(project))[:32]>/idx` through graft's `--dir`.
  The user's repository is never written, and an existing user `graft/` index is never read
  or modified. An index directory without the adapter's `owner.json` marker is never overwritten
  (`graft.index-not-owned`).
- Graft runs with a rebuilt environment, not a filtered one. Only `PATH` (a private dir holding
  `node` and optionally `git`; no `npm`, so graft's registry update check cannot run),
  `HOME` and `TMPDIR` (private, inside the cache), `DO_NOT_TRACK=1` (graft telemetry gate),
  `GRAFT_NO_REFRESH=1`, `NODE_OPTIONS=--max-old-space-size=2048`, `GIT_CONFIG_NOSYSTEM`,
  `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_OPTIONAL_LOCKS=0` and `LC_ALL=C` are set. `GRAFT_API_KEY`,
  `GRAFT_PROVIDER`, `GRAFT_MODEL`, `GRAFT_BASE_URL`, any `*_API_KEY`/`*_TOKEN`, proxies and
  `GRAFT_DIR` are never inherited. cwd is an empty cache directory (graft loads `.env` from cwd).
- Graft's global `~/.graft` is never touched; the adapter never runs `graft telemetry`, `init`,
  `upgrade` or any command that writes configuration.
- Bounds: per-invocation deadline (`deadline_ms`, capped at 300 s) kills graft's process group;
  `harness/cancel` kills it immediately; graft output is capped at 4 MiB; indexing refuses
  projects with more than 20000 candidate source files; results are shrunk to `max_result_bytes`.
- Tested with all network denied: `sandbox-exec -p '(version 1)(allow default)(deny network*)'`
  around the adapter (cold build, warm reuse and a second process reusing the warm index).

## Freshness

Before each invocation the adapter runs `graft check --json`; drift (or a missing index) triggers
an incremental `graft build`. Referenced files are also compared by sha256 against graft's recorded
`body_hash`, so a same-size edit with preserved mtime still forces a rebuild. If the index still
disagrees with disk the result is `stale`. Cost is reported in `payload.metadata.refresh`
(`action` reuse|build|refresh, `ms`, `files_changed`, `files_indexed`) and `index_digest` (sha256 over
sorted `path\0body_hash` of the indexed files). A different project root (renamed directory, second
worktree) maps to a different index.

## Result mapping

Item: `{path, span{start_line,end_line}, digest, provenance, language, rank, text?}`. `path` is
project-relative POSIX. `digest` is `sha256:<hex>` of the exact bytes of the span's lines
(lines joined by LF, no trailing terminator: the context broker's re-hash convention); a file-level item hashes all lines. `rank` is graft's order, 1-based.

| Operation | Graft command | Provenance | Exhaustive |
| --- | --- | --- | --- |
| `orient` `{max_dirs?}` | `map --json` | `structural` | never |
| `search` `{query, max_items?, in?, refresh?}` (`mode`, `include_source` are adapter-internal defaults) | `ask --json` / `grep --json --fixed` | `structural` / `inferred` (text match) | ranked never; exact only if not truncated |
| `skeleton` `{path}` | `skeleton --json` | `structural` | never |
| `references` `{symbol, direction?, depth?, path_prefix?, exhaustive?}` | `callers --json`; with `exhaustive:true` also `grep --fixed` | `structural`, `inferred` for text sites | only with `exhaustive:true` and untruncated |

Nothing is ever labelled `compiler-verified`. `coverage` is `{complete, exhaustive, indexed_files,
skipped:[{path,reason}]}`. `exhaustive` means every textual occurrence in the **indexed** files; `complete` is
false while any source file is skipped (listed up to 200) or the result is truncated.
`metadata.absence_proven` is true only if both hold; an empty result with it false must never be read as
"no references". Call edges alone (`callers`) are static name wiring and are not exhaustive.

Diagnostics codes are adapter-local (`graft.*`), e.g. `graft.identity-mismatch`, `graft.version-untested`,
`graft.index-stale`, `graft.truncated`, `graft.symbol-not-found`, `graft.coverage-incomplete`,
`graft.gitignore-unavailable`, `graft.semaprax-source`, `graft.timeout`, `graft.cancelled`.

## Unsupported cases

- `.spx` / `.spatch`: reported as skipped (`unsupported: Semaprax source ...`); `skeleton` of them is
  `unsupported` and never reaches graft. Semantic `.spx` queries belong to the Semaprax compiler.
- Languages graft has no parser for (for example Rust in 0.18.0's parser set), files over 1 MB, dot-directories,
  `node_modules`/`target`/`vendor`-style directories: not indexed, reported when they carry a source extension.
- Without `git`, graft indexes git-ignored files; the adapter then returns `partial` with
  `graft.gitignore-unavailable`.
- `--deep`, model-backed ingestion, `graft viz`, `mcp`, LSP call edges (`--lsp`), submodule and nested-repo
  following: not used.
- Only macOS arm64 with graft 0.18.0 has been exercised. No Linux, no hosted, no production support is claimed.

## Tests

```sh
node --test packages/semaprax-harness-adapters/graft/test/*.test.mjs
```

`real-graft.test.mjs` needs graft on `PATH` (or `SEMAPRAX_TEST_GRAFT=<abs path>`) and skips otherwise;
`isolation.test.mjs` uses a shim upstream only to observe the environment and argv passed to graft, plus the
sandboxed offline run against the real graft (macOS only).
