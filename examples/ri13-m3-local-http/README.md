# RI-13 M3 local HTTP subprofile

This saved application uses an authored, canonical `source-local-future.v1`
Project, its generated Rust registration module, a caller-created Tokio
current-thread runtime, and locked `reqwest` against a local server. The
SEMAPRAX function validates its input and result through checked contracts;
the Rust effect handler returns a parsed numeric HTTP body. The checked
result is `84` from seed `41` and server body `43`.

From this directory, with Rust/Cargo 1.98.0 and an offline cache containing
the committed lock's packages:

```sh
export CARGO_TARGET_DIR="$PWD/target/ri13-m3"
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo run --locked --offline --bin prepare
cargo run --locked --offline --bin consumer
```

`prepare` reads the held Project, checks it, and writes deterministic
`src/generated.rs` bytes. That ignored local file is the only generated copy;
`consumer` compiles those bytes and checks the embedded Project revision,
source identity and suspension plan before registering the callback. The
consumer owns the executor and network capability. Neither preparation nor
generated code starts a runtime, accesses the network, or publishes a package.

The consumer runs success, loopback connection refusal, HTTP 503, invalid
numeric body, checked source postcondition failure, timeout, and cancellation
after server receipt. Every server accepts one request and checks for a retry.
`call_typed` preserves transport, timeout, status, and parse host errors
separately from checked source failures.
The consumer also records the exact application-owned copy from reqwest's
foreign response `Bytes` into callback-owned `Vec<u8>` storage. Its success,
invalid-body, and source-postcondition cases assert 2, 7, and 3 copied bytes;
the status, timeout, and cancelled calls assert zero because they do not reach
that conversion. The ledger does not claim reqwest buffering, HTTP decoding,
or UTF-8 validation copies.
It also sends a local 4,096-byte, zero-padded numeric response through the
same checked continuation and asserts exactly 4,096 callback-owned copied
bytes while preserving the scalar result `84`.
The focused Project harness loads the same saved source into a held Project and
runs the local HTTP cases inside the existing Project test binary, with no
nested Cargo build. It checks generated module rendering, demonstrates that
omitting the timeout permits the delayed response, and refuses held source
drift. The standalone application above separately compiles and executes the
generated module. The exact physical Project selector is
`ri13_m3::saved_m3_application_runs_offline_and_refuses_timeout_and_stale_binding_mutants`
in `--test project` with `--ignored --exact`.

Local evidence on 2026-10-04: Darwin arm64, Cargo/rustc 1.98.0. The exact
Project selector passed 1/1. Both locked offline commands above exited zero;
`prepare` printed `ri13-m3-prepared` and `consumer` printed
`ri13-m3-local-http-ok`. Running the built consumer with `omit-timeout`
exited 101 at the timeout assertion. Changing `response + seed` to
`response - seed` without regenerating, then running `expect-stale`, printed
`ri13-m3-stale-refused`; the source was restored afterward. These are local
receipts from the detached checkout, not fresh-checkout or Linux evidence.

The owning Project selector also runs the local HTTP route with a test-only
wrong host return and with its HTTP-status guard removed. The first produces
the checked-source result `83` rather than `84`; the second accepts a numeric
503 body rather than returning the typed `HttpStatus(503)` failure. Both differ
from the authentic integration oracle. These are mutation controls for the
selected route, not claims about arbitrary host capabilities.

The [local M3 measurement protocol](measurements/README.md) compares direct
Rust, an equivalent handwritten checked adapter, and the generated route
using raw per-request samples. It reports the narrow signature and copy
coverage without treating loopback timing as a general ABI-overhead result.

Developer friction in this subprofile: two `.spx` files, one Project manifest,
one pinned Cargo manifest/lock, one 21-line preparation command, and one Rust
consumer with a caller-authored HTTP effect handler and server. There are zero
handwritten FFI shims, unsafe blocks, per-foreign-function carrier conversions,
or checked-in generated SDK copies. The HTTP handler is required application
code, not inferred from SEMAPRAX source, and counts as one explicit escape
hatch. It maps reqwest failures into four typed host outcomes. The selected
SEMAPRAX body is scalar; the current v21 Project profile cannot combine its
`rust_async` export with RI-06 Url or RI-07/08 record/callback imports in one
linked Project. This example is therefore **M3 source-local HTTP only**; it
does not satisfy the full M1+M2+M3 application gate, benchmark comparison,
Linux evidence, the other required copied-byte cases, or public SDK publication
criteria of #371.
