# Integer Numeric Profile v2

Audience: language users and compiler contributors.

Status: bounded implementation; local evidence is owned by
`tests/language/integer_profiles.rs` and the existing byte-span corpus.

Named compiler operations extend Conversions v1 without casts or coercion:

| Name | Persistent identity | Operand | Result | Semantics |
| --- | --- | --- | --- | --- |
| `i64_from_u8` | `core.num.i64_from_u8` | u8 | i64 | Exact zero extension |
| `i64_from_i32` | `core.num.i64_from_i32` | i32 | i64 | Exact sign extension |
| `usize_from_u8` | `core.num.usize_from_u8` | u8 | usize | Exact zero extension |
| `i64_from_usize` | `core.num.i64_from_usize` | usize | i64 | Refuse values above i64 MAX |
| `usize_from_i64` | `core.num.usize_from_i64` | i64 | usize | Refuse negative values |

The last two names, signatures, identities and `semaprax.convert.v1` code 1
range failure are preserved from v1. Portable usize is u64 on every target.
Operands evaluate once, left to right; direct extension and bounded checks
consume constant semantic work regardless of magnitude. Failure is sticky,
settles canonical cleanup and leaves the result unpublished. Source and HIR
independently authenticate operand/result types; graph and revision identities
retain the exact compiler operation identity. The frozen prelude and existing
operations' identities are unchanged.

The interpreter, native C11 O0/O2, and ordinary scalar/aggregate Core Wasm
implement this integer profile. Modules selecting it may use portable u64
usize internally; Public Scalar Export v1 still refuses usize in public
parameter/result signatures. The additive Wasm failure wire status 21 means
`semaprax.convert.v1`, code 1. Generated browser runtime/status declarations
select this mapping only for modules naming an integer conversion, preserving
legacy generated runtime bytes otherwise. Float conversions and
`string_from_str` keep their existing narrower backend scope.

Integer `%` admits equal operand types i64, i32, u8 and usize; the result keeps
that type. Signed remainder truncates division toward zero, so -7i32 % 3i32
is -1i32. Zero divisor selects `semaprax.arithmetic.v1` code 6; signed MIN % -1
selects code 7, including the aggregate Core Wasm lane. Floats and mixed types
remain refused. The source/HIR operation and canonical cleanup status inventory
are the existing remainder operation, with no downstream plan repair.

`std.bytes.byte_to_i64` uses exact widening and `position_of(next_index)` uses
checked conversion of next_index - 1usize. Their public signatures, contracts,
stable identities, and subtract-one meaning remain unchanged. Counting loops
are eliminated; an out-of-range position fails with the conversion status.
