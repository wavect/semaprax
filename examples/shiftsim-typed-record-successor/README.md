# ShiftSim typed-record successor

This source draft uses named `Patient`, `Worker`, `Assignment`, and `Metrics`
records. Actual worker rows hold both `id` and `free_at`; consuming
`vec_update_field<Worker>(workers,index,finish,"free_at")` updates availability
without rebuilding the row or cloning its ID. The scheduler no longer maintains
a parallel worker-state vector. The prior `WorkerState` declarations and helper
bodies remain independent controls. Patient state and sorted arrival indices
remain separate for their distinct scheduling roles. Patient uniqueness uses
`vec_unique_by`; borrowed IDs are copied only into completed output assignments.
The original tasks, acceptance/oracles, 8/256 bounds and one-million-step fuel
limit remain unchanged. Current-head executable qualification is pending; this
source change establishes no allocation or agent-efficiency result.
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
The schedule uses `break;` to stop a borrowed scan at its count bound or
first match. Indexed helpers return the first eligible index directly with
`return index;`. The dispatch loop uses `break;` when no worker or patient
is ready.
The exact Server-wrapper program remains a differential control for all 15
cases and original invalid-input checks. Qualification is pending. Conversion
vector removal changes allocation-failure ordinals; no allocation gain is claimed.

The request and response schemas are authored in `src/request.spx` and
`src/response.spx`. From this example directory, use the full toolchain to
generate both codecs and select the real command in a fresh directory.

```sh
(
  set -e
  example="$(pwd -P)"
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/shiftsim.XXXXXXXX")"
  output="$scratch/shiftsim-app"
  semaprax-full new "$output" --from "$example" --codec-plan codec-plan.json \
    --command-profile language-command-io.collection-record.v1 \
    --command-path src/app.spx --command-source src/app.command.spx
  cd "$output"
  semaprax-full check .
  semaprax-full test . --target native-collection-record-pure.v1
  semaprax-full build . --target native -o shiftsim
  ./shiftsim < "$example/fixtures/request.json"
)
```

The generated README records the codec plan, command activation, source
inventory and graph digest. A fresh destination leaves the example sources
available for review. You can also inspect the paired codec bundle independently.

```sh
semaprax json-codec . --plan codec-plan.json --output codecs.candidate.json
```

`codec-plan.json` selects the streaming request and bounded collection
response profiles. The review bundle retains the authored schema bytes,
complete candidate sources and exact per-entry receipts. It grants no
source mutation authority.

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
