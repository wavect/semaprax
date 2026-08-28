# C++ Shim Projection v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax cxx-shim <file.spx>` is a deterministic, read-only projection that
derives one C++17-compatible header fragment from verified program facts for
explicitly selected public monomorphic scalar functions. It is the first
executable slice of the completion-matrix row "C++" under Ecosystem
interoperability. The emitted `extern "C"` declaration lines are extracted
verbatim from the production native C11 projection, so every shim declaration
matches the ABI the native backend actually emits. It imports no header,
parses no C++, generates no adapters or safe wrappers, maps no strings,
buffers, aggregates, or resources, compiles nothing, executes nothing, adds
no exception or ownership policy beyond the bounded slice below, and changes
no source.

## Command

```sh
semaprax cxx-shim <file> --function name|stable-id[,...] [--function ...] [--max-bytes N] [--emit-fragment]
```

- `--function` is required at least once; selections may be display names or
  explicit stable IDs, may repeat the flag, and may use comma lists. Between
  1 and 64 unique targets are accepted; duplicates and unknown targets fail
  closed (`SPX-X101`, `SPX-X102`). Two tokens that resolve to the same
  declaration are rejected.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-X103`; output is
  never truncated or repaired.
- `--emit-fragment` prints the bare deterministic fragment bytes instead of
  the authenticated envelope. The default output is one canonical compact
  JSON envelope plus one trailing newline.

## Admission model

The admission profile is exactly C Header Emission v1's: only explicitly
selected functions are considered; a selected function is admitted only when
it has an explicit stable identity, is monomorphic, declares no effects, has
only by-value direct parameters over the full Copy-scalar surface (`i64`,
`i32`, `u8`, `bool`, `f32`, `f64`, `char`; mixed signatures allowed), and
returns a direct scalar from that same surface.
Every other selected function is recorded as an exclusion with one closed
reason: `automatic_identity`, `generic_function`, `declared_effects`,
`unsupported_parameter_mode`, `unsupported_parameter_type`, or
`unsupported_result_type`. Exclusions never abort generation; an
all-excluded invocation yields a valid empty `extern "C"` block. If at least
one function is admitted, the production native C11 lane must succeed; any
backend diagnostic (for example the resource gate) fails the whole command
closed.

## Fragment content

Emitted declaration lines are extracted verbatim from the actual
`codegen::emit_c` native projection — exactly one prototype line must exist
per admitted symbol or the command fails with `SPX-X105` — so every shim
declaration matches the ABI the native backend really emits, including the
`spx_status_token` return, the leading `struct spx_context *spx_ctx`
parameter, positional unnamed parameters, and the `*spx_result_out` out
parameter. The declarations sit inside one `extern "C"` block so C++ name
mangling cannot silently mismatch the native symbols.

Each admitted function carries a generated block comment containing only
typed facts: the display name, the persistent stable ID, each `requires` and
`ensures` clause rendered by the canonical expression formatter, the declared
effect set (`none` when empty), the fixed status-contract note that
`*spx_result_out` is written only at the final success commit, and the fixed
ownership annotation `caller-free / by-value scalars`. No exception, memory,
or lifetime policy beyond this bounded slice is claimed. Derived comment text
is rejected with `SPX-X104` if it contains `*/`, newlines, carriage returns,
or control characters, so host input can never terminate a comment or smuggle
bytes into the artifact. Functions are ordered bytewise by stable identity.

The include guard is derived only from the sorted admitted stable identities
through a domain-separated SHA-256 digest (`semaprax.cxx-shim.guard.v1`),
formatted as `SPX_CXX_SHIM_` plus 32 lowercase hex characters.
Formatting-only source edits keep the guard byte-identical; renames that
preserve identities keep the guard; renames that change an admitted identity
change the guard. The preamble records the graph revision and admitted count.

## Envelope and verification

`cxx_shim::generate` returns canonical compact JSON with fixed key order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.cxx-shim.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `selection` (`requested`,
  `functions_total`, `admitted`, `excluded`), `functions` (each with
  `stable_id`, `name`, `symbol`, verbatim `signature`, per-declaration
  domain-separated digest, and `matches_native`), `exclusions`, embedded
  `fragment_sha256`, embedded `fragment` text, and fixed `nonclaims`.

The fixed nonclaims are `no_header_import`, `no_cxx_compilation`,
`no_exception_or_lifetime_policy_beyond_the_bounded_slice`,
`no_string_buffer_aggregate_or_resource_mappings`, `no_hosted_execution`,
and `read_only`.

`cxx_shim::verify_envelope` independently recomputes the outer payload digest
over the exact serialized payload bytes, re-checks the declared byte count,
and re-authenticates the embedded fragment digest before returning the
fragment. Any mutation anywhere in the envelope invalidates verification.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-X1xx` family: `SPX-X101` options, `SPX-X102` selection,
`SPX-X103` budget exhaustion, `SPX-X104` hygiene, `SPX-X105` envelope/native
consistency.

## Evidence

Executable evidence lives in `tests/cxx_shim_projection_v1.rs`,
`tests/interop_scalar_widen_v1.rs`, plus module
tests in `src/cxx_shim.rs`: pinned golden envelope and path-independent
`extern "C"` fragment digests over `examples/meaning.spx`, byte-identical
double runs, verbatim cross-consistency against the native projection, every
exclusion reason exercised against real programs, selection edge cases, guard
stability under formatting-only drift and display-name-only renames, guard
change under identity rename, budget-exhaustion failure, separate tamper
rejection for every digest field (outer digest, byte count, schema,
`matches_native`, source digest, per-declaration digest, signature text,
embedded fragment digest, and fragment text — including forged-but-re-signed
envelopes that only the inner replay catches), and CLI exit-code contracts.
No C++ compiler is invoked and no target execution is claimed; a stable shim
workflow, exception/ownership policy, maintained adapters, unsafe
classification, and hosted promotion all remain open.

## Scalar-surface widening (2026-08-23)

The shared admission profile was widened from by-value `i64`/`bool` to the
full Copy-scalar surface: `i64`, `i32`, `u8`, `bool`, `f32`, `f64`, and
`char` parameters and results, with mixed signatures allowed. Fragments need
no rendering changes because declaration lines are extracted verbatim from
the production native projection, which already emits `int64_t`, `int32_t`,
`uint8_t`, `uint32_t` (for `char`), `float`, `double`, and `bool` for those
scalars; widened `extern "C"` declarations are pinned byte-level against that
projection in `tests/interop_scalar_widen_v1.rs`. Fragment shape, guard
derivation, digest domains, hygiene rules, budget behavior, diagnostics
(`SPX-X101`–`SPX-X105`), fixed nonclaims, and the bounded ownership slice are
unchanged, all pre-existing pinned KATs remain green, and no new diagnostic
codes were needed. Still nonclaimed: C++ compilation or conformance, any
exception/memory/lifetime policy beyond the bounded slice, adapters, string,
buffer, aggregate, or resource mappings, and hosted execution.

See also [C-HEADER-V1.md](C-HEADER-V1.md) for the sibling C11 header tranche
that shares this admission profile and envelope discipline.
