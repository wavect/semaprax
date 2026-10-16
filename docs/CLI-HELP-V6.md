# CLI Help v6: checked task/profile pages

Status: source implementation; focused executable qualification and #722's
matched live-agent experiment remain pending.

Audience: CLI users, coding agents, and compiler contributors.

This additive read-only form extends [V5](CLI-HELP-V5.md):

```text
semaprax help language author:tasks
semaprax help language author:task:<task>:<Project-profile>:<target>:<offset>
```

The closed pilot has two exact pairs:

| Task | Project profile | Target | Existing checked starter |
| --- | --- | --- | --- |
| `file-text` | `source-command.v1` | `native64` | `source-command-file-text` |
| `stdin-stream` | `language-command-io.stream-data.v1` | `native64` | `stdin-stream-data` |

Every other pair refuses with the exact admitted selector and links to existing
cards. This means outside this pilot, not unsupported by the language. The
`stdin-json` task explicitly refuses: the checked stdin starter demonstrates
transport and counting and has no JSON dependency/import. V5's `author:stdin-json`
and codec cards remain available for separate prerequisites; they are not a
checked composed JSON application. Generated-web task guidance is outside this
pilot and adds no claim about the separate TeamDesk acceptance gate.

Each successful response is at most 2,048 UTF-8 bytes, with canonical unsigned
decimal offsets. Every page repeats the complete route prerequisite capsule:
exact template, table manifest, Project profile, input, selected target, complete
source-file inventory, required capabilities, declared manifest dependencies,
checked example selector and exact manifest/source hashes, status, next check,
test/build commands and exact-detail selectors. Capabilities must be both
declared and independently granted by ordinary execution. File-text tests use
`--target native`; stream-data's pure test closure retains its interpreter route.
Selected streaming command execution remains native-only.

Pages contain only whole declarations imported by that starter, sorted by exact
stable identity. Each declaration repeats its library dependency, library
required-profile, targets, status, complete documentation/signature, effects and
contract counts, and exact lookup selector. Library required-profile is a
separate role from the consumer Project profile; matching strings or keyword
hits never authorize composition. In particular decimal preconditions remain
visible: the starter's input must satisfy its decimal contracts. A page cannot
silently truncate a prerequisite or declaration. If one capsule plus one row
cannot fit, the response refuses with its exact library selector. All descriptor
identities are resolved before pagination, so a stale later identity cannot hide
behind a successful first page. Duplicate/stale identities refuse.

The footer says complete checked-starter imported subset or gives the exact next
selector. This is not all declarations compatible with a profile. The stdin
transport example has zero external library imports; offset zero returns its
complete capsule and explicit transport-only coverage. Nonzero offsets refuse.
All catalog declarations remain reachable through unchanged full/exact lookup
and V5 keyword search. All 34 old cards, full-reference/catalog access, scaffold
v2-v8 capsules and interfaces remain unchanged. The route index adds only one
`author:tasks` link, staying within its existing byte limit. V8 already points to
that index, so no existing capsule is silently rewritten.

`src/cli/help/authoring/tasks.rs` owns selection and rendering. Its embedded
`tasks/catalog.json` is derived from the existing table-layout scaffold with the
fixed name `task-demo`, not user paths or free text. The descriptor regression
rederives the ordinary checked scaffold, parses its actual manifest and source
imports, and compares complete hashes/fields. The existing ignored
`standard_library::regenerate_catalogs` owner also regenerates this descriptor;
new source/template changes must update the generated descriptor and pass its
ordinary comparison. Exact declaration content comes from the existing generated
`std/catalog.json`, whose original catalog producer/gates remain unchanged.
Help parses only these embedded descriptors/catalogs; it never builds or compiles
source, reads user files/environment/PATH, writes output files, calls a host hook,
or grants a target/capability/provider authority.

The existing standalone/full CLI help harnesses share one new task-page owner
covering successful deterministic continuation, all descriptor imports,
malformed/profile-target refusal and inert empty-directory execution. The new
unit owners cover complete descriptor derivation, all imported rows, stale IDs,
whole repeated capsules, exact/one-short/one-extra byte bounds and empty subsets.
Existing help/search/catalog/scaffold gates remain required; no new top-level
test binary is introduced. No gate was executed during source authoring.

Correctness does not close #722: latest-baseline discovery replay and at least
five fresh trials per arm with identical prompts/model/settings and unchanged
LogLens49/ShiftSim15 acceptance remain necessary. Report captured versus delivered
bytes/tokens, turns, total/net input, authored tokens, fixed harness context,
accepted-task conditional costs, wall time and unavailable billed cost as null.
No lookup bytes, static page length, token, RSS or application-fit saving is
claimed by this source change.
