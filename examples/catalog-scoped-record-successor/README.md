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
an extra String inventory. `Tag` wraps each owned decoded String once. Scoped
`vec_field` reads inspect numeric fields, adjacent IDs and tag prefixes without
cloning an unrelated row or String; adjacent views reuse `std.text.equals`.
Reads retain each vector's live generation
through their last use. `SortItem` has department/priority/ID declaration order
for business sorting; consuming it into response `Item` preserves the required
ID/department/priority/stock/mark JSON member order. The real response encoder
preserves vector order and metrics order. No pointer, input position or token
spelling becomes sorting authority.

The report is below 65536 bytes for every admitted 256-row/16-byte-ID value
(even a conservative 128 bytes per row plus 128 metrics bytes is smaller).
Its encoder preflight uses a 65535-byte limit, leaving room for one newline.
Ordered publication fragments own a String label and Bytes payload. The two
fragment allocations, stable sort, clone and replacement occur outside loops;
the existing Bytes/fuel/provider/ownership limits remain unchanged. Output
refusal or short provider writes are status 1, distinct from invalid input's
status 2 and exact `invalid catalog request\n` stderr line.

Install into a separate copy with a verified compiler. Keep `src/app.spx` as the
bootstrap until **both** complete-source derivations have been produced:

```sh
semaprax json-codec . --source src/request.spx --type catalog.request \
  --output src/request.generated.spx --profile stream-utf8-owned-request.v1 \
  --max-string-bytes 16
semaprax json-codec . --source src/response.spx --type catalog.report \
  --output src/response.generated.spx --profile bounded-collection-response.v1 \
  --max-string-bytes 16
cp src/request.generated.spx src/request.spx
cp src/response.generated.spx src/response.spx
cp src/app.command.spx src/app.spx
semaprax test .
semaprax build . --output catalog-native
./catalog-native < fixtures/request.json
```

The shared [typed application setup operator](../../benchmarks/typed_application_setup_support/README.md)
retains the authored schemas before generation, binds the actual compiler-bundled
library closure, replays both outputs against the unchanged bootstrap, and returns
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
