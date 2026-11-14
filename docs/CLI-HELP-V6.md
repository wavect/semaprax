# CLI Help v6: checked task/profile pages

Status: local task-page checks and offline discovery replay completed. #722's
matched live-agent experiment remains pending.

Audience: CLI users, coding agents, and compiler contributors.

This additive read-only form extends [V5](CLI-HELP-V5.md):

```text
semaprax help language author:tasks
semaprax help language author:task:<task>:<Project-profile>:<target>:<offset>
```

The closed pilot has four exact pairs:

| Task | Project profile | Target | Existing checked starter |
| --- | --- | --- | --- |
| `file-text` | `source-command.v1` | `native64` | `source-command-file-text` |
| `stdin-stream` | `language-command-io.stream-data.v1` | `native64` | `stdin-stream-data` |
| `stdin-json` | `language-command-io.collection-record.v1` | `native64` | neutral schema/codec example; admission/qualification pending |
| `selected-file` | `source-command.selected-file.resource-output.v1` | `native64` | neutral primitive example; admission/qualification pending |

Every other pair refuses with the exact admitted selector and links to existing
cards. This means outside this pilot, not unsupported by the language.
The `stdin-stream` starter still demonstrates transport/counting without JSON
imports. The distinct `stdin-json` route below supplies a neutral schema/codec example;
it does not promote the transport starter into a JSON application.
Generated-web guidance remains outside this pilot and adds no TeamDesk claim.

Each successful response is at most 2,048 UTF-8 bytes, with canonical unsigned
decimal offsets. The original two task routes repeat their complete prerequisite capsule:
exact template, table manifest, Project profile, input, selected target, complete
source-file inventory, required capabilities, declared manifest dependencies,
checked example selector and exact manifest/source hashes, status, next check,
test/build commands and exact-detail selectors. Capabilities must be both
declared and independently granted by ordinary execution. File-text tests use
`--target native`; stream-data's pure test closure retains its interpreter route.
Selected streaming command execution remains native-only.

Those original pages contain only whole declarations imported by that starter, sorted by exact
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
test binary is introduced. See the local results below.

Correctness does not close #722: latest-baseline discovery replay and at least
five fresh trials per arm with identical prompts/model/settings and unchanged
LogLens49/ShiftSim15 acceptance remain necessary. Report captured versus delivered
bytes/tokens, turns, total/net input, authored tokens, fixed harness context,
accepted-task conditional costs, wall time and unavailable billed cost as null.
No lookup bytes, static page length, token, RSS or application-fit saving is
claimed by this source change.


## Generic paired JSON task

`author:task:stdin-json:language-command-io.collection-record.v1:native64:0`
shows neutral request/response schemas, a complete generic manifest and codec
plan, a no-op command, primitive tests and explicit user-authoring steps. It
contains no benchmark solution, algorithm, source path or input-hash index.
The user must implement decoding, application policy, response construction,
encoding and meaningful tests. These minimal examples are not an application.

The full-host starter route preserves held-input/no-replace publication and
ordinary source-admission checks from [Application starter v1](APPLICATION-STARTER-V1.md).
Installation performs no native execution. Help never grants installation
or execution authority. V31 retains its old `test --target native` refusal;
`native-collection-record-pure.v1` is an explicit separate pure-test route.

Every JSON page repeats its profile, target, input, capabilities and pending
qualification. Whole workflow, prerequisites, plan, generic file contents and
exact dependency-catalog entries are paginated without truncation. Complete
identity validation precedes pagination; stale, duplicate and foreign selectors
refuse. The original generic file-text and stdin-stream scaffolds stay unchanged.
Exact/one-short/one-extra tests cover every entry, including continuation bytes.

The test-only descriptor producer compares profile/input/capability/dependency
fields with the internal retained fixture, derives the actual bundled dependency
closure and checks neutral sources through ordinary parsing/HIR. The existing
full historical starter admission owner remains independent. A separate owning
control checks both neutral Projects, derives the generic plan and independently
replays the complete ordinary codec candidate. Both neutral Projects and the
independent plan replay passed locally. Runnable application qualification
remains separate.

## Generic selected-file task

`author:task:selected-file:source-command.selected-file.resource-output.v1:native64:0`
shows a neutral complete manifest, minimal main/test modules and user-authoring
steps. It introduces no template or application solution. Runtime metadata and
pages contain no benchmark names, paths, solution identities or input hashes.
The test-only descriptor owner checks the neutral manifest and both modules
through ordinary parsing, HIR resolution and validation, and compares exact
profile/input/capability fields with the retained selected-file fixture. These
primitive no-op examples do not establish useful behavior or executable admission.

Every page repeats the v34 profile, native64 Unix restriction, input and four
capabilities, invocation-selection/descriptor lifetime rules, unchanged argv,
file, attempt, reserved-read and output bounds, and ordinary-admission caveat.
Whole-entry pagination refuses a budget that cannot advance. Strict canonical
continuation and all-identity validation precede pagination; help stays inert.
The v1 file-text scaffold, v33 profile and existing full/card/library routes stay
unchanged. Actual selected-file application qualification remains a separate
gate, as do the five fresh trials per arm required by #722.

## Local discovery replay

The macOS arm64 `semaprax` and `semaprax-full` builds from `c28cc4432`
returned identical, deterministic stdout for each command below. Both CLI
harnesses passed all three task-page checks. The neutral Project admission
and codec replay also passed with a canonical temporary parent, which avoids
the macOS `/var` symlink while preserving Project's symlink refusal.

| Help command or route | Responses | Total UTF-8 stdout bytes |
| --- | ---: | ---: |
| `help language` | 1 | 1,580 |
| `help language all` | 1 | 82,160 |
| `help library` | 1 | 925 |
| `help library all` | 1 | 259,193 |
| `help language author:routes` | 1 | 2,039 |
| `help language author:tasks` | 1 | 673 |
| Selected-file task, all continuation pages | 3 | 5,263 |
| Paired stdin JSON task, all continuation pages | 44 | 78,536 |

The selected-file first page is 1,622 bytes and gives the invocation prefix,
capabilities, bounds and manifest. The paired JSON first page is 1,654 bytes
and gives the plan installation, check, native test, build and run commands.
The full JSON sequence requires 44 requests when followed exhaustively. Every
page was at most 2,048 bytes. Each binary was run twice per command in an
empty directory, which remained empty. The binaries' SHA-256 digests were
`de70b5ec8e087ec4e718e6ecd14d26a873a3721cb7c309ee70fdf97528754e08`
for `semaprax` and
`f12a5d60d6efe4065c9ad6885eb75d438aa8d521db18e0a8bb08b320f47e5f26`
for `semaprax-full`.

This replay compares local command output for the discovery questions.
Actual delivered tool responses, model requests, turns, usage, billing and
task acceptance still require the matched LogLens and ShiftSim trials.

The standalone help harness subsequently passed all 13 checks after the
byte-partition build and run forms were moved into the shared command catalog.
The exhaustive catalog and all scoped help spellings now derive those forms
from the same entries. The owning historical catalog comparison passed with
both older snapshots preserved. The local CLI was relinked against the
retained compiler library for this CLI-only change.
