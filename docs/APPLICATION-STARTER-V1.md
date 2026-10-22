# Fresh application starter v1

Audience: compiler contributors and reviewers.

Status: source implementation; executable qualification pending. This does not
qualify a runnable typed application while ordinary compiler admission refuses
it (including the currently open source-capacity G171808 dependency).

The full host composes existing checked codec plans with an explicit command:

```sh
semaprax-full new fresh-app --from authored-project --codec-plan codec-plan.json \
  --command-profile language-command-io.collection-record.v1 \
  --command-path src/app.spx --command-source authored-project/src/app.command.spx
cd fresh-app
semaprax check .
semaprax test . --target native-collection-record-pure.v1
semaprax build . --target native -o app
./app < request.json
```

The plan is the unchanged [codec plan v1](APPLICATION-JSON-CODEC-PLAN-V1.md),
including its exact optional expected Project revision, bounds and overlapping
selector refusals. Its decoder and profile selector are shared with json-codec;
there is one generator and one regeneration replay. The private starter path first derives
unverified source/receipt drafts twice, compares every field and complete source
byte, releases the second draft, then builds one complete codec candidate and
performs both ordinary entry/test HIR validations. A draft carries no checked
candidate revision, HIR, publication operation or authority. Public codec-plan
derive/verify APIs retain independent checked replay. A successful derivation
replays the whole bundle, activates the explicitly supplied command source at
one existing manifest path, formats all authored modules canonically, then
rebuilds and independently validates the complete candidate through ordinary
Project/source/HIR/profile admission. Command activation cannot overlap a codec
replacement. It does not select a new manifest, target, dependency or capability.
Formatting can remove comments in this fresh canonical projection; exact original
source remains in the receipt's codec bundle. Original files are never rewritten.

`--command-path` and `--command-source` appear together or neither appears.
The latter is an explicit bounded regular non-symlink UTF-8 file, at most 131072
bytes. The plan is at most 16384 bytes. Both selected input files are opened through the existing held-directory bounded
regular-file owner: final-component no-follow, nonblocking Unix classification,
opened-handle regular-file authentication and exact named-handle rechecks. Early
path metadata is only a convenience refusal and never establishes authority.
Both byte sequences are independently rechecked before staging. The authenticated Project
owner independently performs its ordinary final input recheck. These operations
bind one held invocation revision; subsequent edits cannot mutate returned bytes.

Selected plan and command-source paths may be absolute or ordinary relative
paths from the invocation's working directory. The full host resolves each
spelling once before its first read and reuses it for the prepublication recheck.
This is absolute path resolution, not symlink canonicalization: the existing
held parent walk still rejects symlinks and parent-traversal components, and
the final regular-file and named-handle checks remain required.

The closed admitted command profiles are `source-command.v1`,
`source-command.resource-output.v1`, `source-command.selected-file.v1`,
`source-command.selected-file.resource-output.v1` and
`language-command-io.collection-record.v1`. The caller's selection must exactly
match the authenticated manifest. Unknown profiles and incomplete or duplicate
options refuse before reading inputs. Existing single-codec, bundle and template
creation routes retain their contracts and frozen capsule bytes.

The starter admits 1..32 sorted unique direct `src/*.spx` authored paths and at
most 2 MiB combined canonical source. It copies only manifest-authored modules;
compiler-bundled dependencies retain their original checked bindings rather than
becoming copied input files. External dependency subjects, Rust packages and law
sources refuse because their complete installation contracts are outside v1. There are exactly three root files, in existing
scaffold order: README.md, AGENTS.md, semaprax.toml, followed by canonical sources
in path order. README contains exact check, declared native test, native build and
run routes. Selected-file routes explicitly retain the original argument-index
handshake; the selected application argument is index zero. A caller whose file
argument occupies another slot must select that index explicitly. No launcher
silently reparses application flags. Other commands keep their own original argv
or stdin protocol.

README embeds compact deterministic `semaprax.application-starter.v1` receipt
JSON (at most 12 MiB): exact input plan and its SHA-256, the complete replayed
codec bundle including original schemas/sources, optional exact command activation
and its SHA-256, final Project/workspace revisions, canonical manifest hash and
every final authored source path/hash, plus exact command routes. The bundle
revision is the pre-activation candidate; the receipt's final revision is the
complete activated/formatted generation. Neither is silently substituted for
the other. Receipts are evidence and confer no authority.

Only the full host exposes installation. Its existing scaffold held-parent
publisher acquires a fresh same-parent private stage, writes create-new files,
authenticates the complete exact held inventory, and performs its existing
no-replace directory pivot and postpublication authentication. Runtime source
names have owned bounded inventory storage; old static inventories retain their
original name allocation behavior. Existing destinations, symlinks, invalid
names, racing creators, changed parents/stages and surplus inventory refuse.
Ordinary prepublication failures discard only proven-owned staged files. A
failed pivot after child handles settle can leave inert staging residue; loss
of binding after a successful pivot reports uncertainty and preserves both trees.
No failed operation gains overwrite or recursive cleanup authority. The original
scaffold failure selection and uncertainty behavior remain authoritative.

No managed apply is exposed. There is no ACTIVE pivot, in-place source mutation,
Git/editor/raw-path transaction guarantee, dependency download, native compilation
or application execution during installation. Later check/test/build/execute are
separate explicit host operations. A source-valid tree is not executable evidence.
This changes starter/profile/prompt conditions; no command, token, time or numeric
efficiency improvement is claimed without later matched trials.

The v31 declared native test route is the explicit
[collection-record pure test successor](PROJECT-NATIVE-TEST-COLLECTION-RECORD-PURE-V1.md).
Other admitted source-command starters select the existing `--target native`.
The old v31 native-target refusal remains unchanged. Typed application source
and backend limits still fail closed before publication: this starter does not
repair or suppress G171808, grant helpers special admission, or claim ShiftSim15
passage before the real owning gate succeeds.

Owning pending checks:

- `cli_new_project_v1 application_starter::application_starter_typed_shiftsim_fresh_check_native_tests_and_original_fifteen`
  creates a fresh typed request/decision/response application, checks its
  canonical inventory and graph-bound receipt, executes the explicit native
  tests/build and all original fifteen acceptance inputs. Its current compiler
  dependency is a real failure, not a skipped or weakened expectation.
- `cli_new_project_v1 application_starter::application_starter_selected_file_declared_native_tests_and_unchanged_cli49`
  creates a selected-file project and executes declared native tests and the
  unchanged independent CLI49 receiver. The receiver is test input, not an
  installed product capability.
- `cli_new_project_v1 new_project::application::tests::` owns partial writes,
  old-source preservation, existing/symlink destinations/parents and malformed
  options. Its deterministic plan/activation swap controls replace an inspected
  regular input with a symlink or FIFO before the held open; both refuse before
  staging and preserve external/authored files. `project_publication::application::tests::` owns exact arities/path
  refusals. Existing scaffold physical namespace-race cases remain required.
- `project standard_library::application_json::starter::` and existing plan
  decoder/overlap/stale/source/bounds/receipt gates retain closed decoding,
  canonical replay and no authority escalation. Existing native test old-profile,
  result/timeout/output/settlement known answers remain required.

No Cargo build, tests, Clippy, benchmark, hosted run or full gate was performed
when authoring this source batch. The user prohibited full gates. Minimum exact
owning checks are pending the parent's grouped compilation/verification budget.

## Starter invocation recovery (#837)

Source implementation; focused executable qualification remains pending.
Each explicit starter option (`--from`, `--codec-plan`, `--command-profile`,
`--command-path`, or `--command-source`) selects the full host's closed starter
invocation grammar. A request missing its plan or profile reports that complete
grammar with status two before selected-input reads or destination publication.
The ordinary template grammar remains selected when no starter option appears.

The standalone compiler recognizes these reserved starter options only to refuse
with status two and the exact next command `semaprax-full help new`; it gains no
installer or private host authority. Earlier ordinary argument errors remain
selected by its left-to-right parser. This guidance is not successful installation,
ordinary compiler admission, CLI49/ShiftSim15 execution or #837 closure evidence.
