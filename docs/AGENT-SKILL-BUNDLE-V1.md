# Agent Skill Bundle v1

Status: **LOCAL** bounded implementation with an executable reference,
regression corpus, and a pinned drift gate, implemented in
`src/agent_skill_bundle.rs`. This is issue #196
("Generate a version-matched Agent Skill bundle and simplify the public
semantic workflow"). It is the second consumer of the shared
`SEMANTIC-DISCOVERY` package foundation (issue #125,
[Semantic Discovery v1](SEMANTIC-DISCOVERY-V1.md)): it composes that module's
operation catalog rather than restating it.

Audience: coding agents and SDK/MCP client authors who need one small,
version-matched description of the exact supported workflow, without reading
this repository, plus compiler contributors extending the bundle.

## Why a bundle instead of a hand-written skill file

A hand-authored Markdown or JSON "skill" file for an agent tool drifts from
the compiler the moment either changes independently. This issue's whole
point is that the bundle must be **generated** from the installed compiler's
real, compiled-in capabilities and must **fail closed** the moment it no
longer matches them — not describe a workflow the installed compiler cannot
actually perform.

`generate_agent_skill_bundle()` is a pure function of the compiled-in
toolchain: `CARGO_PKG_VERSION`, the optional `SEMAPRAX_BUILD_COMMIT`, the
live [`installed_query_capabilities`](INSTALLED-AGENT-GUIDANCE-V1.md) and
[installed diagnostic catalog](INSTALLED-DIAGNOSTICS-V1.md) digests, the
bundled `std/catalog.json` bytes, and the closed
[`SEMANTIC_DISCOVERY_OPERATIONS`](SEMANTIC-DISCOVERY-V1.md) catalog. It takes
no source file: unlike `semantic_discovery::generate_discovery_manifest`,
which binds one module's exact revision, this bundle describes the
**installed compiler itself**, not any one `.spx` file, so there is nothing
to bind to a file revision. Two calls in the same build always produce
byte-identical output (`tests::generation_is_byte_identical_on_repetition`).

## The envelope: `semaprax.agent-skill.v1`

```json
{
  "schema": "semaprax.agent-skill.v1",
  "digest": "sha256:...",
  "bytes": 1234,
  "payload": {
    "schema": "semaprax.agent-skill.v1",
    "authority": false,
    "compiler": {"package": "semaprax", "version": "0.4.1", "build_commit": null, "binary_identity_claimed": false},
    "authority_classes": ["candidate_only", "publication", "read_only", "source_write", "test_execute"],
    "target_profiles": ["core-wasm", "interpreter", "native-c11"],
    "package_status": {"schema": "semaprax.standard-library-catalog.v1", "digest": "sha256:...", "module_count": 38},
    "discovery": {"schema": "semaprax.semantic-discovery.v1", "operations": ["... the same closed read-only catalog Semantic Discovery v1 renders, including this bundle's own agent_skill entry ..."]},
    "public_workflow": ["... ten entries, sorted by verb; see below ..."],
    "known_limitations": ["..."]
  }
}
```

The envelope wrapper (`schema`/`digest`/`bytes`/`payload`) and its
domain-separated SHA-256 digest follow the exact pattern used by
`capability_manifest`, `region_report`, `installed_guidance`, and
`semantic_discovery`: the digest binds the EXACT payload byte sequence
between the `"payload":` key and the closing `}`, and `bytes` is that
substring's length. `authority` is always `false`: this document is
descriptive data, never a capability grant, exactly like
`installed_query_capabilities` and every other installed-guidance envelope.

`compiler` is `installed_guidance::compiler()`, shared verbatim (not
re-derived) with `installed_skill`. `package_status` reports the bundled
standard-library catalog's own `schema` field, its module count, and a
domain-separated digest of the exact committed `std/catalog.json` bytes; it
is not a live package-registry inventory (`known_limitations` says so
explicitly). `discovery.operations` is exactly what
`semantic_discovery::render_operations_catalog` renders for the live
`installed_query_capabilities`/`installed_diagnostic_catalog` digests —
composed by calling that function, never copied by hand.

## Version binding and negotiation

"Version-matched" means: the bundle is bound to `CARGO_PKG_VERSION` plus the
optional `SEMAPRAX_BUILD_COMMIT` build metadata inside `compiler`, exactly as
`installed_guidance::compiler()` already validates it (a present build commit
must be exactly 40 lowercase hex characters or generation fails closed).
There is no separate "bundle version" field to fall out of sync with the
compiler that produced it.

`negotiate_agent_skill_schema(requested: &str)` lets an SDK/MCP client assert
the exact schema it was built against before trusting anything else in a
fetched bundle:

- `requested == "semaprax.agent-skill.v1"` → `Ok(())`.
- anything else → `Err` (`SPX-G587`), with **no compatibility fallback**. A
  client that asked for a schema this compiler does not serve gets a hard
  error naming both schemas, never a silent downgrade to an older shape. This
  directly answers one of issue #196's named failure cases: "Version
  negotiation can silently fall back to an older schema."

## The public semantic workflow

`PUBLIC_WORKFLOW` is the small, closed, ten-verb vocabulary issue #196
requires: `apply`, `context`, `impact`, `inspect`, `propose`, `publish`,
`rebase`, `repair`, `review`, `test`. Sorted by verb; the sort is enforced by
`tests::public_workflow_is_sorted_by_verb_and_covers_the_required_ten_verbs`,
which also pins that this is EXACTLY that set — no more, no fewer.

Each entry states:

| Field | Meaning |
|---|---|
| `verb` | The public name. |
| `authority_class` | Exactly one of `read_only`, `candidate_only`, `source_write`, `test_execute`, `publication` — the five classes issue #196 requires every command to state. |
| `cli_command` | The EXISTING top-level `semaprax` command this verb wraps. |
| `cli_usage` | That command's admitted invocation shape, copied from its `cli::help::COMMANDS` entry. |
| `note` | One sentence on what the verb does and does not authorize. |

| Verb | Authority class | Wraps |
|---|---|---|
| `inspect` | `read_only` | `graph` |
| `context` | `read_only` | `context` |
| `propose` | `candidate_only` | `change preview` |
| `impact` | `read_only` | `impact` |
| `apply` | `source_write` | `apply-semantic-workspace-change-evidence` |
| `repair` | `source_write` | `repair` |
| `rebase` | `candidate_only` | `change rebase` |
| `review` | `read_only` | `review` |
| `test` | `test_execute` | `test` |
| `publish` | `publication` | `project-candidate-git-publish` |

**This is a naming and authority-labeling layer, not a new command surface.**
Every `cli_command` names a command that already exists, is already
catalogued in `cli::help::COMMANDS`, and keeps its own existing grammar,
diagnostics, and semantics unchanged. `PUBLIC_WORKFLOW` adds no new parser,
no new dispatch arm, and no new authority: it is exactly the "public CLI may
wrap internal commands but must not fork their semantics" the issue asks
for. Nothing here narrows or hides the wrapped commands' own broader
grammar (multiple `change` subcommands, multiple `query` operations, and so
on remain fully available); this table only picks the ten operations an
agent following the minimal workflow needs and gives each one memorable name
plus its authority class.

**Authority transitions stay explicit.** No verb here performs more than one
authority class's worth of effect: `propose`/`rebase` never write source or
publish, `apply`/`repair` never publish, and only `publish` performs
publication. An agent must call a distinct verb to cross each boundary,
matching the non-negotiable requirement that authority transitions remain
explicit and that combining preview and apply must never create an
authority escalation.

**The drift gate.** Because the CLI's canonical command catalog
(`cli::help::COMMANDS`) is compiled only into the `semaprax` **binary**, not
the library (`AGENTS.md`'s `--bins` vs. `--lib` distinction), the
authoritative check that every `cli_command` above still names a real,
catalogued command lives in the binary-side test
`cli::agent::tests::public_workflow_commands_are_all_catalogued`. The
lib-side `agent_skill_bundle::tests::every_public_workflow_command_names_an_
existing_top_level_cli_surface` only pins the small set of distinct command
names the table currently uses, so an edit to the table is visible in
review; it cannot by itself prove the binary still admits those commands.

## `semaprax agent skill`

```sh
semaprax agent skill
semaprax agent skill --require-schema semaprax.agent-skill.v1
```

Prints the exact envelope above. `--require-schema <schema>` calls
`negotiate_agent_skill_schema` first and fails closed (exit code 1, no
output) on any mismatch, before generating or printing anything — this is
the CLI-level hook an SDK/MCP client integration uses to refuse an
incompatible installed compiler rather than silently proceeding against
schema it does not understand. This subcommand adds no new top-level
command: it is one more admitted verb of the existing `agent` command
alongside `inspect`, `run`, and `replay`.

## Composing this module

This module is the second consumer of the `SEMANTIC-DISCOVERY` package
(issues #125, #196, #197, #200; see
[Semantic Discovery v1 § Composing this module](SEMANTIC-DISCOVERY-V1.md#composing-this-module)).
It calls `semantic_discovery::render_operations_catalog` directly rather than
re-deriving the operation list, and its own `agent_skill` catalog entry
(pointing at `AGENT_SKILL_SCHEMA`) is added to `SEMANTIC_DISCOVERY_OPERATIONS`
itself, so the closed catalog always lists this bundle too. It shares
`installed_guidance::compiler()` with `installed_skill` rather than
re-deriving compiler identity a second time.

Neither `generate_agent_skill_bundle` nor `negotiate_agent_skill_schema`
performs a host effect, opens a socket, or grants a capability.

## Diagnostics

| Code | Meaning |
|---|---|
| `SPX-G585` | The embedded standard-library catalog or another internal input is malformed. Indicates a compiler build defect, not a caller error. |
| `SPX-G586` | The bundle exceeded its bounded output budget (`MAX_AGENT_SKILL_BUNDLE_BYTES`). Fails closed; never truncated. |
| `SPX-G587` | `negotiate_agent_skill_schema` was asked for a schema this compiler does not serve. There is no fallback. |

## Non-goals

- This is not a second operation catalog. The read-only operation inventory
  embedded under `discovery` is exactly `semantic_discovery`'s catalog,
  composed by function call.
- `package_status` is not a live package registry, dependency resolver, or
  network-backed inventory; it reports the bundled standard-library catalog
  this compiler ships, nothing more.
- `PUBLIC_WORKFLOW` does not delete, rename, or change the grammar of any
  existing command. Internal protocol commands not listed here (workspace
  session routes, MCP transport, persistent service commands, generated
  clients, and so on) remain fully available to tool authors; this table is
  a minimal on-ramp, not the complete command surface.
- Generating this bundle is not a support, compatibility, or publication
  claim for any target profile it lists; `TARGET_PROFILES` states which
  backends a safe program has equivalent checked behavior on, not which
  hosted environment currently runs it.
