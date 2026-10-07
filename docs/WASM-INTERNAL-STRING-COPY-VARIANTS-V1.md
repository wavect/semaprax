# Standalone Wasm Copy Variant String Settlement v1

Audience: compiler contributors and runtime implementers.

Status: Partial — the narrow profile is implemented and verified by the
focused local executable gate below.
This is the explicit additive Wasm lane for [Copy Variant Guards v1](COPY-VARIANT-GUARDS-V1.md).
It does not complete general aggregate or ownership-changing guards.

## Selection and compatibility

`wasm::internal_strings::emit_copy_variant_module(&Program, &[String], InternalStringOptions)`
selects this profile explicitly. The older `emit_module` remains the closed,
nominal-free [Internal String Settlement v1](WASM-INTERNAL-STRINGS-V1.md)
entry and retains `SPX-W111` for nominal variant expressions. The existing
CLI Web package, Project routes, Target Evidence and ordinary Wasm do not
select this additive entry implicitly.

The descriptor retains `semaprax.wasm-internal-strings.v1` and adds the exact
`profile: "copy-variants-v1"` marker. The existing v1 descriptor receives no
new field. The compiler-generated trusted runtime embeds the exact descriptor
and authenticates the exact module SHA-256/length. The new profile uses the
same ten String imports, private fixed memory, scalar facade, sticky status
normalization and poisoned-on-uncertainty behavior. It adds no host authority,
raw view argument, public nominal value, import or lifecycle.

## Closed additive admission

Exports still require explicit identities, at most eight value `i64`/`bool`
parameters, and one `i64`/`bool` result. The selected effect-free, monomorphic,
acyclic closure retains the v1 function, expression-depth, node, literal-pool,
stack, cleanup-emission, module-size and String quota limits. Public arguments
and results cannot carry String owners, variants, arrays or views.

Inside selected bodies, the profile additionally admits:

- Copy variants whose substituted payload fields are direct Copy scalars;
  construction, exact case or payload-free case-or matching, unguarded
  fallbacks and the authenticated scalar-operator guards of Copy Variant
  Guards v1. The internal value vocabulary is `i64`, `bool`, `char`, `u8`,
  `usize` and owned String; other field expressions remain `SPX-W111`.
- Fixed `u8` arrays, exact named `array_as_slice` views, `byte_len` and
  `byte_get`. A view is non-consuming and retains its authenticated owner
  provenance. Arrays/views remain internal locals, not function signatures.
  Byte lookup returns the ordinary `Option<u8>`; out-of-range lookup returns
  `None`, including empty-array and one-past-end indices.

Owned/non-Copy variant payloads, records, dynamic owned Bytes, other byte/view
operations, nominal internal signatures, user generic function instances,
foreign calls, effects and unsafe expressions remain outside this profile.
HIR validation and independent cleanup replay precede admission. Malformed
source guards retain their source diagnostics; malformed HIR remains
`SPX-H006`, and unsupported selected profile shapes remain `SPX-W111`.

The explicit [General Loop Match v1](GENERAL-LOOP-MATCH-V1.md) selector adds
ordinary guard trees, private Copy variant helper signatures and Copy variant
match results. This frozen selector retains `SPX-W111` for those extended
selected shapes; prior admitted artifacts retain exact bytes.

## Lowering and settlement

Variant layouts are derived and independently validated from the selected
closure only. Valid unselected declarations cannot add layouts or change the
artifact. Each scrutinee executes once; case tests and guards preserve authored
order and lazy evaluation. Payload bindings precede a reached guard. False
guards fall through, terminal guard failure precedes any arm value, and
unguarded exhaustiveness remains required.

Fixed-array views carry only a compiler-derived frame pointer and length.
Before Len/Get reads, generated code checks the complete span against the
65536-byte private shadow-stack region, rejecting tagged arena/range carriers.
Get checks the full unsigned index before narrowing for a memory load; it
writes canonical Option tags and payload layout. String handles and byte-view
carriers never share host imports.

Selected String arm results retain canonical transfers and exact scope exits.
Normal block completion settles scalar operand temporaries after owned result
handoff, including empty and nested branch blocks. Canonical finalizer vectors
remain in their authored order. The runtime must observe an empty owner arena
and restored stack after every admitted success or recoverable failure; traps
retain the existing fail-stop behavior.

## Focused executable gate

`cargo test --locked -p semaprax --test language guarded_copy_variants::`
compares the six original guard programs and the index-boundary control on the interpreter, native C11 at O0/O2
with allocation/free accounting, and this explicit String-settling Wasm entry
with repeated calls. It covers false/true fallthrough, wrong-case skipped
failure, lazy operands, Result payloads, fixed-array sentinel lookup including
one-past-end None, empty-array Len/Get, indices above u32 and signed i64,
maximum usize, String result reuse, and checked guard failure. The additive
[loop construction gate](LOOP-COPY-VARIANT-CONSTRUCTION-V1.md) also covers
Copy constructors inside loops, operand temporaries, nested String results and
selected failures; Vec traversal keeps its exact `SPX-W111` profile refusal.

The same harness asserts the old entry's exact `SPX-W111` refusal, the new
profile marker, request-order determinism, unselected owned-declaration
isolation, unsupported owned payload refusal, and malformed guard refusal.
Existing hostile HIR guards/cleanup edges remain `SPX-H006`.
`native_if_string_temporaries::` separately checks Len-based empty/nested
branch settlement on the original v1 entry.
