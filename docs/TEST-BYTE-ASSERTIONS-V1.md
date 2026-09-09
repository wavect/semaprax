# Test Byte Assertions v1

Status: additive private `std.test.bytes` slice with focused local execution evidence;
the complete Everyday testing scope remains open.

Audience: standard-library authors, compiler contributors, and agents writing
bounded conformance checks.

This specification adds exact byte comparisons in the same helper family as
`std.test`, supplied by the sibling `std.test.bytes` package at
`std/test-bytes`. It does not change the existing `std.test` facade or its
public descriptor, report schemas, or ambient authority model. The existing
scalar predicates and failure-bit helpers remain unchanged.

## Functions

The source-authored `std.test.bytes` helper family owns these stable IDs:

```semaprax
@id("std.test.bytes.equal")
fn equal_bytes(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool

@id("std.test.bytes.equal-remaining")
fn equal_remaining(left: borrow Reader, right: borrow Reader) -> bool

@id("std.test.bytes.failure-bit-equal")
fn failure_bit_equal_bytes(
    left: borrow Slice<u8>, right: borrow Slice<u8>, failure_bit: i64
) -> i64

@id("std.test.bytes.failure-bit-equal-remaining")
fn failure_bit_equal_remaining(
    left: borrow Reader, right: borrow Reader, failure_bit: i64
) -> i64
```

`equal_bytes` returns true exactly when both slices have equal length and every
byte at the same index is equal. `equal_remaining` compares the suffix from
each Reader's current position through the end of its backing Bytes. Both
Reader positions are borrowed observations and remain unchanged. Each Reader
requires a position no greater than its backing byte length; an invalid cursor
fails through the ordinary contract path before comparison.

Each `failure_bit_equal_*` helper requires `failure_bit > 0`, returns `0` when
the corresponding equality predicate succeeds, and returns exactly
`failure_bit` when it fails. The failure-bit result therefore composes with
the existing `failure_bit_unless` test status convention.

## Bounds and authority

The comparisons scan only the caller-supplied slice or Reader suffix and use
the actual backing lengths already checked by the language. They introduce no
new arbitrary byte cap, allocation site, mutation, ambient effect, or process,
filesystem, network, or environment authority. Both inputs remain borrowed;
the helpers do not create an owned buffer or public nominal descriptor.

## Admission and verification

The functions are internal `std.test.bytes` composition and may be used by
checked test modules. This sibling uses the private `useful-data.v2` profile,
has no exports, and depends exactly on `std.io` and `std.test`. The existing
scalar `std.test` facade and public descriptor remain unchanged; its old
scalar export profiles cannot admit the Reader-shaped helpers. The focused gate
`standard_library::testing::test_bytes_package_and_bundled_consumer_execute_across_engines`
passes source-package and bundled-consumer examples/conformance on the interpreter,
native C11 `-O0`/`-O2`, and repeated Core Wasm with settled byte owners. It covers
binary and empty slices, length and byte mismatches, exhausted Readers, different
positions with equal suffixes, self-comparison, preserved positions, and exact
positive failure-bit results through `i64::MAX`.
`standard_library::testing::test_bytes_invalid_cursors_and_failure_bits_select_contract_failure`
passes five invalid-cursor/bit cases across those engines, including repeated
Wasm settlement. The existing bundled scalar `std.test`/`std.time` consumer and
unchanged scalar source preserve the old facade. These are local observations,
not hosted promotion or a completed fixtures/property/fuzz/snapshot facility.

The source ownership is the sibling `std/test-bytes` package; the existing
`std.io.Reader` identity supplies the Reader shape. This expands the source
helper family without introducing a new report model. The package manifest and generated catalog expose this private dependency without
widening the scalar package's public descriptor.
