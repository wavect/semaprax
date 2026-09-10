# Base64 v1

Status: implemented bounded source profile; hosted-green on the named
`std-library-depth` CI job at 78ee5107, outside the v0.4.0 release baseline. Decoding and
unpadded or URL-safe alphabets remain out of scope.

Audience: language users, compiler contributors, standard-library authors, and
backend implementers.

This profile is the bundled `std.encoding.base64` package: pull-based padded
standard Base64 *encoding* over a borrowed byte view. It introduces no buffer,
no owned byte type, and no allocation; a caller reads each encoded digit with
`base64_byte(view, index)` and writes it wherever it chooses, in any order.
[Standard Library v1](STANDARD-LIBRARY-V1.md) owns the package inventory this
profile extends.

Base64 encoding is a sibling package rather than more of `std.encoding` for the
reason that document already records for `std.data.json` and `std.io.lines`:
`std.encoding` carries no `profile` line, so it sits on the default Project v1
route, which admits only Copy scalar boundaries and rejects a `borrow
Slice<u8>` or `usize` parameter with `SPX-G174`. `std.encoding.base64` declares
`profile = "owned-data-api.v1"` and reaches the shared digit table through an
ordinary `[dependencies] std.encoding = "=0.1.0"` import, so the alphabet is
defined exactly once; identities are `std.encoding.base64.*` and the
`std.encoding.*` identities stay exactly as released.

## Encoding policy

Standard Base64 groups input into three-byte blocks and emits four digits per
block from the 64-symbol alphabet already defined by
`std.encoding.encode_base64_digit`, padding a short final block with `=`
(ASCII `61`) so every encoded length is a multiple of four.
`base64_len(input)` is that total encoded length for an `input`-byte source;
its postcondition pins the multiple-of-four result directly. The profile
performs no other rewriting: no line wrapping, no whitespace, no alternate
alphabet, and no decoding.

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

## Boundaries

This profile adds no owned buffer, no Writer, no stream, no decoding, and no
public export. It does not widen a public nominal or generic surface, and it
grants no ambient filesystem, process, or network authority. It does not claim
line wrapping, MIME framing, or a URL-safe or unpadded alphabet, all of which
remain open. `std.encoding`'s existing digit table, contracts, and identities
are unchanged; this package only imports them.

## Focused local evidence

```sh
cargo test --locked -p semaprax --test project standard_library::base64
cargo test --locked -p semaprax --test project standard_library::every_public_declaration_has_a_std_identity_contracts_examples_and_conformance
cargo test --locked -p semaprax --test documentation
```

The conformance module checks nine vectors against Python's
`base64.b64encode`: the empty input, `"f"`, `"fo"`, `"foo"`, `"foob"`,
`"fooba"`, `"foobar"`, the non-text bytes `\x00\xff\x10`, and `"Man"`, each in
its own short function folded into a failure bitmask so no chain approaches
the `SPX-H006` replay bound. The shipped example encodes a short input and
checks a couple of digits and the padded length.

The package's examples and conformance execute on the interpreter, native C11
at `-O0` and `-O2`, and Core Wasm under Node through the standard-library gate.
Two hostile interpreter cases reject a digit index at the padded length and any
digit of the empty input with the exact `requires`-false contract status. A
graph check replays the projection, pins both accessors to a borrowed view with
an empty owned inventory, and rejects a one-byte change to the padding policy.

This is source-level Base64 encoding into a caller-supplied buffer. It is not
evidence of a decoder, a streaming codec, or any public API.
