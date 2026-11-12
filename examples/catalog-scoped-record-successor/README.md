# Catalog typed successor

This is ordinary application source for the explicit
`language-command-io.collection-record.v1` **v31 successor**. The original
[Catalog contract](../../benchmarks/catalog-tokens-v1/SPEC.md) mandates v30;
this example is not original-v30 qualification. Its owning test retains every
one of the original 23 functional cases and all expected status/stdout/stderr
bytes, plus the original item/publication ownership requirements. Neither the
SPEC, corpus, independent oracle, TypeScript setup nor campaign seeds are changed.
Runtime qualification is pending; there is no live trial or efficiency claim.

The request schema derives an owned UTF-8 decoder with a 16-byte per-string
bound. Application code separately enforces nonempty ASCII identifiers, decoded
uniqueness and numeric domain ranges. Empty tags select all, every item is
validated before filtering, and raw JSON whitespace has no input byte cap.
Malformed JSON, unknown/duplicate/missing fields and incorrect scalar kinds are
the real generated scanner/decoder's responsibility, not a handwritten parser.

`InputItem` starts with ID so its stable sort makes duplicates adjacent without
an extra String inventory. Tags stay in their decoded `Vec<string>` and are
sorted directly. `vec_str_at` reads adjacent tags and selection prefixes;
`vec_field` reads existing item fields, and adjacent IDs reuse `std.text.equals`.
These views retain their original vector through their last use. Response
`Item` rows sort by explicit department/priority/ID/stock/mark keys, preserving
the required ID/department/priority/stock/mark JSON member order. The real
encoder preserves vector and metrics order. No pointer or token spelling grants
sorting authority. The retained Tag-wrapper sources remain differential controls
for all 23 cases. Both routes passed the unchanged application corpus.

The report is below 65536 bytes for every admitted 256-row/16-byte-ID value
(even a conservative 128 bytes per row plus 128 metrics bytes is smaller).
Its encoder preflight uses a 65535-byte limit, leaving room for one newline.
The command binds the whole owned `CatalogResult.report` and passes it directly
to the encoder in the valid branch. Invalid results settle the bound report
before returning the existing rejection. The recursive unpack and reconstruction
remain an independent 23-case differential control.
Ordered publication fragments own a String label and Bytes payload. The two
fragment allocations, stable sort, clone and replacement occur outside loops;
the existing Bytes/fuel/provider/ownership limits remain unchanged. Output
refusal or short provider writes are status 1, distinct from invalid input's
status 2 and exact `invalid catalog request\n` stderr line.

Install into a separate copy with a verified compiler. Keep `src/app.spx` as the
bootstrap until **both** complete-source derivations have been produced:

```sh
semaprax json-codec . --plan codec-plan.json --output candidate.json
semaprax json-codec . --plan codec-plan.json --output candidate.replay.json
cmp candidate.json candidate.replay.json
python3 - <<'PYTHON'
import json
from pathlib import Path
bundle = json.loads(Path("candidate.json").read_text())
# Review the complete bundle before writing into this separate installation copy.
for source in bundle["candidate_sources"]:
    Path(source["path"]).write_text(source["source"])
PYTHON
cp src/app.command.spx src/app.spx
semaprax test .
semaprax build . --output catalog-native
./catalog-native < fixtures/request.json
```

The shared [typed application setup operator](../../benchmarks/typed_application_setup_support/README.md)
retains the authored schemas before generation, binds the actual compiler-bundled
library closure, replays the complete paired bundle against the unchanged bootstrap, and returns
a regular complete source tree with explicit build/test/run scripts. It does not
qualify the 23 cases itself or seed paid trial candidates.
Its read-only `--verify-installation` action rechecks a completed tree against the
same compiler/source/build bindings and retained source inventories before the
independent qualification runner; edited or incomplete trees are refused.

The owning Project test derives/replays both candidates against the unchanged
bootstrap, checks canonical source, runs the two source tests, and compares the
native build plus C11 O0/O2 against all 23 exact retained rows (including 256
items, eight tags, escaped keys/IDs, over-65K whitespace, invalid excluded rows
and malformed trailing input). The selector is
`standard_library::application_json::owned::catalog_scoped_records::` (one Rust
test). That functional gate does not authorize paid trials or compiler/profile
relabeling. Fresh v31 application qualification requires exact current compiler
source/binary/profile and the existing independent Catalog acceptance runner.
