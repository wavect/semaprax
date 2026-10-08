# Byte Conversions v1

Audience: language users, agent authors, and compiler contributors.

Status: bounded implementation with focused local interpreter, native C11,
and scalar/aggregate Core Wasm verification in
`tests/language/integer_profiles.rs`.

Byte Conversions v1 extends the additive integer conversion profile with two
compiler-owned scalar operations:

| Name | Persistent identity | Signature | Meaning |
| --- | --- | --- | --- |
| `u8_from_i64` | `core.num.u8_from_i64` | `(value: i64) -> u8` | Return the exact byte for `0 <= value <= 255`; otherwise fail |
| `char_from_u8` | `core.num.char_from_u8` | `(value: u8) -> char` | Return the Unicode scalar with the same value, U+0000 through U+00FF |

`u8_from_i64` evaluates its operand once. A negative value or a value above
255 selects `semaprax.convert.v1` code 1, class `adapter`, retryable false.
Failure is sticky, settles canonical cleanup, and leaves the result
unpublished. The operation never truncates or wraps. `char_from_u8` is exact
and infallible; it is not an ASCII assertion and therefore accepts every byte.

Both operations take and return Copy scalars. Source verification reserves the
names, checks the exact arity and operand type, and resolves calls to their
persistent identities. HIR validation independently authenticates operand and
result types. Canonical formatting preserves their source spelling, while the
semantic graph records the persistent callee identities above.

The reference interpreter, native C11, and scalar and aggregate Core Wasm
lanes implement the same behavior. Native C checks the signed range before
casting. Wasm checks the signed i64 range before `i32.wrap_i64`; the wrap is
therefore only a representation step after proof that the value fits. The
existing additive Wasm failure wire status 21 maps to
`semaprax.convert.v1` code 1. Converting `u8` to `char` is an exact scalar
representation change on every backend.

```semaprax
let byte = u8_from_i64(value);
let character = char_from_u8(byte);
```

No target limit, public ABI, implicit coercion, or frozen Conversions v1
catalog changes. The focused owner harness covers 0 and 255, both rejected
adjacent values, all 256 byte values, non-ASCII U+00FF, stable source
diagnostics, source/graph round trips, and interpreter/native/scalar-Wasm/
aggregate-Wasm parity.
