# ShiftSim typed-record successor

This source draft replaces the retained scheduler's six-slot scalar packing
with named `Patient`, `Server`, `Assignment`, and `Metrics` records. The
declared `Patient` field order is the dispatch key `(priority, arrival, id)`;
the stable owned-record sort preserves those ties. Mutable scheduling state
uses separate Copy records indexed alongside the sorted owners, so waiting and
completion scans do not deep-clone patient IDs. IDs are cloned only when a
completed assignment becomes part of the owned report.

The request and response schemas are authored in `src/request.spx` and
`src/response.spx`. Bootstrap the ordinary compiler-generated helpers in a
copy of this directory. Derive **both** complete replacements against the same
unchanged bootstrap before installing either one:

```sh
semaprax json-codec . --source src/request.spx --type shiftsim.request \
  --output src/request.generated.spx --profile stream-owned-request.v1
semaprax json-codec . --source src/response.spx --type shiftsim.report \
  --output src/response.generated.spx --profile bounded-collection-response.v1 \
  --max-string-bytes 16
cp src/request.generated.spx src/request.spx
cp src/response.generated.spx src/response.spx
cp src/app.command.spx src/app.spx
semaprax check .
semaprax test .
semaprax build . --target native -o shiftsim
./shiftsim < fixtures/request.json
```

For retained schema snapshots, exact compiler/build-log bindings, replay of
both derivations and a regular installation tree, use the shared
[typed application setup operator](../../benchmarks/typed_application_setup_support/README.md).
It also installs explicit build/test/run scripts for the unpaid v31 qualification
route. It never copies this application into a paid trial or changes trial prompts.
Before fresh qualification, the same operator supports `--verify-installation`
to recheck a completed source tree without regenerating it or claiming acceptance.

The request generator supplies the existing stream normalizer and owned
decoder for `{servers:Vec<string>, patients:Vec<Patient>}`. The response
generator emits the bounded encoder for `Report`, whose assignments contain
both patient and server IDs. The command rejects malformed JSON, duplicate or
non-ASCII identifiers, oversized arrays, and out-of-range numeric fields with
status 2, one diagnostic line on stderr, and empty stdout. Successful output is
one canonical JSON line. `tests.spx` covers a tied two-patient/eight-server case
and the full 256-patient serial boundary.

The manifest selects the explicit v31 collection-record command profile.
This is an application-source draft, not a qualified candidate: its
compiler/backend execution and all 15 retained acceptance cases remain
pending. The historical SEM03 candidate, SPEC, corpus, oracle, output, and
qualification records are unchanged. This example makes no performance or
token-savings claim.
