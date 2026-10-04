# RI-13 combined application measurement gate

This gate runs the three saved RI-13 applications as one reproducible receipt:

1. M1 prepares a held Regex/Url Project and executes its two generated Rust/C
   owner carriers.
2. M2 prepares one source revision, deserializes and serializes real generated
   Serde records, then executes generated `Fn` and `FnMut` callbacks through
   `std::Iterator`.
3. M3 prepares and runs the generated local HTTP Future consumer, then records
   direct, handwritten-check, and generated route samples.

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

`--fresh-target` refuses a preexisting target directory. This makes the six
named prepare/consumer stages a clean-target build receipt. Omit it only when
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
| `route_timing_and_allocator_requests.routes.*.{mean_ns,p50_ns,p90_ns,p99_ns}` | M3 route samples around direct Rust, handwritten checks, and generated checked source | A nontrivial batch-work result; every route includes loopback HTTP and numeric parsing |
| `route_timing_and_allocator_requests.routes.*.allocator_requests` | Current-thread allocator calls and requested bytes while one M3 route runs | Copied bytes, retained heap, peak memory, process-wide allocation, or allocations in the server thread |
| `m3_copy_ledger` | Per-route response wire bytes and the exact zero-byte generated `i64 -> i64` boundary | Copies inside reqwest, HTTP decoding, `Response::text`, or the caller-owned callback |

The M3 measurement requires the allocator instrumentation added with the
RI-13 measurement work. The request count deliberately records `realloc` as
one reallocation plus its old-size deallocation and new-size allocation. It
does not infer byte copies from allocator activity. The separate copy ledger
uses the CSV's exact payload count and the selected generated signature: this
M3 route crosses only `i64` values, so its generated boundary copies zero byte
payload. It records the uninstrumented foreign copy domains explicitly.

## Acceptance still open

This gate does not close #371. It still needs a source profile that actually
links all three categories, copy measurement for the foreign body and host
callback paths, a nontrivial batch comparison that meets its acceptance target,
and fresh Linux x86_64 evidence. The receipt makes those gaps explicit rather
than converting one local run into a portability or performance claim.

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
