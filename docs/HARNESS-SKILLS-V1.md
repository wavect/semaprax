# Harness skills v1 (HP-13)

Audience: toolchain contributors and harness adapter authors.

Implements `skill.catalog/v1` of [HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md)
in `crates/semaprax-harness/src/skills/`. Diagnostics use letter `M`.

## Model

Skills are passive instruction data. Discovery reads only host-approved roots
(`ApprovedRoot { path, origin, approved_digest? }`); `list` returns bounded
metadata, `load(digest)` returns content by exact digest. Skills are off unless
`SkillCatalogConfig.enabled`; a disabled catalog is empty and not an error.

## Formats

| Format | Layout | Status |
|---|---|---|
| Markdown | directory with `SKILL.md`, front-matter keys `name`, `description`, `version`, `license`, `tags: [a, b]`, `dependencies: [..]` (strict subset; unknown or duplicate keys refused) | supported |
| Manifest | `skill-bundle.json`, schema `semaprax.skill-bundle.v1`: `name`, `version`, `license`, `tags`, `entries: [{file, digest}]`; entry digests verified | supported |
| Cursor rules, MCP prompts, VS Code extensions | `.cursorrules`/`.cursor`, `mcp-prompts.json`, `extension.vsixmanifest` | **unsupported** (`SPX-HPM008`) |

Repository instruction files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
`.cursorrules`) never register as skills or policy (`SPX-HPM005`).

The adopted bundle is `packages/semaprax-harness-adapters/skills/reuse-before-generation/`.

## Behavior

- Per skill: origin, license, version, digest (`sha256:<hex>` over SKILL.md or
  manifest plus entries, domain separated), tags, dependencies, byte and word size.
- Selection: task tags (`task_tags(kind)`) plus explicit names/digests; ties by
  name; at most 3 tag-selected. `Recommender` is a hook for future decision tasks.
- Budget: one `max_bytes` covers catalog descriptions (at most a quarter) and
  loaded content. Omissions are listed with `catalog-budget`, `content-budget`
  or `conflict`.
- Precedence: content is rendered as `> `-quoted data under a header stating that
  host/user policy and compiler invariants outrank it and that it grants no
  authority. Lines that disable verification, upload secrets, run installers or
  claim authority are flagged (`SPX-HPM020`..`023`), never obeyed.
- Scripts are never run. `scripts/*` and `tool:<name>` dependencies are
  `requires_host_tool` and unsatisfied unless the host lists them in `host_tools`
  (`SPX-HPM011`).
- Same name with different digests: all isolated, none selected by name
  (`SPX-HPM010`). Changed digest after listing: `SPX-HPM006`.
- Content cache key: (bundle digest, authorization digest of the approved roots).
- `model_visible_bytes` and the exact `text` are returned for the HP-15 observer.

## Diagnostics

HPM001 malformed SKILL.md/unreadable file; 002 bad manifest; 003 manifest entry
missing or digest mismatch; 004 bad approved root; 005 instruction file ignored;
006 stale digest; 007 unknown digest or skills disabled; 008 unsupported
ecosystem; 009 skill does not fit budget; 010 name conflict; 011 missing
dependency; 012 approved-digest mismatch; 020-023 flagged text (warnings).

## CLI

`skills [list|load <digest>] --root <abs-dir>... [--tags t1,t2] [--select a,b] [--max-bytes N] [--json]`

## Workflow integration

`semaprax harness adopt --skills <abs-dir> [--origin label]` approves a machine-local root (refused inside the
project, `SPX-HPB024`). `skills::PlainSkills` is the builtin `semaprax/plain-skills` provider: `list`/`load`
payloads from `SkillService` satisfy the `skill.catalog` contract validators (tested). The workflow renders the
prompt for the task family tags and counts `model_visible_bytes`.
