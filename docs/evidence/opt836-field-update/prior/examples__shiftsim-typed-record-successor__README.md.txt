# ShiftSim typed-record successor

This source draft replaces the retained scheduler's six-slot scalar packing
with named `Patient`, `Assignment`, and `Metrics` records. Server IDs stay
in a sorted `Vec<string>` and use `vec_str_at` for validation and selection. The
declared `Patient` field order is the dispatch key `(priority, arrival, id)`;
the stable owned-record sort preserves those ties. Mutable scheduling state
uses separate Copy records indexed alongside the sorted owners, so waiting and
completion scans do not deep-clone patient IDs. Patient uniqueness now copies
each ID once into a separate owned String vector,
sorts it, and compares adjacent borrowed slots. Dispatch order stays in the
original patient vector. The frozen pairwise helper remains an independent
control, including 256 IDs; the application still uses its original one-million-step
limit. These validation copies and the vector change allocation-failure ordinals.
Output IDs are copied when a completed assignment enters the owned report.
Runtime/fuel improvement is unqualified until the focused application gates run.
A separate Copy `Vec<i64>` sorts arrival times once. Its monotonic cursor counts
all arrivals at or before the current event; subtracting the dispatched count
gives the exact waiting count. The next event is the first later arrival or the
first later worker completion. Patient priority order and server ties remain in
their original vectors. Frozen full-scan waiting/next-time helpers and the old
scheduler remain independent controls. This adds a Copy vector allocation and
changes physical failure ordinals; the original 256/8 bounds, corpus, and fuel
limit stay unchanged.
Dispatch checks for a free worker before scanning waiting patients. After the
existing validation branch, both Copy-state lookups use their exact retained
lengths (at most 8 workers and 256 patients), have no effects or contracts,
and perform bounded arithmetic. Skipping the patient scan when no worker is
free skips no language failure on these valid states. General indexed reads
keep their checked bounds and failure rules.
The exact Server-wrapper program remains a differential control for all 15
cases and original invalid-input checks. Qualification is pending. Conversion
vector removal changes allocation-failure ordinals; no allocation gain is claimed.

The request and response schemas are authored in `src/request.spx` and
`src/response.spx`. Bootstrap the ordinary compiler-generated helpers in a
copy of this directory. Derive the paired codecs as one complete checked,
reviewable bundle against the unchanged authored revision:

```sh
semaprax json-codec . --plan codec-plan.json --output codecs.candidate.json
```

`codec-plan.json` selects the original streaming request and bounded collection
response profiles. The bundle retains original authored schemas separately from
all candidate sources and exact per-entry receipts. Review the complete candidate
before installation through ordinary source or managed transaction authority;
there is no user-managed derive/replay order between the two codecs. The command
never overwrites source and the bundle grants no mutation authority. After
installing the reviewed candidate in the copy, activate the real command:

```sh
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

The scheduler keeps Copy-only state scans and metric decisions in ordinary
checked helper functions. Each scan preserves ascending order, first-match
sentinels and lazy arithmetic; arrival candidates are scanned before completion
candidates. This prevents independent internal branch combinations from being
multiplied inside the owning dispatcher. The 65,536 cleanup path limit remains
unchanged, and the exact earlier schedule is retained as a refusal control.
The existing 15-case wire corpus owns qualification of this source factoring;
no runtime performance improvement is claimed without execution.
