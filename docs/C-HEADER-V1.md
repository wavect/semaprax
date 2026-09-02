# C Header Emission v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax c-header <file.spx>` is a deterministic, read-only projection that
derives one C11 header from verified program facts for explicitly selected
public monomorphic scalar functions. It is the first executable slice of the
completion-matrix row "C and Objective-C" under Ecosystem interoperability.
It imports no header, imports no raw binding, generates no safe wrapper,
performs no Objective-C mapping, maps no strings or buffers, compiles
nothing, executes nothing, and changes no source.

## Command

```sh
semaprax c-header <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-header]
```

- `--function` is required at least once; selections may be display names or
  explicit stable IDs, may repeat the flag, and may use comma lists. Between
  1 and 64 unique targets are accepted; duplicates and unknown targets fail
  closed (`SPX-D101`, `SPX-D102`). Two tokens that resolve to the same
  declaration are rejected.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-D103`; output is
  never truncated or repaired.
- `--emit-header` prints the bare deterministic header bytes instead of the
  authenticated envelope. The default output is one canonical compact JSON
  envelope plus one trailing newline.

## Admission model

Only explicitly selected functions are considered; nothing is emitted for
unselected declarations. A selected function is admitted only when it has an
explicit stable identity, is monomorphic, declares no effects, has only
by-value direct parameters over the full Copy-scalar surface (`i64`, `i32`,
`u8`, `bool`, `f32`, `f64`, `char`; mixed signatures allowed), and returns a
direct scalar from that same surface.
Every other selected function is recorded as an exclusion with one closed
reason: `automatic_identity`, `generic_function`, `declared_effects`,
`unsupported_parameter_mode`, `unsupported_parameter_type`, or
`unsupported_result_type`. Exclusions never abort generation; an
all-excluded invocation yields a valid empty header. If at least one function
is admitted, the production native C11 lane must succeed; any backend
diagnostic (for example the resource gate) fails the whole command closed.

## Header content

Emitted declaration lines are extracted verbatim from the actual
`codegen::emit_c` native projection — exactly one prototype line must exist
per admitted symbol or the command fails with `SPX-D105` — so every header
declaration matches the ABI the native backend really emits, including the
`spx_status_token` return, the leading `struct spx_context *spx_ctx`
parameter, positional unnamed parameters, and the `*spx_result_out` out
parameter.

Each admitted function carries a generated block comment containing only
typed facts: the display name, the persistent stable ID, each `requires` and
`ensures` clause rendered by the canonical expression formatter, the declared
effect set (`none` when empty), the fixed status-contract note that
`*spx_result_out` is written only at the final success commit, and the fixed
ownership annotation `caller-free / by-value scalars`. Derived comment text
is rejected with `SPX-D104` if it contains `*/`, newlines, carriage returns,
or control characters, so host input can never terminate a comment or smuggle
bytes into the artifact. Functions are ordered bytewise by stable identity.

The include guard is derived only from the sorted admitted stable identities
through a domain-separated SHA-256 digest (`semaprax.c-header.guard.v1`),
formatted as `SPX_HEADER_` plus 32 lowercase hex characters. Formatting-only
source edits keep the guard byte-identical; renames that preserve identities
keep the guard; renames that change an admitted identity change the guard.
The preamble records the graph revision and admitted count.

## Envelope and verification

`c_header::generate` returns canonical compact JSON with fixed key order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.c-header.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `selection` (`requested`,
  `functions_total`, `admitted`, `excluded`), `functions` (each with
  `stable_id`, `name`, `symbol`, verbatim `signature`, per-declaration
  domain-separated digest, and `matches_native`), `exclusions`, embedded
  `header_sha256`, embedded `header` text, and fixed `nonclaims`.

`c_header::verify_envelope` independently recomputes the outer payload digest
over the exact serialized payload bytes, re-checks the declared byte count,
and re-authenticates the embedded header digest before returning the header.
Any mutation anywhere in the envelope invalidates verification.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-D1xx` family: `SPX-D101` options, `SPX-D102` selection,
`SPX-D103` budget exhaustion, `SPX-D104` hygiene, `SPX-D105` envelope/native
consistency.

## Evidence

Executable evidence lives in `tests/offline_package/c_header_emission.rs`,
`tests/interop_scalar_widen_v1.rs`, plus module
tests in `src/c_header.rs`: pinned golden envelope and path-independent
header digests over `examples/meaning.spx`, byte-identical double runs,
verbatim cross-consistency against the native projection, every exclusion
reason exercised against real programs, selection edge cases, guard
stability under formatting-only drift and display-name-only renames, guard
change under identity rename, budget-exhaustion failure, tampered-envelope
rejection, and CLI exit-code contracts. No C compiler is invoked and no
target execution is claimed; hosted promotion remains pending.

## Scalar-surface widening (2026-08-23)

The shared admission profile was widened from by-value `i64`/`bool` to the
full Copy-scalar surface: `i64`, `i32`, `u8`, `bool`, `f32`, `f64`, and
`char` parameters and results, with mixed signatures allowed. Headers need no
rendering changes because declaration lines are extracted verbatim from the
production native projection, which already emits `int64_t`, `int32_t`,
`uint8_t`, `uint32_t` (for `char`), `float`, `double`, and `bool` for those
scalars; widened prototypes are pinned byte-level against that projection in
`tests/interop_scalar_widen_v1.rs`. Envelope shape, guard derivation,
digest domains, hygiene rules, budget behavior, diagnostics
(`SPX-D101`–`SPX-D105`), and nonclaims are unchanged, all pre-existing pinned
KATs remain green, and no new diagnostic codes were needed. Still nonclaimed:
imports, raw bindings, safe wrappers, Objective-C mapping, string or buffer
mappings, compiled conformance evidence, and any target execution.

See also [PROPERTY-TESTS-V1.md](PROPERTY-TESTS-V1.md) for the sibling
read-only scalar analysis tranche and its shared admission profile.

The additive [Project image C artifact lane](IMAGE-C-ARTIFACTS-V1.md) reuses
this admission, header rendering, hygiene and exact native-prototype extraction
for manifest-selected stable IDs across canonical Project sources. It emits
the checked linked native C11 source as a separate artifact and independently
rebuilds the complete Project before regeneration. It preserves this command's
single-file bytes and exclusions, and adds no public linkage or compiled
consumer support.
