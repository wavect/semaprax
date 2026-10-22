# Independent Main44 source and receiver custody

Incoming subject: `44f52ebee749b2877ba001c5398acbf803a277a1`.

`compact-inverse7.json` and `receiver-inverse12.json` are the complete incoming
arrays, byte for byte. The corresponding existing 3220/16c/a3/e47 domains remain
separate. New `source_custody_main44` and `semantic_receivers_main44` modules
execute every original predicate against these exact incoming arrays, through
a sealed length/SHA reader and a separate whole-source input projection.

`path_array_main44` compiles the complete incoming path array, vector, sequencing
minimum, predecessor, and test receivers under their old borrowed budget and
Candidate APIs. The prefix helper binds to the unchanged Main8a helper. Changes
to copied receivers only bind namespaces, relocate source includes, or set the
local HIR selection flag. The complete indexed HIR receiver sets that flag, so
both direct indexed-HIR and ordinary HIR calls re-enter the retained sequence.
Shipping replay owns a separate funded lexical minimum implementation.

Frozen source includes are complete incoming files in `raw/`. Every one is
bound to current complete source bytes by `current-to-main44.json`; relocated
old vector paths have explicit physical bindings in `physical-paths.json`.
The new `source_domain` Rust guard checks every complete preimage and refuses
physical source drift. `namespace-bindings.json` retains original and adapted
source bytes for each copied file. New sources are joined into all three
HIR/workspace/interop authority inventories, as listed in `new-source-inventory.json`.

After the parent integrates all remaining changes, refresh this domain once:

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 \
  docs/evidence/main44-independent-custody-20261016/refresh-and-prove.py --refresh \
  > docs/evidence/main44-independent-custody-20261016/static-proof.json
```

Without `--refresh` the command is read-only and refuses source drift. It proves
all 7 and 12 original inverses and archives, the complete incoming source views,
namespace source bindings, and all three inventory memberships. The parent's
separate final endpoint refresh must also bind its existing 3220/16c/a3/e47 and
later domains. This evidence records static checks only; no compiler, build,
Rust tests, browser tests, or Clippy ran in this lane.

Test roots added under `--lib`:

- `cleanup_plan::replay::path_array_main44::sequence::tests`
- `cleanup_plan::replay::path_array_main44::sequence_vector::tests`
- `cleanup_plan::replay::path_array_main44::tests`
- `cleanup_plan::replay::path_array_main44::completed_tests`
- `cleanup_plan::replay::path_array_main44::source_domain`
- `cleanup_plan::replay::compact_stage::source_custody_main44`
- `source_reflows::semantic_receivers_main44`
