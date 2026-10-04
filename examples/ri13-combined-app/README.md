# RI-13 combined application measurement gate

This gate runs the three saved RI-13 applications and one linked generated route
as one reproducible receipt:

1. M1 prepares a held Regex/Url Project and executes its two generated Rust/C
   owner carriers.
2. M2 prepares one source revision, deserializes and serializes real generated
   Serde records, then executes generated `Fn` and `FnMut` callbacks through
   `std::Iterator`.
3. M3 prepares and runs the generated local HTTP Future consumer, then records
   direct, handwritten-check, and generated route samples.
4. The linked route prepares all selected M1/M2/M3 generated packages and
   executes its one-process consumer after the three profile-specific routes.

The standalone M1, M2, and M3 profiles remain as independent measurement
routes. The linked preparation below uses one additional, closed Project
profile rather than treating those standalone profiles as one ABI.

`unified-project/` is the concrete authored one-Project route. It combines
the M1 imports, M2 declarations, and M3 yielding export under
`source-local-future-indexed-rust.v1`. That profile admits only the exact
Regex/Url dependency pair and two M1 exports; putting those dependencies under
the ordinary `source-local-future.v1` profile still rejects with `SPX-J100`.
The successful `linked/prepare` path writes
one generated subject binding for the authenticated M1, M2, and M3 inputs, and
the consumer build refuses to link without it.

M1's committed Regex and Url API indexes are `aarch64-apple-darwin` inputs.
Their package generator requires the current native target, so a Linux x86_64
attempt correctly stops at `SPX-B112` before it can produce the linked M1
packages. This is an acceptance limit of the pinned M1 input, not a fallback
or a reason to retarget its authenticated index.

## Reproduce

From the repository root, use a private target directory in this checkout and
an explicit C compiler for M1's generated C objects:

```sh
CLANG=/usr/bin/clang python3 examples/ri13-combined-app/measure.py \
  --fresh-target \
  --target-dir "$PWD/target/ri13-combined-app" \
  --output examples/ri13-combined-app/measurements/local-receipt.json
```

`--fresh-target` refuses a preexisting target directory. This makes the eight
named prepare/consumer stages a clean-target build receipt. The final
`linked_prepare` and `linked_consumer` stages are the physical M1/M2/M3
linkage check; `linked_consumer` emits its marker only after the generated M1
owner carriers, M2 record/callback module, and M3 Future module return their
checked values in one process. Omit `--fresh-target` only when
intentionally measuring a warmed target, which the receipt identifies.

The harness is offline and locked. It executes M1 and M2 before M3, requires
each consumer's success marker, and refuses an M3 CSV without all five
allocator-request columns. The built M2 consumer is the record/Serde and
callback proof in this sequence; a manifest or source-text check cannot stand
in for it.

Run its parser-only control without Cargo:

```sh
python3 examples/ri13-combined-app/measure.py --self-test --output /tmp/unused.json
```

The authored-code and escape-hatch disclosure is also reproducible without Cargo:

```sh
python3 examples/ri13-combined-app/friction-ledger.py \
  > /tmp/ri13-developer-friction.json
python3 -m unittest examples/ri13-combined-app/test_friction_ledger.py
```

It hashes and counts each authored Rust file by M1, M2, M3, and linked route;
refuses handwritten ABI or `unsafe` escape-hatch tokens in those application
sources; excludes prepare-derived artifacts from the authored count; and names
the small caller-owned M3 HTTP/runtime configuration separately. It is a
reproducible disclosure rather than a developer-effort score.

## Receipt categories

The JSON receipt intentionally separates three quantities:

| Field | What it measures | What it does not mean |
| --- | --- | --- |
| `full_build_and_consumer_stages` | Wall time for each locked Cargo prepare/consumer command, including compilation and process execution | Per-call route latency or compiler-only time |
| `route_timing_and_allocator_requests.routes.*.{mean_ns,p50_ns,p90_ns,p99_ns}` | M3 route samples around direct Rust, handwritten checks, and generated checked source | A nontrivial batch-work result; every route includes loopback HTTP and numeric parsing |
| `batch_throughput.routes.*` | Fifteen 64-operation local HTTP batches per route, with normalized operations per second and allocator requests per batch | A portability or production throughput claim; generated registration remains in every operation |
| `route_timing_and_allocator_requests.routes.*.allocator_requests` | Current-thread allocator calls and requested bytes while one M3 route or batch runs | Copied bytes, retained heap, peak memory, process-wide allocation, or allocations in the server thread |
| `m3_copy_ledger` | Response wire bytes, the fixture's exact `Bytes` to `Vec<u8>` response copy, its generated host callback subset, and zero-byte scalar boundaries | Copies inside reqwest or HTTP decoding before the observed copy, UTF-8 validation, or a complete application copy total |
| `linked-receipt.py` `copied_byte_ledger` | Exact scalar M3 boundary/callback payload cells and an explicit unavailable foreign HTTP-body cell for the linked no-HTTP callback | A measurement of foreign HTTP-body copies |
| `linked_copy_ledger` | M1 Regex buffer-scan input bytes and Regex/Url generated-adapter copy events and bytes, M2 generated mirror clone bytes, and scalar callback boundaries | `regex::Regex::is_match`, `url::Url::parse`, and Serde deserialization copies inside foreign implementations, which remain unavailable |

The M3 measurement requires the allocator instrumentation added with the
RI-13 measurement work. The request count deliberately records `realloc` as
one reallocation plus its old-size deallocation and new-size allocation. It
does not infer byte copies from allocator activity. The separate copy ledger
emits the fixture's explicit `Response::bytes()` to callback-owned `Vec<u8>`
copy on a new run. `host_callback_captured_bytes` is the same operation when
the generated route performs it, so it is a scoped subset of foreign-body
copied bytes and must not be added to it. The generated boundary still crosses
only `i64` values; reqwest and HTTP copies before this explicit capture remain
unknown. Existing committed samples predate this instrumentation. The linked
no-HTTP callback marks its foreign-body cell `not_exercised`.

The receipt also runs fifteen 64-operation batches for each route. It reports
normalized operations per second separately from scalar samples and includes
generated registration in every operation, so it is a reproducible local batch
path without claiming a universal threshold result.

The linked consumer emits its own one-line canonical ledger after it executes
the M1 owners and M2 record/callback routes. The measurement parser requires
the Regex carrier to report its exact 28-byte borrowed matcher input, zero
adapter copy events, and zero adapter bytes; it binds the M2 record fixture's
25-byte JSON plus the generated mirror's three-byte label clone. It also
exercises one `Fn` and one `FnMut` invocation; their `i64` argument/result
boundary carries zero buffer bytes. This does not turn `Regex::is_match`, the
Url parser, or `serde_json` deserialization into a zero-copy claim: their
foreign internal byte observations remain unavailable.

## Acceptance still open

This gate does not close #371. It still needs exact copied-byte evidence for
the issue’s buffer scan and the uninstrumented foreign portions of ownership
transfer and deserialization, a nontrivial batch regression investigation, and
fresh Linux x86_64 evidence. The receipt makes those gaps explicit rather than
converting one local run into a portability or
performance claim.

## Linked generated fixture

`linked/prepare` authenticates `unified-project/` for all M1/M2/M3 generated
outputs. The closed profile carries the exact M1 dependencies and exports,
while retaining the M3 Future selection. The `consumer` binary links the M1
owner crates, M2 C/Rust callback module, and M3 generated Future module in one
process. It emits `ri13-linked-project-ok` only after all three routes return
their checked values.

The prepare result is intentionally untracked. Reproduce the linked route with
its own private target directory:

```sh
CLANG=/usr/bin/clang CARGO_TARGET_DIR="$PWD/target/ri13-linked" \
  cargo run --locked --offline --manifest-path examples/ri13-combined-app/linked/prepare/Cargo.toml --bin prepare
CLANG=/usr/bin/clang CARGO_TARGET_DIR="$PWD/target/ri13-linked" \
  cargo run --locked --offline --manifest-path examples/ri13-combined-app/linked/Cargo.toml --bin consumer
```

`linked-receipt.py` performs the no-Cargo structural assertion for the route.
It binds one authenticated Project selection to its M1/M2/M3 preparation
paths, generated C inputs, consumer result checks, and existing copied-byte
ledger contract. It preserves unavailable foreign copy cells and does not
replace the compiled linked consumer gate.

Run its static control without Cargo:

```sh
python3 examples/ri13-combined-app/linked-receipt.py --self-test
```

## Linux x86_64 evidence runner

`scripts/ri13-linux-x86_64-evidence.sh` plans a reproducible Apple Container
run without starting a container by default. It requires an already-pulled
Linux x86_64 toolchain image named by immutable digest and an existing Linux
Cargo cache that contains the locked dependencies; it never pulls an image or
permits guest network access. It also requires a directory containing separate
Linux x86_64 Rustdoc extractor envelopes named
`regex-1.13.1-index-envelope.json` and `url-2.5.8-index-envelope.json`.
Capture those from the exact locked package sources with the pinned extractor
and `--target x86_64-unknown-linux-gnu`, then convert them with
`crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py` as specified in
[Rust API Index v2](../../docs/RUST-API-INDEX-V2.md). Editing the committed
Darwin envelopes is not an admissible substitute. The runner validates their
schema, package identity, and Linux target before it starts a container; the
M1 and linked preparation routes replay them and retain the existing exact
target and package checks.

The pinned evidence image supplies stable Rust 1.98 but does not contain the
pinned nightly extractor. Prepare the two envelopes first with a separately
installed Linux x86_64 `nightly-2026-10-02` toolchain and an already populated
Linux Cargo home. The preparation runner mounts both inputs read-only, verifies
the exact `regex` and `url` crate archive checksums from the committed locks,
uses locked offline `cargo rustdoc`, and retains the raw JSON plus canonical
envelopes and digest receipt. It does not run the M1/M2/M3 application gate:

```sh
scripts/ri13-linux-x86_64-index-prepare.sh --run \
  --image 'ri13-linux-evidence:rust-1.98.0@sha256:c9024b5897124ae3a7f124a41dbe4d301c7319daf4e9aed82b23424648eb311e' \
  --cargo-home /absolute/linux-x86_64-cargo-home \
  --nightly-toolchain /absolute/nightly-2026-10-02-x86_64-linux \
  --output /absolute/ri13-linux-rust-api-index-preparation
```

Pass `/absolute/ri13-linux-rust-api-index-preparation/envelopes` to the evidence
runner as `--rust-api-index-dir`. A missing exact nightly toolchain is an
unavailable prerequisite, not permission to alter target facts in an envelope.

For example, this only prints the exact plan:

```sh
scripts/ri13-linux-x86_64-evidence.sh --plan \
  --image 'registry.example/ri13-toolchain@sha256:IMAGE_DIGEST' \
  --cargo-home /absolute/linux-x86_64-cargo-home \
  --rust-api-index-dir /absolute/ri13-linux-rust-api-indexes \
  --evidence /absolute/ri13-linux-x86_64-evidence
```

After provisioning those inputs, replace `--plan` with `--run`. The runner
refuses a preexisting evidence directory, clones the checked revision into its
own writable evidence worktree, mounts the original checkout nowhere in the
guest, and uses `container run --rm --network none`. The guest rejects any
platform other than Linux x86_64 and writes `environment.json`, M1/M2/M3
combined receipt and batch measurement, linked Project receipt, and command
logs to the named evidence directory. `output-digests.json` records SHA-256
digests for those outputs. It therefore leaves no running container and does
not turn a Mac-local result into Linux evidence. No Linux evidence has been
recorded by this route yet.

## Explicit Wasm refusal evidence

RI-13's generated Rust routes are not Wasm or cross-target support. The
following command uses one supplied current-head compiler binary to request the
Wasm target for each independent M1, M2, and M3 Project manifest. It records
the exact manifest profile (`implicit scalar.v1` for M1/M2 and
`source-local-future.v1` for M3), compiler and checkout identity, command,
diagnostic codes, and raw stdout/stderr artifacts. Every row must exit nonzero
and leave its requested `.wasm` output absent.

```sh
python3 examples/ri13-combined-app/wasm-refusal.py \
  --semaprax /absolute/path/to/current/semaprax \
  --checkout "$PWD" \
  --commit "$(git rev-parse HEAD)" \
  --artifact-dir /secure/local/ri13-wasm-refusal-artifacts \
  --output /secure/local/ri13-wasm-refusal.json
```

The receipt proves only an explicit refusal for the named source profiles and
target. It does not assert a fallback, Wasm execution, a native result, or
cross-target portability. A missing compiler or changed checkout is
`unavailable`; a zero exit or materialized output is a failed refusal.
