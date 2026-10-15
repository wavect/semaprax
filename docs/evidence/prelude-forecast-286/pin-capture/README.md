# Source-only pin capture plan

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

No producer or compiled gate ran in this lane. No pin literal is changed here.
The exact existing owners and separate ignored producer are registered in
`selectors.json`. Their original assertions remain active. There was no existing
environment regeneration support; the two owning integration tests now accept
an optional `SEMAPRAX_PRELUDE_PIN_CAPTURE_DIR` evidence sink. It creates fresh
files from actual API outputs before existing literal assertions and never
replaces prior evidence. The existing ignored library producer still prints its
full actual projection wire with its original marker.

## Root execution after the focused build

Use already-built **test executables**, from the same root source build. Do not
pass the CLI executable. No additional Cargo target is needed for the initial
capture. Run from the integrated root checkout:

```sh
scripts/capture-prelude-forecast-pins.sh \
  /absolute/private-target/debug/deps/semaprax-TEST_HASH \
  /absolute/private-target/debug/deps/workspace-TEST_HASH \
  /absolute/fresh-evidence-directory
```

The script runs precisely the two existing public graph/analysis test owners
plus the existing ignored full-wire library producer, serially. It writes raw
logs, each exact selector's exit status, source head/local patch, binary hashes,
full actual wire bytes, and `candidates.json`. Ordinary failing assertions remain
failures in `status.tsv`; capture success is not gate success. Missing actual
artifacts produce script exit 3. The script never edits source pins, guesses a
value, builds, selects another test binary, or runs a full gate.

## Review and update from actual bytes only

1. Preserve the original test literals and complete historical receipt/source
   files here. The two pre-hook test sources have exact inverse records; the
   browser JS owner is also archived byte-for-byte. Retain this evidence when
   updating current pins; do not normalize historical values.
2. Check source/binary provenance and every owner log. Read actual full graph
   JSON and budget before taking its numeric debit and document SHA. Read all
   eight full analysis wires before taking their document SHAs; inspect their
   embedded graph digest bindings. The projection producer emits its complete
   JSON, whose document SHA is computed directly from those captured bytes.
3. Compare full historical/current wires where available. For this correction,
   audit the forecast-derived builder debit and authenticated digest changes;
   reject unrelated field drift. The script does not assert exclusive causality
   or manufacture a historical wire from a new forecast.
4. Update only reviewed current literals in the owning tests from
   `candidates.json` and actual JSON bytes. Preserve old literals/history in the
   archives. Rerun those same exact owners to qualify the new pins. A failed or
   absent producer output is not a candidate.

The Wasm browser owner is deferred for the first capture and introduces no new
required build target. Its baseline/renamed graph-digest pins are affected by a
changed builder budget. When root elects to run that existing owner, append its
already-built Wasm test executable as the optional fourth script argument. Its
original baseline assertion can stop before the renamed producer executes;
review/update only captured baseline literals, then rerun into another fresh
evidence directory to obtain the actual renamed graph. The original assertions
are not bypassed to accelerate capture. The browser fixture JS pins must come
from those actual baseline/renamed producer outputs, followed by its existing
qualification selector. Project/workspace revision pins must not move without
actual source-evidence justification.

Alternative existing real browser artifact producers, when independently
needed, are the CI commands in `.github/workflows/ci.yml`:

```sh
cargo run --locked -p semaprax -- build examples/calculator-project/semaprax.toml \
  --target web -o /absolute/isolated-baseline/generated
cargo run --locked -p semaprax -- build /absolute/isolated-renamed-project/semaprax.toml \
  --target web -o /absolute/isolated-renamed/generated
```

The renamed input is a separate copied calculator project with exactly the
existing `fn add` → `fn sum` display rename. These commands are documented only;
this packet does not run them or expand the current focused qualification.

## Exact candidate-to-pin updater (prepared; not executed)

`scripts/update-prelude-forecast-pins.py` defaults to a read-only proposed update.
Only after root supplies its actual frozen-source captures and reviews the full
wires:

```sh
python3 scripts/update-prelude-forecast-pins.py /absolute/root-capture \
  --repo /absolute/private-pin-update-worktree \
  --expected-capture-head ROOT_APPROVED_HEAD
```

It requires every initial lib/workspace wire, exact owning selectors, one-test
logs, successful existing ignored producer, actual capture manifest/hash/length
matches, frozen capture source, graph payload digest binding, unchanged graph
schema/cap, and the same public graph binding in all eight analysis artifacts.
It fails closed if any expected old literal is missing or occurs more than once.
It uses no new numeric or hash value except those parsed/hashed from actual
captured wire bytes. The original capture exit statuses remain evidence; a pin
assertion failure does not become a successful gate.

The exact change set is eleven literals:

- `tests/workspace/semantic_graph.rs`: public `used_builder_bytes` and whole
  document SHA from `public-graph.json`.
- The same file's eight-element SHA array, in order: `context`, `forward`,
  `capability-context`, `type-context`, `impact`, `custom-impact`, `review`,
  `capability-review`, each hashed from its complete captured JSON bytes.
- `src/workspace_graph/type_proof_tests.rs`: `document_sha` literal from the
  existing ignored producer's complete `projection-wire.json` bytes.

After reviewing that proposal, append `--apply --evidence-dir /absolute/fresh-pin-evidence`
to preserve the complete actual capture, full pre-update source files, old pin
values and exact before/after substitutions before updating those eleven source
literals in the explicitly selected private worktree. This does not update root,
browser pins, assertions, quotas, source schemas or other files. It does not
rewrite historical receipts to resemble the new model. Root should refresh the
nearby historical comments only with the actual validated accounting delta;
the updater leaves existing history prose intact. Run the same three normal
pin owners and existing qualification controls after cherry-picking the reviewed
source change. No full gate, compiler invocation or producer execution is added
by this updater.

The graph-binding validator uses the owning v1 formula exactly:
`SHA256(b"semaprax.workspace-semantic-graph.artifact-digest.v1\0" ||
u64_le(payload byte length) || payload)`, with only the one graph-digest field
removed. Whole-document pin SHAs independently hash the complete captured wire
bytes. An independent source review identified and corrected an earlier private
updater draft that incorrectly used raw SHA256(payload) for graph binding; that
draft never ran against actual captures and changed no pin literal.

Independent review additionally tightened the private updater before use. Its
required trust arguments now include the existing ROOT round directory, exact
40-character approved capture head, and externally ROOT-approved raw SHA-256
values of `owning-build-result.json` and `prelude-pin-producers-result.json`:

```sh
python3 scripts/update-prelude-forecast-pins.py /absolute/root-capture \
  --repo /absolute/private-pin-update-worktree \
  --round /absolute/root-round \
  --expected-capture-head FULL_40_HEX_ROOT_HEAD \
  --expected-build-receipt-sha256 ROOT_APPROVED_BUILD_RECEIPT_SHA256 \
  --expected-producer-receipt-sha256 ROOT_APPROVED_PRODUCER_RECEIPT_SHA256
```

It consumes that wrapper's successful frozen build/capture binding and exact
lib/workspace executable roles/hashes, and checks ROOT's current full head and
clean status including untracked files. The wrapper already verifies this
binding before, during and after its actual execution; no capture protocol or
frozen ROOT source is changed. Target pin-owner files must equal the trusted
frozen commit byte-for-byte. An owner exit 101 now requires exactly one panic
at the actual expected stale-pin assertion line, with the exact old right value
and captured actual left value (budget, document SHA or complete eight-SHA
array). Non-pin getter, CLI, locking, wire or other assertion failure blocks an
update even when all wire files were captured.

Source reads, hashes, archives and writes use raw bytes; unexpected CRLF/CR is
rejected rather than normalized. Every source file is preflighted again before
any source write. Apply evidence must be fresh and outside both capture and the
target repository, preventing recursive evidence copying. Later import of that
external reviewed evidence into the private pin commit is a separate explicit
source/evidence step. These checks change no pin values or test assertions.
