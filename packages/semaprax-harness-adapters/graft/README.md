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
| Qualified versions | **0.18.0** and **0.21.1** (npm `latest` on 4 Oct 2026), macOS arm64, Node 24.3.0. Any other version is `unsupported` until qualified. |

The adapter verifies the executable before use: the realpath's `package.json` must be
`@nanonets/graft` with a `repository` of `NanoNets/context-graph-engine` or `trailhq/Graft` (the
same repository, renamed), `graft --version` must equal that package's version, and that exact
version must have a compatibility profile. A look-alike package is `refused`.

### Compatibility profiles (qualifying a new version)

`compat/profiles.json` is a table keyed by exact version, never a range. Each profile records the CLI flag
names the adapter passes (`cli`), the wiring schema version, the extractor id (the parser/config identity
graft writes into `.cache/fingerprint.<id>.json`), the extensions the installed parser really indexes
(`parsed_extensions`) and those it does not, and the update-check/telemetry behaviour found. A future
version needs no compiler change:

1. Install it into an isolated prefix (`npm install --prefix <dir> @nanonets/graft@<v>`; graft's tree-sitter
   grammars are native, so `--ignore-scripts` alone leaves a non-starting binary: run
   `CI=1 DO_NOT_TRACK=1 npm rebuild` in the prefix, which skips graft's own telemetry postinstall).
2. `node scripts/qualify.mjs <graft>` builds a throwaway project, checks every command's flags, reads the
   wiring/extractor and probes each extension; it prints the profile entry and exits non-zero for an
   incompatible version.
3. Add the entry to `compat/profiles.json`, the version to `harness-provider.json`
   (`upstream.versions`, `support.tested`), run the node tests with `SEMAPRAX_TEST_GRAFT_NEW=<graft>` and the
   `real_tools_v1` graft tests with `HARNESS_GRAFT_NEW`.

An unqualified version fails with `graft.version-unqualified`, whose message lists the qualified versions and
these steps.

### Language support (measured, not inferred)

For 0.18.0 and 0.21.1 alike, `qualify.mjs` finds symbol parsers for `.ts .tsx .js .jsx .mjs .cjs .py .go .rs
.java .kt .scala .rb .php .c .h .cpp .hpp .cc .cs .swift` (Rust is supported: `.rs` yields function nodes and
call edges; `graft map` omits `rust` from its `totals.languages` list, so never read support from `map`).
`.sql .sh .proto` are code extensions graft walks but has no parser for, and `.spx`/`.spatch` are unknown to it.
The adapter reports the first group as indexed and every other source file in `coverage.skipped` with
`unsupported: the installed graft parser indexes no .<ext> files` (or the Semaprax-source reason).

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
  `<cache>/graft-context/<sha256(realpath(project))[:32]>/gen/g<N>` through graft's `--dir`
  (see [Generations](#generations-and-index-adoption)). The user's repository is never written, and a
  user `graft/` index is not read unless adoption is switched on. A cache without the adapter's
  `owner.json` marker is never overwritten (`graft.index-not-owned`).
- Graft runs with a rebuilt environment, not a filtered one. Only `PATH` (a private dir holding
  `node` and optionally `git`; no `npm`: graft 0.18.0 and 0.21.1 both start a detached `npm view` registry check
  from `$HOME/.graft/update-check.json` (24 h TTL), which finds no `npm` and learns nothing),
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
a refresh under the single-flight lock: a copy of the live generation is rebuilt (`graft build` replays
unchanged files) and published atomically. A forced rebuild passes `--no-reuse`. Referenced files are also compared by sha256 against graft's recorded
`body_hash`, so a same-size edit with preserved mtime still forces a rebuild. If the index still
disagrees with disk the result is `stale`. Cost is reported in `payload.metadata.refresh`
(`action` reuse|build|refresh, `outcome`, `served_by`, `ms`, `files_changed`, `files_indexed`, `coalesced`, and
the work split `verification_ms`, `index_ms`, `files_verified`, `bytes_hashed`, `copied_bytes`; the host allows only
scalar metadata, so these are flat) and `index_digest` (sha256 over
sorted `path\0body_hash` of the indexed files). A different project root (renamed directory, second
worktree) maps to a different index.

## Generations and index adoption

The adapter-owned index lives in immutable generations: `gen/g<N>/` (complete once published), `CURRENT`
(the live name, replaced by rename) and `refresh.lock/` (a `mkdir` lock with the holder's pid; a dead holder is
reclaimed). A refresh seeds `gen/g<N+1>.partial` from the live generation, builds into it, renames it and then
`CURRENT`; the previous generation is kept for in-flight readers. A reader therefore sees a complete old or a
complete new generation; a second process that waited for the lock re-checks and reuses the generation the first
published (`coalesced: true`) instead of rebuilding. A failed build discards its partial directory and leaves
`CURRENT` untouched.

Adoption of an existing user index is **off by default** and opt-in through the adapter environment (the host
reserves `SEMAPRAX_HARNESS_*`): `SEMAPRAX_GRAFT_ADOPT_INDEX=read-only|copied-snapshot` and optionally
`SEMAPRAX_GRAFT_USER_INDEX=<relative dir>` (default `graft`, inside the project only; nothing in `$HOME` is
scanned). On every call the adapter verifies the index before use and refuses it when: the directory is a symlink or
resolves outside the project; `wiring.json` has another schema version; the extractor id differs from the
profile's (changed parser/config, including an index built by another graft version); it holds `--deep`
summaries or concept nodes (code-only policy); an indexed path is git-ignored/excluded/absent or its content digest
differs from the working tree (a same-size edit with preserved mtime included; a Git revision is never taken as
proof); or a source file the parser covers is missing from it. `read-only` queries the user directory in place and
compares a tree digest afterwards (`graft.user-index-modified` fails the call); `copied-snapshot` copies it to a
read-only `adopted/<digest>` directory once and queries the copy. Any refusal falls back to the owned cache and the
result says so (`metadata.refresh.outcome = incompatible`, `served_by`, `index_adoption.reasons`, diagnostic
`graft.index-incompatible`). The user index is never written, refreshed or upgraded.

Outcomes: `reused-user-index`, `copied-validated-index`, `incremental-refresh`, `rebuilt`, `incompatible`, plus
`reused-owned-index` for an already fresh owned generation. `verification_ms`/`files_verified`/`bytes_hashed`
(adapter hashing, plus `graft check` on the owned path) are reported apart from `index_ms` (graft construction).
The generic descriptor and refresh primitives for other providers are in
`crates/semaprax-harness/src/context/index_adoption.rs` ([HARNESS-CONTEXT-V1](../../../docs/HARNESS-CONTEXT-V1.md#index-adoption-hn-10)).

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

Diagnostics codes are adapter-local (`graft.*`), e.g. `graft.identity-mismatch`, `graft.version-unqualified`,
`graft.index-stale`, `graft.truncated`, `graft.symbol-not-found`, `graft.coverage-incomplete`,
`graft.gitignore-unavailable`, `graft.semaprax-source`, `graft.timeout`, `graft.cancelled`.

## Unsupported cases

- `.spx` / `.spatch`: reported as skipped (`unsupported: Semaprax source ...`); `skeleton` of them is
  `unsupported` and never reaches graft. Semantic `.spx` queries belong to the Semaprax compiler.
- Languages graft has no parser for (`.sql`, `.sh`, `.proto`), files over 1 MB, dot-directories,
  `node_modules`/`target`/`vendor`-style directories: not indexed, reported when they carry a source extension.
- Without `git`, graft indexes git-ignored files; the adapter then returns `partial` with
  `graft.gitignore-unavailable`.
- `--deep`, model-backed ingestion, `graft viz`, `mcp`, LSP call edges (`--lsp`), submodule and nested-repo
  following: not used.
- Only macOS arm64 with graft 0.18.0 and 0.21.1 has been exercised. Linux is untested (no Node/graft image is
  available to Apple `container` here), and no hosted or production support is claimed.

## Tests

```sh
node --test packages/semaprax-harness-adapters/graft/test/*.test.mjs
```

`real-graft.test.mjs`, `adoption.test.mjs` and `languages.test.mjs` need graft on `PATH` (or
`SEMAPRAX_TEST_GRAFT=<abs path>`) and run again against `SEMAPRAX_TEST_GRAFT_NEW=<abs path>` when set; they skip
without a real install. `generation.test.mjs` needs none. Use `--test-force-exit` so a failing test cannot leave an
adapter child holding the runner open. `isolation.test.mjs` uses a shim upstream only to observe the environment and argv passed to graft, plus the
sandboxed offline run against the real graft (macOS only).
