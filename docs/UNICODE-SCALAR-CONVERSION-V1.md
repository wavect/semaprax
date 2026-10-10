# Unicode Scalar Conversion v1

Audience: language users and compiler contributors.

Status: source implementation and owning regressions authored; current-head
interpreter, native and Core Wasm execution is pending. This is the OPT-725
conversion seam; it does not qualify a general application JSON codec.

`char_from_i64(value: i64) -> char`, persistent identity
`core.num.char_from_i64`, copies exactly one Unicode scalar value. Accepted
integers are `0..=1114111` (`0..=0x10FFFF`), excluding `55296..=57343`
(`0xD800..=0xDFFF`). NUL, noncharacters, and supplementary-plane scalars are
accepted. This operation neither decodes bytes nor normalizes Unicode.

A negative integer, surrogate code point or integer above the scalar range
selects the existing `semaprax.convert.v1`, code `1`, adapter-class,
nonretryable status. The complete signed i64 is checked before narrowing;
high bits cannot disappear into a valid character. The operand evaluates once
before conversion. Ordinary left-to-right call staging and lazy boolean/branch
evaluation apply. A selected failure remains sticky, runs canonical cleanup,
and leaves the result unpublished. The conversion allocates no owning carrier
and acquires no capability. `string_from_char` may subsequently materialize the
scalar's UTF-8 bytes under its existing allocation and ownership contract.

The operation is a reserved compiler intrinsic represented by the existing
ordinary monomorphic Call AST/HIR. No new parser grammar, graph/cache schema,
prelude catalog, public ABI or host import is introduced. Source checks the
single i64 operand (`SPX-T204` for arity, `SPX-T205` for type), refuses explicit
type arguments (`SPX-T225`), and reserves the authored function name and exact
stable identity (`SPX-S113`). The shared String operation table's identities
are unavailable to every authored declaration kind, including fields and
variant cases. HIR independently checks declaration IDs and function names in
the index, executable headers, generic templates and attached instances before
any operation can dispatch. An origin label, including `CompilerOwned` in
retained metadata, cannot confer intrinsic authority. Namespaced lookalikes and
ordinary nonfunction field names remain valid. HIR also checks operand/result types, ownership,
arity and absence of generic metadata (`SPX-H006`). Graph verification binds
the source and reserved identity; retained HIR must pass the same independent
validation after cache decoding.

The interpreter, native C11 and scalar/aggregate Core Wasm source paths lower
the same operation. Both Wasm paths select existing wire status `21`, which
maps to conversion domain/code `semaprax.convert.v1`/`1`. The aggregate path
uses the ordinary call commit and failure-expression cleanup plan. Native
checks the signed operand before its `uint32_t` cast and result publication.
Modules not naming this operation retain their prior projections;
`char_from_u8` remains the exact, infallible byte-to-same-scalar conversion.
The frozen Conversions v1 catalog and all existing target/public boundaries
remain unchanged. No new Unicode or collection resource limits are granted.

Owning executable gates (authored, not yet executed for this change):

- `--lib string_ops::unicode_scalar_tests`: canonical source/graph/cache
  roundtrip, source drift, forged retained calls, exact source diagnostics.
- `--lib hir::validation::string_intrinsic::tests`: every reserved String
  operation identity, authored declaration aliases, hostile retained origins,
  function/template/instance impersonation and admitted ordinary names.
- `--lib source_verify::iterative_verifier_tests::unicode_scalar_conversion_matches_recursive_oracle`:
  iterative and recursive source checking agree on valid and invalid domains.
- `--lib hir::validation::call_parameters::tests::string_signature_views_match_all_materialized_descriptors_at_zero_identity_budget`:
  the new intrinsic's borrowed signature matches its materialized descriptor.
- `--test language integer_profiles::unicode_scalar`: identical source on the
  interpreter, C11 O0/O2 and scalar plus aggregate Core Wasm, repeated failure
  and success, exact graph identity, all UTF-8 width boundaries, surrogate
  endpoints, signed extremes and a nonzero high-word operand. Separate sources
  exercise lazy evaluation, operand and sibling failure order, live Bytes
  cleanup and an owned String staged before conversion failure.

The existing integer/byte conversion and deterministic artifact gates remain
required. This source batch provides no measured agent token or cost savings.
