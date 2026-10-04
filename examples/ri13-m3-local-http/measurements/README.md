# RI-13 M3 local measurement protocol and coverage ledger

This is exploratory local evidence for the saved **scalar M3** HTTP route.
The three routes use the same pinned `reqwest` client, one explicit Tokio
current-thread runtime, one local TCP server, the same two-byte body `43`,
and seed `41`. Direct Rust fetches and adds the result. The handwritten
adapter performs the selected source's input check, request `seed + 1`, and
result postcondition in Rust. The generated route registers and awaits the
held Project's checked source `yield` through generated module bytes. Every
route must return `84` on every sample. Route order rotates each iteration.

`measure.rs` runs nine warmups and 90 timed requests per route, recording each
elapsed nanosecond duration in CSV. The server accepts exactly 297 requests.
New runs also record current-thread allocation/deallocation/reallocation calls
and requested allocation/deallocation bytes around each route invocation. The
counter excludes the local server thread and setup before the timed call. It
measures allocator requests, not copied bytes, retained heap, peak residency,
or allocations made by another thread. Older committed CSVs omit these columns;
the summary accepts both shapes and records allocation means only when present.
Each sample includes loopback TCP, HTTP parsing, a new closed connection and
numeric body parsing. These costs are substantial in the direct and handwritten
routes; the separate generated probe attributes a majority of that route's
time to preparation. There is no isolated scalar-call, buffer-scan,
ownership-transfer, generic-record or stateful-callback timing in this M3
subprofile. These tiny calls do not isolate nontrivial batch work, so this
ratio does **not** establish the issue's normalized nontrivial batch-work
threshold. The pre-recorded investigation trigger is generated throughput
below 90% of the handwritten route; it calls for diagnosis, not an automatic
support or performance verdict. The summary uses paired iteration bootstrap
resampling with a fixed seed to expose uncertainty.

From the saved example directory, after the locked offline `prepare` command
in the parent README:

```sh
export CARGO_TARGET_DIR="$PWD/target/ri13-m3"
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo run --locked --offline --quiet --bin measure > measurements/local-samples.csv
python3 measurements/summarize.py measurements/local-samples.csv > measurements/local-summary.json
```

The CSV and summary should be committed only with exact toolchain, target,
checkout, command and pass/fail receipt. Cold/warm build times, generated
binary size, process-wide allocations, retained heap and total copied bytes
need separate measurements; this run does not infer them from API shape.

The local `prepare` run on Darwin arm64 with Cargo/rustc 1.98.0 rendered a
4,176-byte `src/generated.rs` file. That is the generated module's source
size for this exact selected Project; it excludes linked binary size and
the caller-authored host effect.

## Recorded local result and investigation

The offline `measure` command exited zero on Darwin arm64 with Cargo/rustc
1.98.0. It ran from detached base `2749556f6` plus the pending saved-app and
measurement source patches; it is local pre-integration evidence, not a
fresh-checkout, Linux or hosted receipt. The exact raw samples are in
[`darwin-arm64-2026-10-04.csv`](darwin-arm64-2026-10-04.csv), and the
deterministic summary is in
[`darwin-arm64-2026-10-04-summary.json`](darwin-arm64-2026-10-04-summary.json).

| Route | Mean latency | p50 | p90 | p99 | Serialized calls/s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Direct Rust | 0.303 ms | 0.308 ms | 0.353 ms | 0.446 ms | 3,300.69 |
| Handwritten checked adapter | 0.292 ms | 0.307 ms | 0.331 ms | 0.356 ms | 3,424.99 |
| Generated checked source | 2.351 ms | 2.301 ms | 2.574 ms | 2.855 ms | 425.28 |

Generated/handwritten normalized throughput is **0.1242** with paired
iteration bootstrap 95% interval **[0.1207, 0.1276]**. This is far below the
pre-recorded 0.90 investigation trigger. The result is a substantial local
slowdown in this tiny loopback call and must not be described as passing the
issue's nontrivial batch threshold.

One focused attribution run of `cargo run --locked --offline --quiet --bin
measure -- probe` exited zero. It ran 3 warmups and 30 generated local HTTP
calls and retained each phase in
[`darwin-arm64-preparation-2026-10-04.csv`](darwin-arm64-preparation-2026-10-04.csv).
Mean total latency was 2.528 ms (median 2.450 ms, p90 2.795 ms). Measured
`register` plus `call_typed` preparation averaged 1.321 ms (median 1.296 ms,
p90 1.422 ms), **52.26%** of total by sums; paired bootstrap 95% interval
for this fraction was **[51.71%, 52.79%]**. The subsequent await averaged
1.206 ms (median 1.164 ms, p90 1.366 ms), including local HTTP and checked
source resume. These phase times are measured. Inspection of
`SourceLocalFuture::prepare_revision` shows signature selection, checked
program cloning and a prefix interpreter run before polling; the poll path
resumes the checked source after the host Future. Assigning exact costs to
those individual operations is an inference, because the probe did not time
them separately. The 90-sample comparison and 30-sample attribution ran at
different times and should not be subtracted as if paired.

## Signature coverage for this saved application

| Exact item | Route used | Coverage |
| --- | --- | --- |
| `ri13.m3.score(i64) -> i64` with one `yield i64 -> i64` | Selected `source-local-future.v1` Project, generated Rust registration | Bounded automatic checked source execution after explicit preparation and registration |
| `reqwest::Client::builder`, `.retry(never)`, `.timeout`, `.build` | Caller-owned Rust host | Explicit configuration, no generated capability |
| `reqwest::Url::parse`, `Client::get`, `RequestBuilder::send` | Caller-owned Rust effect handler | Explicit adapter callback; no source-authenticated reqwest import |
| `Response::status`, `Response::bytes`, numeric body parse | Caller-owned Rust effect handler | Explicit status/parse/transport error mapping |
| Tokio current-thread runtime and local cancellation | Caller-owned Rust host | Explicit executor and cancellation control |
| RI-06 owner-tied `Url`/Regex, RI-07 records/Serde, RI-08 stateful callbacks | No signature selected in this Project profile | Unsupported in this saved M3 Project; M1/M2 and combined M3 remain open |

The response body has two wire bytes per request. The route now calls
`Response::bytes()` then copies that foreign body into an application-owned
`Vec<u8>` before UTF-8 parsing. New CSV output records that exact two-byte copy
for every route; on the generated route it also records the same two bytes as
owned inside the host callback. These are overlapping scoped observations, not
two copies to add together. The generated boundary receives and returns only
`i64`; there is no body buffer parameter at that boundary. The committed raw
CSV predates this instrumentation. The ledger still does not measure copies
internal to reqwest, HTTP decoding before the explicit copy, UTF-8 validation,
or any other required RI-13 payload shape.

| Route | Semaprax boundary | Host body handling | Exact fixture-owned copied bytes | Process allocations |
| --- | --- | --- | --- | --- |
| Direct Rust | None | `Response::bytes()` then `Vec<u8>` | 2 foreign-body bytes per call | Unmeasured |
| Handwritten adapter | Explicit `i64` checks | `Response::bytes()` then `Vec<u8>` | 2 foreign-body bytes per call | Unmeasured |
| Generated Semaprax | Selected `i64` request/result | `Response::bytes()` then callback-owned `Vec<u8>` | 2 foreign-body bytes and the same 2 callback-captured bytes per call | Unmeasured |
