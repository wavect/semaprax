# RI-13 combined application measurement gate

This gate runs the three saved RI-13 applications and one linked Project route
as one reproducible receipt:

1. M1 prepares a held Regex/Url Project and executes its two generated Rust/C
   owner carriers.
2. M2 prepares one source revision, deserializes and serializes real generated
   Serde records, then executes generated `Fn` and `FnMut` callbacks through
   `std::Iterator`.
3. M3 prepares and runs the generated local HTTP Future consumer, then records
   direct, handwritten-check, and generated route samples.
4. The linked Project prepares all selected M1/M2/M3 generated packages and
   executes its one-process consumer after the three profile-specific routes.

The profiles deliberately remain separate. The current Project Future profile
does not admit the M1 owner imports or M2 record/callback projection in the
same linked Project. The receipt is a combined executable gate for the three
real application paths, not a claim that those source profiles are one ABI.

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

## Receipt categories

The JSON receipt intentionally separates three quantities:

| Field | What it measures | What it does not mean |
| --- | --- | --- |
| `full_build_and_consumer_stages` | Wall time for each locked Cargo prepare/consumer command, including compilation and process execution | Per-call route latency or compiler-only time |
| `route_timing_and_allocator_requests.routes.*.{mean_ns,p50_ns,p90_ns,p99_ns}` | M3 scalar route samples around direct Rust, handwritten checks, and generated checked source | A nontrivial batch-work result; every route includes loopback HTTP and numeric parsing |
| `batch_throughput.routes.*` | Fifteen 64-operation local HTTP batches per route, with normalized operations per second and allocator requests per batch | A portability or production throughput claim; generated registration remains in every operation |
| `route_timing_and_allocator_requests.routes.*.allocator_requests` | Current-thread allocator calls and requested bytes while one M3 route or batch runs | Copied bytes, retained heap, peak memory, process-wide allocation, or allocations in the server thread |
| `m3_copy_ledger` | Per-route response wire bytes plus exact zero-byte generated and host-callback scalar payload boundaries | Copies inside reqwest, HTTP decoding, `Response::text`, or host callback captures |
| `linked-receipt.py` `copied_byte_ledger` | Exact scalar M3 boundary/callback payload cells and an explicit unavailable foreign HTTP-body cell for the linked no-HTTP callback | A measurement of foreign HTTP-body copies |

The M3 measurement requires the allocator instrumentation added with the
RI-13 measurement work. The request count deliberately records `realloc` as
one reallocation plus its old-size deallocation and new-size allocation. It
does not infer byte copies from allocator activity. The separate copy ledger
uses the CSV's exact payload count and the selected generated signature: this
M3 route crosses only `i64` values, so its generated and host-callback payload
boundaries copy zero bytes. It records unavailable foreign response-body and
host-capture cells explicitly; the linked receipt marks its no-HTTP callback's
foreign-body cell `not_exercised` rather than inferring a zero copy count.

The receipt also runs fifteen 64-operation batches for each route. It reports
normalized operations per second separately from scalar samples and includes
generated registration in every operation, so it is a reproducible local batch
path without claiming a universal threshold result.

## Acceptance still open

This gate does not close #371. It still needs copied-byte measurement for the
foreign body and host callback paths, a nontrivial batch comparison that meets
its acceptance target, and fresh Linux x86_64 and macOS arm64 evidence. The
receipt makes those gaps explicit rather than converting one local run into a
portability or performance claim.

## Linked Project fixture

`project/` is one held `source-local-future.v1` Project that carries all three
selected identities: the two M1 indexed native-Rust exports, the M2 record and
callback declarations, and the single M3 Rust Future export. Its
`linked/` package has a `prepare` binary that derives all four generated
packages/modules from that Project source, then a `consumer` binary links the
M1 owner crates, M2 C/Rust callback module, and M3 generated Future module in
one process. The consumer emits `ri13-linked-project-ok` only after all three
routes return their checked values.

The prepare result is intentionally untracked. Reproduce the linked route with
its own private target directory:

```sh
CLANG=/usr/bin/clang CARGO_TARGET_DIR="$PWD/target/ri13-linked" \
  cargo run --locked --offline --manifest-path examples/ri13-combined-app/linked/prepare/Cargo.toml --bin prepare
CLANG=/usr/bin/clang CARGO_TARGET_DIR="$PWD/target/ri13-linked" \
  cargo run --locked --offline --manifest-path examples/ri13-combined-app/linked/Cargo.toml --bin consumer
```

`linked-receipt.py` performs the no-Cargo structural assertion for the route:
it checks the canonical manifest selections, every required source identity,
the linked prepare/consumer command order, and emits a deterministic receipt
whose inputs are SHA-256 hashes of the tracked fixture files.

## Linux x86_64 evidence runner

`scripts/ri13-linux-x86_64-evidence.sh` plans a reproducible Apple Container
run without starting a container by default. It requires an already-pulled
Linux x86_64 toolchain image named by immutable digest and an existing Linux
Cargo cache that contains the locked dependencies; it never pulls an image or
permits guest network access. For example, this only prints the exact plan:

```sh
scripts/ri13-linux-x86_64-evidence.sh --plan \
  --image 'registry.example/ri13-toolchain@sha256:IMAGE_DIGEST' \
  --cargo-home /absolute/linux-x86_64-cargo-home \
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
not turn a Mac-local result into Linux evidence.
