# Harness skills v1 (HP-13, HN-03, HN-19, HN-04, HN-06)

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

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
| Agent Skills (Markdown) | directory with `SKILL.md`; YAML front matter parsed by the bounded `yaml-rust2` profile (below); legacy keys `version`, `tags: [a, b]`, `dependencies: [..]` stay readable | supported |
| Manifest | `skill-bundle.json`, schema `semaprax.skill-bundle.v1`: `name`, `version`, `license`, `tags`, `entries: [{file, digest}]`; entry digests verified | supported |
| Cursor rules, MCP prompts, VS Code extensions | `.cursorrules`/`.cursor`, `mcp-prompts.json`, `extension.vsixmanifest` | **unsupported** (`SPX-HPM008`) |

Repository instruction files (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
`.cursorrules`) never register as skills or policy (`SPX-HPM005`).

The adopted bundle is `packages/semaprax-harness-adapters/skills/reuse-before-generation/`.

## Agent Skills profile (HN-03)

`skills::agentskills::parse` reads front matter with `skills::yaml` (event-pull
over `yaml-rust2`, no in-house YAML). Folded (`>`), literal (`|`), quoted
scalars and comments work. Fields: `name` (required, 1-64, lowercase), `description`
(required, 1-1024 chars), `license`, `compatibility` (1-500), `metadata` (string
map), `allowed-tools`. Ecosystem keys (`argument-hint`, `hooks`, `model`, ...)
and namespaced keys (`x-*`, `a.b`, `a:b`) are kept as inert JSON in
`SkillEntry.extensions`; any other key is `SPX-HPM001` (specific, never a
silent grant). `allowed-tools` and `hooks` become `SkillEntry.requested`
(`tool:<pattern>`, `hook:declared`): requests needing host authorization; nothing
is executed, installed or granted. The normalized view is derived and versioned
(`semaprax.skill-view.v1`); the original bytes are never rewritten (snapshots
copy them exactly). The pre-HN-03 parser survives as `skills::legacy` (reference
and regression pin: it refuses the official Ponytail/Caveman files).

Hard bounds: front matter 16 KiB, depth 6, 512 nodes. Refused: aliases, anchors,
tags (`SPX-HPM030`), non-scalar keys, multiple documents, duplicate keys
(`SPX-HPM032`), bound violations (`SPX-HPM031`), malformed UTF-8 (`SPX-HPM001`).
Pinned official fixtures (byte-exact, `PROVENANCE.json`) live in
`crates/semaprax-harness/tests/fixtures/skills/official/`.

## Resources (progressive, exact-digest)

Every file of a bundle is inventoried (`SkillEntry.resources`: path, kind,
bytes, sha256): `references/`, `assets/` are `reference-asset`, `scripts/` is
`executable-script`, the rest `passive-text`. `skill.catalog/v1` `load` takes an
optional `resource: {path, digest}`; the result carries `artifact_refs:
[{path, digest}]` and the quoted `text`. The resource must be in the inventory,
match its digest exactly, be UTF-8 text without NUL, and fit
`max_resource_bytes` (default 64 KiB); scripts are refused (`SPX-HPM033`) and
never run. `list` exposes no body or resource text. A resource presentation is
charged once per (skill digest, path): `ResourceLoad.charged_bytes` is the
framed length the first time and 0 afterwards; `resource_bytes_charged()` sums.

## Identity (HN-19)

`SkillEntry.digest` is the artifact-v2 digest over the whole directory (see
[HARNESS-ARTIFACT-IDENTITY-V1](HARNESS-ARTIFACT-IDENTITY-V1.md)); the old digest
is kept as `legacy_digest` (label `legacy-v1`). An `approved_digest` may be the
legacy one only when that digest covers every file of the bundle; otherwise
`SPX-HPM012`. `with_snapshot_store(dir)` + `activate(digest)` extract an immutable
snapshot; a session loads only from its snapshot and `activate`/`drift` report
source drift.

## Behavior

- Per skill: origin, license, version, digest (artifact-v2 `sha256:<hex>` over every file's path, kind and bytes, plus the labelled `legacy-v1` digest), tags, dependencies, byte and word size.
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
dependency; 012 approved-digest mismatch; 020-023 flagged text (warnings); 030 unsafe YAML construct (alias, anchor, tag, complex key,
multi-document); 031 YAML bound exceeded; 032 duplicate front-matter key; 033 resource refused;
034 unsafe or colliding artifact path (symlink, hardlink, traversal, case collision, bounds, closure rules);
035 snapshot store invalid or corrupt; 036 reserved for drift reports.

## CLI

`skills [list|load <digest>] --root <abs-dir>... [--tags t1,t2] [--select a,b] [--max-bytes N] [--json]`

## Workflow integration

`semaprax harness adopt --skills <abs-dir> [--origin label]` approves a machine-local root (refused inside the
project, `SPX-HPB024`). `skills::PlainSkills` is the builtin `semaprax/plain-skills` provider: `list`/`load`
payloads from `SkillService` satisfy the `skill.catalog` contract validators (tested). The workflow renders the
prompt for the task family tags and counts `model_visible_bytes`.

## Official default skills (HN-04)

`packages/semaprax-harness-adapters/skills/catalog.json` is the curated first-party catalog
(`semaprax.curated-skill-catalog.v1`): canonical id, aliases, official repo, subpath, tracked channel
(`latest-stable`), resolved tag and commit, per-file upstream git blob and sha256, license, compatibility status,
authorship label (`upstream-authored` / `semaprax-authored`), applicability metadata and declared features. Seeded from
`DietrichGebert/ponytail` v4.10.3 (`ef8ca48f`) and `JuliusBrussee/caveman` v3.1.0 (`8af1f1b9`), both re-resolved as the
latest stable on 2026-10-04. The byte-exact files and LICENSE live under `skills/official/<id>/<version>/` and are
compiled into the binary (`include_bytes!`, `skills::official`); discovery and loading never read a repository
checkout or the network. Files are verified against the recorded sha256 and materialized into the content-addressed
store `<harness_home>/artifacts/<hex>/` on first use. `reuse-before-generation` keeps its name, labelled
`semaprax-authored`, and is adopted through an approved root, not embedded.

Without `--root`, `skills` serves the curated set: `list`, `load <id|alias|digest> [--resource PATH]`,
`use <name> [mode]`, `status`, `off <name|all>` with `--scope session|project|user`, `--project`, `--session`,
`--family`, `--instruction`, `--max-bytes`, `--json`. Aliases `ponytail` and `caveman`. Upstream text is rendered as
quoted data with the existing precedence header; the Semaprax `HOST POLICY` block follows it separately
(`skills::frame`). Upstream bodies are never edited.

Features outside the primary skill are separately declared and `unsupported`: Ponytail review/audit/debt/gain/help
skills and hooks; Caveman `ultracave`, `megacave` (wenyan), `caveman-compress`, proxy and hooks. They are refused with
`SPX-HPM039`, never emulated, and never a silent language switch. Supported: Ponytail `lite|full|ultra` (declared in
its Intensity table) and Caveman `on|off|status` (status is answered by the host).

The one switch: `[skills] official = false` in `semaprax.harness.toml`, or `skills off all --scope project|user`
(`skills use all --scope ...` re-enables). Explicit use/load while off is `SPX-HPM042`. A session cannot flip it.
A project root whose bundle claims an official id or alias with different bytes is refused whole
(`DefaultSkills::refuse_shadowing`, `SPX-HPM040`); identical official bytes are not a shadow.

States are reported separately: `default_available`, `selected`, `loaded` (host read and framed the body),
`applied_to_model` (only set by `mark_applied` after a model request consumed it), `omitted` (`not-requested`,
`not-applicable-to-task-family`, `content-budget`, `unavailable`) and `disabled` (`official-skills-switch-off:<layer>`,
`mode-off:<layer>`).

## Task-aware selection and scoped modes (HN-06)

`DefaultSkills::select_for_task(&TaskInput{family, instruction}, max_bytes)` makes zero model calls
(`selection_model_calls == 0`). Inputs: curated applicability (outside the upstream files), task family, the explicit
instruction (`/ponytail [lite|full|ultra|status]`, `/caveman [status]`, `stop <skill>`, `normal mode`, trigger phrases
from the catalog), preset layers and the byte budget. Coding families select Ponytail (shipped mode `full`);
prose/translation/docs do not; Caveman is available but selected only by explicit request or the
`efficient-coding` preset (`[skills] preset`), and only for coding families unless explicitly requested. Only selected
bodies enter the prompt; unselected skills contribute no bytes.

Precedence: explicit current instruction > session override > project preset (`semaprax.harness.toml` `[skills]`
`preset|ponytail|caveman`, overlaid by `skills use --scope project`) > user preset
(`<home>/skills/user.json`) > shipped recommendation. Host policy outranks skill wording. State is per project and
session: `<home>/skills/state/<project>/sessions/<session>.json` (`semaprax.skill-session.v1`: revision, overrides,
locked `(digest, version)` per skill, active modes, applied) and `<home>/skills/state/<project>/project.json`; there is no global mode
file. A session locks the revision it first used and keeps loading it from the snapshot store if the catalog moves.
A stored preference is not proof a mode was active: `Caveman mode: <on|off|unknown>` comes from session facts only.
Corrupt state is refused (`SPX-HPM041`), never reset. A stop (`off`) is final for the session whatever the upstream
persistence text says.

Workflow integration call (for `workflow::skill_prompt`): build `DefaultSkills::embedded(env.harness_home.clone(),
&project_id, &session_id)?.with_project_prefs(config.skills.prefs.clone())?`, call
`select_for_task(&TaskInput{family: &task.family, instruction: Some(&task.goal)}, max_bytes)`, put `.text` in the
prompt once, then after the request is sent call `mark_applied(&loaded_ids)`. Run approved extra roots through
`refuse_shadowing` before `SkillService::render_prompt`.

Diagnostics: 037 catalog or embedded asset invalid; 038 unknown curated skill; 039 unsupported mode or variant;
040 official id shadowed; 041 state or home problem; 042 official skills disabled.

No claim is made that a prompt guarantees model behavior: tests verify selection, framing and explicit state, not
observed model output.

### Updated revisions in the workflow (HN-05)

The workflow constructs `DefaultSkills` from `updates::effective_set`, not the embedded set. Revision locks of a
session id persist; `harness run` drops the `default` session's locks that no longer match the effective set at
session start.
