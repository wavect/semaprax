# Collection law pack v1

This document owns `semaprax.collection-sort-i64.v1`, its separate authored
proof module `semaprax.collection-sort-proof-module.v1`, and its inert
`semaprax.collection-sort-certificate.v1` report. The saved first-user pack is
[`examples/law-packs/collection`](../examples/law-packs/collection/README.md).
The route is checked source plus the existing installed pinned Lean provider;
it does not add a package registry or a publication route.

## Source and translation

The selected declarations are `law15.collection.insert` named `insert`, with
value parameters `value: i64, input: List<i64>`, and `law15.collection.sort`
named `sort`, with value parameter `input: List<i64>`. Both return the exact
compiler-owned immutable `List<i64>`, resolved and independently HIR-validated.
They have no type parameters, effects, requirements, postconditions, yields or
session protocol. The shared source verifier admits one scalar `i64` value
parameter alongside the required List input in this two-parameter match-result
shape. It continues to reject a boolean parameter and other widened shapes. External contracts do not silently become assumptions.

`src/proof_export/list_sort/source.rs` translates every body expression in a
closed grammar: empty-statement blocks, the exact parameter/case variables,
`<=`, `if`, `list_nil`, `list_cons`, direct list matches, and calls to the two
selected functions. A match destructures the original `input` via
`list_uncons`, with exactly `Nil {}` and `Cons { head, tail }`, no guards.
Self-recursion must pass the actual destructured `tail`; insertion cannot call
sort, preventing mutual recursion. Source module imports and shadow definitions
of list operations refuse. Expression work/depth limits are 128/24 per body.
Unsupported signatures, expressions and recursion return `SPX-LI015`.

The emitter preserves the authored branches and calls, including well-typed
bad bodies. It does not substitute a correct sorting algorithm after recognizing
a function name. List operations map to Lean `List Int` constructors/matches;
comparison maps to exact integer comparison. There is no element arithmetic,
unchecked numeric conversion or unproved overflow obligation in this grammar.
The pinned kernel checks structurally terminating definitions before the laws.

## Law inventory and authored proofs

The compiler fixes five theorem statements:

| Name | Statement |
| --- | --- |
| `insert_permutation` | Insertion permutes `value :: input`. |
| `insert_sorted` | Insertion preserves nondecreasing pairwise order. |
| `sort_sorted` | Sorted output is pairwise nondecreasing. |
| `sort_permutation` | Sorted output permutes the complete input. |
| `sort_multiplicity` | For every value, output and input occurrence counts agree. |

The proof module supplies only tactic bodies under these fixed statements.
Its semantics field must equal `semaprax.checked-i64.immutable-list.v1`.
Unknown JSON fields refuse. Each proof is bounded to 4096 bytes/64 lines and
cannot add declarations, imports, axioms, holes, code execution, elaborators,
macros or kernel options. The existing pinned kernel report parser requires all
five named axiom reports and rejects nonstandard axioms. The installed provider
must hold the explicitly supplied binary and exact pinned version; a caller's
fabricated `LeanKernel` implementation is not evidence of a physical run.

The mathematical functions are total on finite lists of exact integers. Their
unbounded structural laws apply to the denotation of representable `i64`
elements. This is distinct from physical totality: capacity, allocation and
stack/depth limits can refuse at runtime. No theorem proves backend lowering,
public ABI, resource sufficiency, or external effects.

## Reports and replay

A certificate binds canonical source, generated definitions, full proof module,
exact generated Lean document, profile/schema versions, declaration coverage and
kernel-reported axioms. Only the two selected declarations are covered; other
declarations are explicitly unsupported. Current source and separately held
proof-module drift refuse before a kernel invocation. Successful replay invokes
the pinned kernel again and compares the complete report. No parsed report
creates a Project `VerifiedLawProof` or grants publication authority.

A separate `refute_multiplicity_on_pair` route checks the fixed concrete input
`[1, 2]`. It proves that the source output remains sorted but has a different
count of 2. Its report is explicitly `bounded_source_counterexample`, bound to
the source and generated definitions. This demonstrates a false multiplicity
law, independently of an authored tactic's failure. It is not positive universal
proof or a runtime execution claim.

## Owning verification

The `language` harness module `collection_law_pack` binds the saved source,
mutants, repair and proof module. Its source test exercises check, canonical
roundtrip, HIR, graph replay, unsupported recursion and custom-axiom refusal.
Its explicitly provisioned Lean test proves the correct body, refuses empty and
duplicate-element bodies, checks their concrete multiplicity witnesses, replays
unchanged-law repair and rejects altered source, proofs, profile and statements.
The duplicate mutant preserves length on `[1, 2]`, so a length-only claim cannot
substitute for exact multiplicity.
