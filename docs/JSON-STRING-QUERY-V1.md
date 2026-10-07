# JSON String Query v1

Status: additive source package for the existing bounded JSON string decoder.

Audience: language users and standard-library contributors.

`std.data.json.query` projects JSON string escape decoding and decoded-token
comparison onto signatures accepted by Project v25
`language-command-io.stream-text.v1`. It uses borrowed `Slice<u8>` and Copy
scalars. It declares no
`std.io.Reader`/`Writer` types, custom interfaces, effects, or host operations.

## Package and API

The package requires `useful-data.v1` and depends on the exact bundled
`std.data.json = "=0.1.0"` scanner. Its source functions have stable IDs under
`std.data.json.query`:

```semaprax
fn decoded_len(input: borrow Slice<u8>, start: usize) -> usize
fn scalar_at(input: borrow Slice<u8>, start: usize) -> i64
fn emit_len(input: borrow Slice<u8>, index: usize) -> usize
fn emit_at(input: borrow Slice<u8>, index: usize, offset: usize) -> i64
fn decoded_token_eq(input: borrow Slice<u8>, left: usize, right: usize) -> bool
```

`decoded_len` accepts the start offset of one quoted JSON string token and
returns its decoded UTF-8 byte length. It uses the shared scanner result
encoding: a result at most `byte_len(input)` is a length; a result greater
than the input length encodes rejection and the first offending byte.
Invalid escapes, raw controls, truncated strings, and unmatched or incorrectly
paired surrogates reject. `decoded_token_eq` compares two quoted tokens by
decoded bytes; malformed tokens compare unequal. `scalar_at`, `emit_len`, and
`emit_at` provide the pull surface for callers decoding without a buffer.

Escape classification, code-unit validation, paired-surrogate handling,
failure offsets, and UTF-8 emission follow `std.data.json.dec`'s existing pure
algorithm. The projection has a separate module and namespace: `.dec` keeps its
owned-data-api profile and Reader/Writer cursor adapters unchanged. The
projection preserves the existing raw-byte policy: escaped Unicode scalars
are encoded as UTF-8, while admitted non-control raw bytes pass through
without separate UTF-8 validation. Use `std.data.json.utf8` when raw UTF-8
validation is required.

JSON has no `\0` escape. `\u0000` is the valid JSON spelling for a NUL scalar;
invalid `\0` rejects.

## Scope and gates

This package provides string-token decoding and comparison, not a JSON value
tree or a complete application object-query API. `std.data.json.doc` remains
the bounded structural walker and raw-span key navigator. An application can
combine it with `decoded_token_eq` to compare escaped member names by JSON
meaning.

`std.data.json.query` is source-only and portable across the interpreter,
native C11, and Core Wasm targets declared in `std/packages.json`. Its examples
and conformance module cover escaped/literal key equality, `\uXXXX`, surrogate
pairs, malformed escapes, raw controls, and truncated tokens. Project v25's
stream-text fixture imports both the structural walker and this query package;
it retains the existing four command
capabilities and 4096-byte stdin reader contract.
