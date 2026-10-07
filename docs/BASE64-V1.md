# Base64 v1

Status: implemented bounded source profile. The original encoder was
hosted-green on the named `std-library-depth` CI job at 78ee5107, outside the
v0.4.0 release baseline. Strict padded decoding adds focused local
interpreter, native C11 and Core Wasm gates; hosted promotion remains separate. Unpadded and URL-safe alphabets
remain out of scope.

Audience: language users, compiler contributors, standard-library authors, and
backend implementers.

This bundled `std.encoding.base64` package provides pull-based padded standard
Base64 encoding plus strict padded decoding over borrowed byte views. It adds
no buffer or owned byte type: callers read encoded or decoded bytes by index,
or decode only after complete validation and capacity preflight into a
caller-owned `std.io.Writer`. Ordinary internal function imports admit the
combination of a borrowed byte view and an explicitly imported resource-free
owned byte record. Public signature/ABI admission remains unchanged.
[Standard Library v1](STANDARD-LIBRARY-V1.md) owns the package inventory this
profile extends.

Base64 encoding is a sibling package rather than more of `std.encoding` for the
reason that document already records for `std.data.json` and `std.io.lines`:
`std.encoding` carries no `profile` line, so it sits on the default Project v1
route, which admits only Copy scalar boundaries and rejects a `borrow
Slice<u8>` or `usize` parameter with `SPX-G174`. `std.encoding.base64` declares
`profile = "owned-data-api.v1"` and reaches the shared digit table and Writer
through ordinary exact-version `std.encoding` and `std.io` dependencies. The
alphabet and cursor types are therefore defined exactly once; identities are
`std.encoding.base64.*`, and all existing dependency identities stay exactly
as released.

## Encoding policy

Standard Base64 groups input into three-byte blocks and emits four digits per
block from the 64-symbol alphabet already defined by
`std.encoding.encode_base64_digit`, padding a short final block with `=`
(ASCII `61`) so every encoded length is a multiple of four.
`base64_len(input)` is that total encoded length for an `input`-byte source;
its postcondition pins the multiple-of-four result directly. The profile
performs no other rewriting: no line wrapping, no whitespace, and no alternate
alphabet.

## Decoding policy

Decoding accepts only a complete standard-alphabet input whose length is a
multiple of four. Empty input, full quanta, a one-pad tail, and a two-pad tail
are admitted. Padding may appear only in the final quantum, and the unused low
bits in the final significant digit must be zero. Thus `Zg==` and `Zm8=` are
canonical while `Zh==` and `Zm9=` are refused even if a permissive external
decoder would produce bytes for them. Whitespace, URL-safe digits, leading or
interior padding, excess padding, trailing bytes after padding, and incomplete
quanta are refused.

`base64_decode_error_kind` reports stable categories: `0` success, `1`
impossible length, `2` non-alphabet byte, `3` misplaced padding, and `4`
nonzero unused pad bits. `base64_decode_error_offset` reports the first
offending byte, or the input length for an incomplete quantum. These observers
do not mutate input or output.

## Operations

`base64_byte(view, index)` is the pull-based digit accessor: `requires index <
base64_len(byte_len(view))` and its postcondition pins every result to the
printable ASCII digit-or-pad range `43..=122`. There is no sequential state; a
caller may read digit `7` before digit `0`, or read a single digit without
producing the rest, and each call reconstructs the source block, the block
offset (`slot`), and the two padding conditions (`has_second`, `has_third`)
from `index` alone. `byte_at_or_zero(view, index)` is the internal, in-range
byte observer the accessor composes: it returns the numeric value of the byte
at `index`, or zero past the view's length, so a short final block still reads
its present bytes without a separate length branch at every call site.

`base64_decoded_len(input)` returns the exact output length for a valid input
and zero for an invalid input; callers distinguish invalid input with the error
kind observer. `base64_decoded_byte(input, index)` is the random-access decoded
byte and requires both a valid input and an in-range output index.

`base64_decode_into(input, own Writer) -> Writer` validates the whole input and
preflights its exact decoded length against the Writer's live cursor before
entering the body. Invalid input or insufficient capacity therefore fails the
contract before any byte is written. On success the Writer prefix and suffix
are preserved, the cursor advances by exactly `base64_decoded_len(input)`, and
the borrowed input remains unchanged.

## Boundaries

This profile adds no owned buffer, stream, or public export. The imported
Writer remains caller-owned, and the package grants no ambient filesystem,
process, or network authority. It does not claim line wrapping, MIME framing,
or a URL-safe or unpadded alphabet. `std.encoding`'s existing digit table,
contracts, quad helper, and identities are unchanged; this package imports the
digit conversion helpers and `std.io.Writer`.

## Focused local evidence

```sh
cargo test --locked -p semaprax --test project standard_library::base64
cargo test --locked -p semaprax --test project standard_library::every_public_declaration_has_a_std_identity_contracts_examples_and_conformance
cargo test --locked -p semaprax --test documentation
```

The conformance module checks nine encoder vectors against Python's
`base64.b64encode`: the empty input, `"f"`, `"fo"`, `"foo"`, `"foob"`,
`"fooba"`, `"foobar"`, the non-text bytes `\x00\xff\x10`, and `"Man"`, each in
its own short function folded into a failure bitmask so no chain approaches
the `SPX-H006` replay bound. Decoder conformance covers empty, full, one-pad and
two-pad inputs, exact Writer prefix/suffix preservation, strict error offsets
and categories, and an encoder-to-decoder round trip containing all 256 byte
values.

The package's examples and conformance execute on the interpreter, native C11
at `-O0` and `-O2`, and Core Wasm under Node through the standard-library gate.
Hostile interpreter cases reject a digit index at the padded length, any digit
of the empty input, malformed decode input, and insufficient output capacity
with the exact `requires`-false contract status. A
graph check replays the projection, pins both accessors to a borrowed view with
an empty owned inventory, and rejects a one-byte change to the padding policy.

This is bounded source-level Base64 encoding and strict padded decoding into a
caller-supplied buffer. It is not evidence of a streaming codec or public API.
