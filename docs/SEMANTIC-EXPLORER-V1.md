# Semantic Explorer View v1

`semaprax.explorer-view.v1` is a bounded, read-only visualization projection over a held checked Project revision. It is an adapter over `ProjectSemanticImage`, `ProjectCandidate`, `workspace_graph`, and `workspace_analysis`; it accepts no graph JSON, source path, or cache input.

## Methods

The default v5 read policy selects `image/explorer-summary` and `image/explorer-page`. Candidate preparation additionally selects `candidate/explorer-summary` and `candidate/explorer-page`. Every request binds the current outer `image_revision`; candidate requests also bind an exact retained `candidate_revision` and select `side: base|candidate`. Image requests always use `current` internally. A candidate base is the candidate's retained original base, never the currently open image.

A summary has `mode`, `target`, `query`, an artifact digest, truncation and coverage facts, and handles for `modules`, `declarations`, `relations`, and `frontier`. A page returns one selected inventory in compiler order. Its handle and cursor bind subject, candidate side, mode, target, direction, depth, node and byte bounds, view, offset, and page options. Page defaults are 32 rows and 64 KiB; fixed maxima are 128 rows and 512 KiB. Summaries are at most 64 KiB. A row that cannot fit fails closed.

`overview` has no target and uses the retained workspace graph. `context` requires a declaration ID and delegates to the existing context kernel with `forward`, `reverse`, or `both`. `impact` requires a declaration ID and delegates to the existing reverse impact kernel. Query defaults are depth 1, 256 nodes, and 256 KiB. Existing kernel limits remain authoritative.

Candidate construction admits a narrow `delete_declaration` intent for an explicit top-level non-main function that is neither a manifest export nor structurally referenced in the retained graph. It rebuilds and replays the full Project as other candidate intents do. After deletion, base-side context and impact still use the candidate's original retained base; the same target on the candidate side fails with the kernel's absent-target diagnostic, rather than returning an empty impact inventory.

Node keys are `project_name:project_revision:side:stable_id`, so the same identity on base and candidate sides cannot collide. A declaration without a file path has `source_reference.kind: "non_file_node"`; an analysis row whose source binding is not carried by its owning kernel uses the explicit `authenticated_source_reference_unavailable_in_analysis_projection` state rather than a fabricated reference. Relation rows preserve their original family and provenance, including parallel sites. The only admitted structural families are `function_import`, `type_import`, `call`, `type_reference`, `effect_requirement`, and `capability_authority`; an unfamiliar retained family is never rewritten as a call. Module totals are presentation facts. Capability/prelude nodes without paths remain non-file nodes. Loans and cleanup plans remain function facets and are not relations.

A response supplies no source, execution, test, or publication authority. It does not claim runtime liveness, coverage, external/dynamic absence, or a complete result when the kernel reports truncation. Source drift stays absorbing at the enclosing v5 session boundary.

## Local review exports

## User guide

### Standalone first

Install or build the public `semaprax` compiler, then point it at a checked
SEMAPRAX project manifest. Here, `repo` means the compiler-admitted `.spx`
sources and `semaprax.toml` manifest of that project. It does not index an
arbitrary Rust, TypeScript, Git, or filesystem repository.

```sh
# Inspect the installed command contract.
semaprax explore

# Generate a source-free, self-contained overview for offline review.
semaprax explore ./semaprax.toml --format html --output ./semantic-explorer.html

# Focus the report on one stable declaration identity.
semaprax explore ./semaprax.toml --target app.main --depth 2 \
  --format markdown --output ./app-main.md
```

`--format` accepts `html`, `json`, `markdown`, and `svg`; `--output` is always
required and is no-clobber. `--target <stable-id>` enables a focused context
view, and `--depth <1..16>` is valid only with that target. HTML opens directly
through `file://` in a browser and embeds the same viewer model, layout,
evidence, cache, change, host, and view assets used by the VS Code extension.
No Node.js, editor, server, account, network access, model provider, test run,
or build action is needed to view a generated report.

Use `--include-source` only when reviewers need the authenticated retained
source. It is accepted for HTML and JSON, rejected for Markdown and SVG, and
does not grant any apply or execution authority. Without it, source bodies are
not bundled. Evidence or facets absent from an offline snapshot display as
`not bundled`; that state is not an error or a claim about the project.

To inspect a candidate, provide both the retained capsule and its expected
digest. The command restores and verifies that capsule against the manifest's
saved source before rendering base and candidate views:

```sh
semaprax explore ./semaprax.toml \
  --candidate-capsule ./candidate.capsule.json \
  --expect-candidate sha256:YOUR_EXACT_CANDIDATE_DIGEST \
  --format html --output ./candidate-explorer.html
```

The command is read-only for project source and candidate history. It writes
only the requested output artifact, and refuses an existing output path.
Snapshots and exports are review displays, never candidate authority.

### Optional VS Code view

The existing `wavect.semaprax` extension packages the same hashed viewer assets.
Set the compiler path, manifest path, and host-policy path as user or machine
settings, then run **SEMAPRAX: Start Saved-Source Session**. Use **Open Semantic
Explorer** for the current project, **Explore Selected Declaration** for a
stable ID, and **Review Candidate Graph** after explicitly opening a candidate.

The extension never discovers a compiler, changes policy, or silently refreshes
saved source. A dirty source buffer, stopped session, or stale session blocks
exploration until it is saved and explicitly refreshed or restarted. Current
source reveal validates its retained digest and UTF-8 to UTF-16 location at
click time; candidate and base source remain immutable review material. The
extension-only actions are panel opening, current-source reveal, and virtual
before/after review. Offline reports deliberately have no live session, source
reveal, candidate mutation, test execution, build, approval, commit, or
publication action.

### Limits and troubleshooting

Explorer responses are bounded projections. Coverage and truncation fields say
what the selected compiler analysis retained; they do not prove runtime
reachability, dynamic absence, cross-language indexing, Git history, or an
approval decision. A candidate view requires a compiler policy that admitted
candidate preparation. If a target is missing, use a stable declaration ID from
the checked project and regenerate after saving source. If an output cannot be
created, choose a new explicit path rather than overwriting a prior review.

The public `semaprax explore` command reads an authenticated manifest and writes an explicit, no-clobber output path. A source-free overview can be generated with:

```sh
semaprax explore ./semaprax.toml --format html --output ./semantic-explorer.html
semaprax explore ./semaprax.toml --target app.main --depth 2 --format svg --output ./app-main.svg
```

Open the HTML file locally or attach the SVG to a review manually. The HTML embeds its data and viewer assets, so viewing it does not require an editor, Node.js, or a local server. Markdown and JSON are also available through `--format markdown|json`; none of these commands posts to GitHub or uploads the output. Candidate review additionally requires both `--candidate-capsule` and `--expect-candidate`, and restores the capsule against the held project source before emitting base or candidate views.

The default snapshot contains compiler-owned declaration names, stable IDs, relative paths and structural relationships. These can be confidential even though full source bodies and capsule bytes are omitted. Detail absent from a snapshot is **not bundled**; absence is not a finding. `snapshot_digest` identifies canonical snapshot content for consistency checks, not external provenance, freshness or approval. HTML, JSON, Markdown and SVG remain display artifacts and cannot be replayed as candidate authority.

`--include-source` is an explicit source-inclusive mode for HTML and JSON only. It embeds the authenticated retained `source_files`; candidate exports also embed the existing immutable `source_review` report, including its source pairs and compiler-generated diff. The command still performs no source apply, candidate mutation, execution, approval or publication. Markdown and SVG reject `--include-source` because their formats do not carry the bounded source bundle.
