# Borrowed Text Byte Access v1

Status: implemented bounded addition with focused local executable evidence.

Audience: language users, agents, and compiler contributors.

`str_byte_at(value: borrow str, index: usize) -> Option<u8>` is the reserved
compiler-owned operation `core.str.byte-at`. It borrows its UTF-8 input and
copies the full unsigned 64-bit index. It returns `Some(byte)` exactly when
`index` is below the input byte length, otherwise `None`, including an empty
input, an index equal to length, and `18446744073709551615usize`. Its indexing
is identical to `byte_get` over `str_as_bytes(value)`: byte offsets may split a
Unicode scalar, and embedded NUL is ordinary data. It does not allocate, clone,
consume, retain, return a borrowed view, or introduce a checked bounds failure.
Its `Option<u8>` result is ordinary Copy data.

```semaprax
module app.read;

@id("app.read")
fn read(text: borrow str, index: usize) -> i64
{
    match str_byte_at(text, index) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => -1, }
}

@id("app.main")
fn main() -> i64
{
    let text = "é";
    let raw = string_as_str(text);
    read(raw, 0usize)
}
```

This returns `195`, the first UTF-8 byte. The source chooses `-1` for `None`;
the accessor defines no sentinel. [Byte Widening v1](BYTE-WIDENING-V1.md)
converts the successful byte exactly. A host-provided or forwarded `borrow
str` uses the same operation without creating the example's input owner.
Literals and owned `string` are not borrowed-str operands: bind
`string_as_str(owner)` first. Existing input limits, UTF-8 boundary validation,
root lifetime and loan rules are unchanged.

The ordinary Call projection carries the exact reserved identity, no generic
instance or type arguments, one Borrow Str argument, one Value Usize argument,
and a Value `Option<u8>` result. Shared source/HIR signature tables authenticate
those facts. Wrong operand types are `SPX-T205`, wrong arity is `SPX-T204`,
explicit type arguments are `SPX-T225`, and declaring the name is `SPX-S113`.
Independent HIR checks reject forged types, ownership or generic metadata with
`SPX-H006`. Loop admission authenticates the existing immutable named text
input and traverses the index expression under ordinary loop rules. Argument
evaluation remains once, left to right, with the existing sticky failure path.
The operation creates no loan or cleanup root of its own.

Native C11 performs the unsigned length comparison before dereferencing the
borrowed pointer and writes the existing canonical Option tag/payload. Aggregate
Core Wasm uses the same authenticated byte carrier and trusted full-width
`byte_get` import as the existing byte-view route. The interpreter reads the
immutable borrowed input and returns its existing Option byte value. All three
preserve the original input owner. Neither target narrows a physical index
before its length proof. No new runtime allocation or host authority is added.

This is an additive intrinsic, outside the closed Useful Text Consumer v1
operation list; old public borrowed-text Wasm exports retain their Option/index
refusal (`SPX-W119`), and standalone internal String profiles retain their
borrowed-Str refusal. Native internal/source execution and already admitted
aggregate Wasm shapes implement it. Frozen source that never calls the operation
keeps its graph/prelude and output bytes. No new graph, prelude or ABI schema
is needed; the old byte/text operation names keep their meanings.

The owning `language::borrowed_text_byte_at` gate checks exact graph identity,
canonical round trip, cleanup-inert reads, inferred Option layout/runtime
selection, source diagnostics, hostile HIR, named-input loop/index admission,
and frozen public-text refusal. `language::byte_widening` runs both direct and
byte-view spellings on identical Unicode/NUL/empty/full-width inputs through
interpreter, native O0/O2 and Node Wasm, with exact input allocation/drop balance
and direct loop execution. The source-verifier recursive oracle has its own
`borrowed_text_byte_at_matches_recursive_oracle` unit control. These focused
local gates pass; they are not hosted evidence or promotion of a broader text profile.
