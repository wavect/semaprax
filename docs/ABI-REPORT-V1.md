# Canonical ABI Report v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax abi-report <file.spx>` is a deterministic, read-only projection that
describes, for explicitly selected public monomorphic scalar functions, both
the native fast ABI and the portable canonical ABI of the same declaration. It
is the first executable slice of the completion-matrix row "Portable canonical
ABI and native fast ABI" under Ecosystem interoperability. It is a report and
descriptor only: it maps no interface semantics beyond the selected scalar
exports, performs no borrowing (the slice is copy-only), runs no
cross-language conformance suite, compiles nothing, executes nothing, and
changes no source.

## Command

```sh
semaprax abi-report <file> --function name|stable-id[,...] [--function ...] [--max-bytes N]
```

- `--function` is required at least once; selections may be display names or
  explicit stable IDs, may repeat the flag, and may use comma lists. Between
  1 and 64 unique targets are accepted; duplicates and unknown targets fail
  closed (`SPX-A201`, `SPX-A202`). Two tokens that resolve to the same
  declaration are rejected.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-A203`; output is
  never truncated or repaired.
- The default output is one canonical compact JSON envelope plus one trailing
  newline.

## Admission model

The admission profile mirrors C Header Emission v1 exactly: a selected
function is admitted only when it has an explicit stable identity, is
monomorphic, declares no effects, has only by-value direct parameters over
the full Copy-scalar surface (`i64`, `i32`, `u8`, `bool`, `f32`, `f64`,
`char`; mixed signatures allowed), and returns a direct scalar from that same
surface. Every other selected function is recorded as an exclusion with one
closed reason: `automatic_identity`,
`generic_function`, `declared_effects`, `unsupported_parameter_mode`,
`unsupported_parameter_type`, or `unsupported_result_type`. Exclusions never
abort generation; an all-excluded invocation yields a valid empty report. If
at least one function is admitted, both production backends must succeed; any
native or HIR diagnostic fails the whole command closed.

## Reported facts

Each admitted function entry carries two sections:

- `native` — the fast ABI facts for the Native64 lane. `signature` is the
  exact prototype line extracted verbatim from the production native C11
  projection (`codegen::emit_c`); exactly one must exist per admitted symbol
  or the command fails with `SPX-A204`, so every reported signature matches
  the ABI the backend really emits. `parameters` and `result` carry the
  language type, the exact C type (`int64_t`, `int32_t`, `uint8_t`,
  `uint32_t` for `char`, `float`, `double`, or `bool`), and the size and
  alignment taken from the checked compiler layouts
  (`aggregate_layout::scalar_size_align` on `Native64`: `i64` 8/8, `i32`
  4/4, `char` 4/4, `u8` 1/1, `f32` 4/4, `f64` 8/8, `bool` 1/1), each with
  `mode: value`. `parameter_passing` records
  by-value copy semantics, and `status_out_contract` records that the lane
  returns `spx_status_token`, receives a leading
  `struct spx_context *spx_ctx`, writes `<c_type> *spx_result_out`, and
  publishes the result only at the final success commit.
- `canonical` — the portable mapping used by the Public Scalar Export
  Profile v1 Core-Wasm lane under `semaprax.wasm-scalar.v1`: every admitted
  scalar is rendered exactly as the backend's Core-Wasm value-type lowering
  renders it (`i64` stays `i64`, `f32` stays `f32`, `f64` stays `f64`, while
  `bool`, `i32`, `u8`, and `char` all ride the `i32` lane). `export` is the
  injective raw symbol `spx_scalar_` plus the lowercase hex encoding of the
  stable ID. `bool_boundary` documents the real adapter behavior — every `bool`
  parameter and the `bool` result trap unless they are canonical Wasm
  booleans (`0` or `1`). `copy_behavior` is fixed to `copy` for this slice;
  nothing here describes borrowing.

Functions are ordered bytewise by stable identity.

## Envelope and verification

`abi_report::generate` returns canonical compact JSON with fixed key order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.abi-report.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `selection` (`requested`,
  `functions_total`, `admitted`, `excluded`), `functions` (each embedding the
  verbatim native `signature` under its own domain-separated digest plus the
  rebuilt-and-digested canonical object text), `exclusions`, and fixed
  `nonclaims`.

`abi_report::verify_envelope` independently recomputes the outer payload
digest over the exact serialized payload bytes, re-checks the declared byte
count, rebuilds every canonical object from its parsed fields, and
re-authenticates both embedded signature digests per function before
returning the summaries. Any mutation anywhere in the envelope invalidates
verification.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-A2xx` family: `SPX-A201` options, `SPX-A202` selection,
`SPX-A203` budget exhaustion, `SPX-A204` envelope/backend consistency.

## Evidence

Executable evidence lives in `tests/offline_package/abi_report.rs`,
`tests/language/interop_scalar_widen.rs`, plus module tests in `src/abi_report.rs`:
pinned golden envelope KATs over `examples/calculator.spx` and
`examples/meaning.spx`, byte-identical double
runs, verbatim cross-consistency against the native projection, byte-level
cross-consistency of every portable mapping and raw export name against the
real Core-Wasm module emitted by `wasm::emit_module_with_scalar_exports` for
the same program and selection (and, for widened scalars, against the real
Core-Wasm module emitted by the ordinary lane for the same functions),
checked-layout agreement including the
Native64/Wasm32 `bool` divergence, every exclusion reason exercised against
real programs, CLI exit-code contracts, budget-exhaustion failure, and tamper
rejection per digest field. No compiler, Node runtime, browser, or any other
target execution is involved, and hosted promotion remains pending.

## Scalar-surface widening (2026-08-23)

The admission profile was widened from by-value `i64`/`bool` to the full
Copy-scalar surface: `i64`, `i32`, `u8`, `bool`, `f32`, `f64`, and `char`
parameters and results, with mixed signatures allowed. Nothing else changed:
envelope shape, digest domains, key order, ordering, budget rules,
diagnostics (`SPX-A201`–`SPX-A204`), and nonclaims are byte-compatible for
previously admitted programs, and all pre-existing pinned KATs remain green.

- Native facts now cover every widened scalar: C spellings mirror the
  production projection exactly (`int32_t`, `uint8_t`, `uint32_t` for `char`,
  `float`, `double`), and sizes/alignments come unchanged from
  `aggregate_layout::scalar_size_align(Native64)`.
- Canonical rows report the exact Core-Wasm value types the backend lowers to
  (`i32` for `bool`/`i32`/`u8`/`char`; exact-width `f32`/`f64`). For widened
  selections these are authenticated byte-level against the ordinary
  Core-Wasm module's type section for the same functions.
- The Public Scalar Export Profile v1 adapter lane
  (`emit_module_with_scalar_exports`) has since been widened to the same
  Copy-scalar surface, so a widened canonical row's `spx_scalar_` export name
  now names an adapter the wrapper lane really emits. The `bool_boundary` note
  generalizes there: `u8` and `char` adapters trap on the same principle. No
  new diagnostic codes were needed — unsupported shapes keep failing closed
  under the existing exclusion vocabulary.

See also [C-HEADER-V1.md](C-HEADER-V1.md) for the sibling read-only native
projection tranche and its shared admission profile, and
[WASM-SCALAR-EXPORTS-V1.md](WASM-SCALAR-EXPORTS-V1.md) for the portable lane
whose boundary behavior this report documents.
