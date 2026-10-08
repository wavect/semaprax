# Byte Widening v1

Status: implemented bounded addition with focused local executable evidence.

Audience: language users, agent authors, and compiler contributors.

## Exact operation

`i64_from_u8(value: u8) -> i64` is a reserved compiler-owned intrinsic with
stable identity `core.num.i64_from_u8`. It returns exactly the unsigned byte
value in `0..=255`. Every `u8` fits in `i64`; the operation neither fails nor
allocates. It does not reinterpret a signed byte, loop to count its value,
truncate, or create a new status domain. Its operand is evaluated once in the
ordinary left-to-right order. Failure while evaluating that operand remains
sticky and follows its ordinary cleanup path.

The operation is pure and copies its operand. Expressions, contracts, bounded
loop conditions and bodies use the same exact signature. Wrong argument types
remain `SPX-T205`; wrong arity is `SPX-T204`; an authored declaration with this
name is `SPX-S113`. There is no implicit `u8` coercion. Direct HIR must carry
an exact monomorphic call, no type arguments or generic instance, one `u8`
argument, and an `i64` result; the existing intrinsic verifier independently
rejects forged metadata with `SPX-H006`.

Calls use the existing ordinary Call AST/HIR/graph projection. The identity is
an additive intrinsic table entry, outside the frozen Conversions v1 and
Project builtin catalogs. No graph/prelude/schema change is required. Source
that does not name the operation retains its projection and target bytes.
The [Conversions v1](LANGUAGE-ERGONOMICS-V1.md) family keeps its exact catalog
and checked statuses. [Integer Numeric Profile v2](INTEGER-NUMERIC-PROFILE-V2.md)
admits its checked integer conversions on Wasm. Additive aggregate Wasm admits
float conversions through [Wasm Text Toolkit v1](WASM-TEXT-TOOLKIT-V1.md);
frozen internal String selectors retain their `SPX-W116` refusal. The new byte widening uses inline
native `(int64_t)` and Wasm `i64.extend_i32_u`; it needs no String runtime or
host conversion import. Existing closed target signature, ownership and
capability rules still apply; this addition does not expand a public byte/text
profile's closed call inventory. The additive internal Copy Variant String
profile can lower the operation over its already admitted `u8` values; the
older internal String profile still excludes those values.

## Borrowed text without an owned copy

A borrowed `str` already provides an allocation-free route through
[Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md):

```semaprax
module app.byte;

@id("app.read")
fn read(text: borrow str, index: usize) -> i64
{
    let view = str_as_bytes(text);
    match byte_get(view, index) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => -1, }
}

@id("app.main")
fn main() -> i64
{
    let text = "é";
    let raw = string_as_str(text);
    read(raw, 0usize)
}
```

This returns `195`, the first UTF-8 byte of `é`. Byte offsets may split a
Unicode scalar; embedded NUL is ordinary data. `byte_get` returns `None` for
an empty view, an index equal to its length, any larger index, and
`18446744073709551615usize`. The example explicitly chooses `-1` for `None`;
the intrinsic does not impose a sentinel or bounds failure. Full-width index
comparison precedes conversion to a physical address. Reading the borrowed input creates no owned copy; this example
initializes its input owner explicitly. Existing root provenance, view lifetime and loan rules are unchanged.

The bundled `std.bytes.get_or` and `std.bytes.byte_to_i64` remain compatible
library choices in their existing Project profile. Their frozen source and
catalog are not replaced by this addition. The additive [Borrowed Text Byte Access v1](BORROWED-TEXT-BYTE-ACCESS-V1.md)
provides `str_byte_at(text, index)` for the same total read without the view
binding.

## Focused executable gates

The `language::byte_widening` module owns canonical source/graph equivalence,
exact intrinsic identity, source diagnostic and hostile HIR controls, all 256
byte values, contracts/loop conditions/lazy operands, interpreter/native
O0/O2 scalar conversion without allocation and exact input-owner settlement, and deterministic Node Core Wasm execution.
Its borrowed-text corpus checks UTF-8 bytes, NUL, empty/out-of-range reads,
`4294967296usize` and `18446744073709551615usize` on the same three backends.
Existing Conversions v1 refusal and old internal String profile refusal are
asserted independently; the explicit Copy profile executes the new operation.
The `source_verify::iterative_verifier_tests::byte_widening_matches_recursive_oracle`
unit gate preserves both verifier projections. No hosted evidence
or promotion of broader frozen text profiles is claimed by this document.
