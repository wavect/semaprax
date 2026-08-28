# Portable SIMD Eligibility Report v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: integration tool authors and compiler contributors.

`semaprax simd-report <file.spx>` is a deterministic, read-only projection
that performs a static vectorization-eligibility analysis of one verified
module. It is the first executable slice of the completion-matrix row "SIMD
and GPU" under Compiler and output targets. The analysis is derived
exclusively from the real resolved HIR nodes (`hir::resolve` over the verified
program) of admitted explicit-ID monomorphic effect-free scalar functions. It
emits no SIMD codegen or intrinsics, emits no SPIR-V/WebGPU/GPU kernels,
makes no autovectorization claim about any backend, executes no target, and
changes no source.

## Command

```sh
semaprax simd-report <file> [--max-bytes N]
```

- There is no selection flag: the report always describes the whole module,
  so two runs over the same bytes are byte-identical.
- `--max-bytes` (default 64 KiB, bounds follow the Agent Context byte limits)
  bounds the whole envelope. Overflow fails closed with `SPX-V102`; output is
  never truncated or repaired.
- The output is one canonical compact JSON envelope plus one trailing
  newline.

## Admission model

A function is admitted when it has an explicit stable identity, is
monomorphic, declares no effects, and has only by-value parameters and a
result whose types are primitive scalars. Unlike the ABI/package profile this
deliberately includes `i32`, `u8`, `f32`, `f64`, `bool`, and `char`
signatures so their bodies can be analyzed and their ineligible expressions
reported honestly. Every other function is recorded as an exclusion with one
closed reason: `automatic_identity`, `generic_function`,
`declared_effects`, `unsupported_parameter_mode`, or
`non_scalar_signature`. Exclusions never abort generation; a module without
admitted functions yields a valid empty inventory. No backend lane runs, so
no backend diagnostic can fail the command.

## Reported facts

The payload carries, in fixed key order: `schema`, `source` (`path`,
`revision`, domain-separated source digest), `limits`, `module` (name,
`functions_total`, `functions_admitted`, `functions_excluded`),
`analysis_scope` (fixed `pure_straight_line_arithmetic_only`), `lane_model`,
`operation_table`, `functions`, `exclusions`, and fixed `nonclaims`.

### Lane model

The fixed portable lane model assumes a 128-bit lane budget, proposes only
widths {2, 4, 8}, and fixes per-element-type width ceilings within one
register, capped at the largest admitted width: `i64`/`f64` → 2,
`i32`/`f32` → 4, `u8` → 8. The proposed width of a region is selected by a
documented deterministic rule: scan the closed candidate widths from largest
to smallest and take the first feasible one, where feasibility requires
`w ≤ ceiling(element_type)` and `w ≤ operators + leaves` (the region's
element count). Every region has at least one operator and one leaf, so the
fallback width 2 is always feasible and the rule is total.

### Portable operation table

The closed table maps each admitted arithmetic operator and element class to
a portable lane-operation name — `int_lane.add/sub/mul/neg` for integer
element types (`i64`, `i32`, `u8`) and `fp_lane.add/sub/mul/neg` for
`f32`/`f64`. These names are portable descriptors only: they name no ISA
intrinsic and imply no emitted code.

### Functions

Functions appear in canonical bytewise stable-ID order. Each admitted
function carries:

- `signature_element_types` — parameter types then result type.
- `regions` — every maximal pure straight-line arithmetic sub-expression:
  a subtree of `+`/`-`/`*` and unary `-` over one lane-eligible element type
  whose leaves are plain numeric literals or projection-free numeric places.
  Regions are discovered top-down at the highest such node and listed in
  pre-order traversal order (source evaluation order). Each entry carries its
  index, canonical rendered root text, a domain-separated `root_sha256`
  (`semaprax.simd-report.region.v1`) over the exact root text, the element
  type, operator and leaf counts, the proposed width, and the closed portable
  operation sequence in post-order evaluation order.
- `ineligible` — every non-covered expression with exactly one closed
  reason, in pre-order traversal order with contract clauses first (requires
  then ensures, rendered by the canonical expression formatter):
  - `call` — call expressions (including native Rust imports);
  - `contract` — requires/ensures clause roots;
  - `division_remainder` — `/` and `%`;
  - `bool_mixing` — comparisons, `&&`/`||`, `!`, and boolean leaves;
  - `char_operation` — char literals and places;
  - `mutation_target` — assignment stores (recorded once, never descended);
  - `computed_operand` — arithmetic operators whose operands are not plain
    values (calls, projections, division results, control flow), so they
    cannot join a pure straight-line region;
  - `control_flow` — `if`/`match`/try expressions (their branches are still
    descended into and may contain regions);
  - `aggregate_operation` — record/variant construction, update,
    projection, and aggregate places;
  - `scalar_leaf` — lone literals or projection-free numeric places outside
    any region, which carry no operation to pack.
- `effect_freedom` — justification facts: the declared effect list (always
  empty for admitted functions), the closed tokens `declared_effects_empty`
  plus `no_call_expressions_in_body` or
  `calls_recorded_as_ineligible` derived from the actual body scan, and the
  exact `call_count`/`assignment_count` facts.

## Envelope and verification

`simd_report::generate` returns canonical compact JSON with fixed key order:
an outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is
the domain-separated SHA-256 of the exact payload bytes
(`semaprax.simd-report.payload.v1`) and `bytes` is their length.

`simd_report::verify_envelope` independently replays the envelope: exact
envelope shape and payload key set, declared byte count, outer digest over
the exact payload bytes, fixed analysis scope, the fixed lane model, the
closed portable-operation table, the fixed nonclaims, module counts against
the listings, both closed vocabularies, strict bytewise ordering across both
listings without duplicates, per-region digests, index continuity, lane
width feasibility against the declared model, operation membership for each
region's element class, operator-count agreement, effect-freedom facts, and
the call-token/count consistency. Any mutation anywhere in the envelope
invalidates verification — including forged-but-re-signed mutations caught
by the inner replay rules.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-V1xx` family: `SPX-V101` options, `SPX-V102` budget exhaustion,
`SPX-V103` envelope/HIR consistency.

## Nonclaims

The report explicitly claims none of the following: no SIMD codegen or
intrinsics emission, no SPIR-V/WebGPU/GPU kernels, no autovectorization
behavior in any backend, no target execution, read-only behavior with no
source changes, and static eligibility description only. The completion-matrix
row remains Partial because portable SIMD lowering, SPIR-V/WebGPU/platform
kernels, and memory/effect rules remain unimplemented.

## Evidence

Executable evidence lives in `tests/simd_report_v1.rs` plus module tests in
`src/simd_report.rs`: pinned golden envelope digests over
`examples/calculator.spx` and `examples/meaning.spx`, byte-identical double
runs, every function-admission exclusion reason, every per-expression
ineligibility reason exercised against real programs, lane-width feasibility
including ceiling and tie-break cases, cross-consistency proving that
reported region operators equal the real Add/Sub/Mul/Neg HIR nodes of the
same program and division entries equal the real Div/Rem HIR nodes, tamper
rejection per digest field including forged-but-re-signed envelopes, drift
binding, budget exhaustion, determinism, and CLI exit-code contracts.
