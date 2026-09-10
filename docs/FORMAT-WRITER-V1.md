# Format Writer v1

Status: implemented additive source profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

Audience: standard-library contributors, compiler maintainers, and backend
implementers.

`std.format` provides allocation-free, type-directed rendering into the
caller-owned `std.io.Writer`. It has no runtime format-string parser and does
not add a public export or a nominal ABI.

## API surface

The package exposes these source functions through an exact bundled dependency:

```text
std.format.append_str(value: borrow str, output: own std.io.Writer)
    -> std.io.Writer
std.format.append_i64(value: i64, output: own std.io.Writer)
    -> std.io.Writer
std.format.append_usize(value: usize, output: own std.io.Writer)
    -> std.io.Writer
std.format.append_bool(value: bool, output: own std.io.Writer)
    -> std.io.Writer
```

Each function consumes and returns the Writer, appending its exact byte
representation at the current position. `append_str` copies the borrowed
string's UTF-8 bytes. Integer rendering is decimal ASCII without leading
zeroes; signed `i64` values include `-` when negative, including the minimum
value. Boolean rendering is `true` or `false`.

The helper functions `byte`, `digit_byte`, `usize_len`, `usize_byte`,
`i64_len`, and `i64_byte` provide the checked length and indexed-byte policy
used by the append operations. Their contracts reject out-of-range byte or
digit requests and out-of-range indexes before a write.

## Writer contract and evaluation

Every append operation preflights the Writer position and exact remaining
capacity before its first `bytes_set`. A rejected call therefore leaves no
partial output or published replacement Writer. A successful call advances the
cursor by exactly the emitted byte count and preserves the caller's unwritten
suffix. Arguments and helper calls evaluate left to right, and ownership is
transferred at the ordinary consuming-call boundary.

The implementation uses the existing `std.io` Writer and byte operations. It
does not allocate, grow a buffer, perform effects, or acquire filesystem,
process, network, home, secret, key, wallet, or signing authority. The package
uses the private `useful-data.v2` profile and the bundled `std.io` dependency;
its export list is empty. Internal borrowed `str` and ordinary owned byte
record signatures are admitted by the compiler, while public export rules and
the public ABI remain unchanged.

## Scope and verification

This slice covers strings, `i64`, `usize`, and `bool` appended into a supplied
Writer. It does not supply general format strings, floating-point rendering,
allocation, or production support.

The historical local verification passed seven owned-function-import unit
tests, including positive borrowed-`str` and ordinary owned-byte-record imports
and refusal of a non-byte record. Eight individually runnable named SPX tests
passed on the interpreter, native C11 at `-O0` and `-O2`, and repeated Core Wasm
with a strict two-entry byte arena; the pure helper case uses zero allocation.
Five short-output and forged-output preflight cases passed twice with exact
`requires`-false status through the bundled `std.format` consumer and its
transitive `std.io` dependency. Metadata and catalog regeneration also passed.

The named tests run individually because the per-function static allocation
limit is unchanged. The implemented release corpus has hosted-green evidence;
the historical case counts retain their original local scope. General format
strings, floating-point rendering and production support remain outside this
bounded profile.
