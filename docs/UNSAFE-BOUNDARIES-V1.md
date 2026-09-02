# Unsafe Boundary Mechanics v1

Audience: language users, tool authors, and compiler contributors.

Status: Partial — the Restricted `unsafe` and raw memory row of
[COMPLETION-MATRIX.md](COMPLETION-MATRIX.md) moves from Missing to Partial on
the strength of this document plus `tests/projections/unsafe_boundaries.rs`.

## Objective

This tranche adds the smallest end-to-end language slice proving ONLY unsafe
boundary mechanics: an explicit, audited, graph-visible boundary around
ordinary safe code. It is a boundary-mechanics slice, not a raw-memory slice:

- No raw pointers, address-of operations, dereferences, volatile access,
  atomics, or memory-layout operations are added. None exist in the language
  yet; none arrive with this tranche.
- The block body is ordinary safe SEMAPRAX: `let` statements, assignment
  statements over mutable locals, and one final value expression. Everything
  inside is checked by exactly the same verifier rules as outside.

The slice proves the three mechanics every future unsafe feature must reuse:
an explicit module-level capability declaration, a mandatory verbatim audit
summary per block, and an explicit additive graph node per boundary.

## Syntax and canonical form

```text
permit { unsafe }                       // module capability declaration (mirrors effect permits)

@audit("short review summary")          // mandatory audit attribute (mirrors @id syntax)
unsafe {
    total = total + 41;
    0                                    // ordinary block body incl. required final value
}
```

- The module-level declaration reuses the existing `permit { .. }` mechanism
  exactly as declared today for effects: same position after the module and
  use declarations, same set syntax, same graph serialization of the permit
  list. `unsafe` is a reserved capability name inside that set.
- The audit annotation follows the existing attribute syntax pattern
  (`@id("...")`); it must appear immediately before the `unsafe` keyword, its
  summary must be a non-empty string literal, and it is recorded verbatim.
- The body parses as an ordinary block: zero or more `let`/assignment/unsafe
  statements plus the one required final value expression. Bindings declared
  inside do not escape; mutation of outer `let mut` locals is visible after
  the block. Unsafe blocks may nest; each carries its own audit summary.

The canonical formatter renders the multi-line form shown above inside
function bodies and the inline form `@audit("s") unsafe { stmts; tail }`
inside expression-position blocks; round-trips are byte-stable.

## Semantics (boundary only)

Backends treat the statement transparently: the native C11 lane emits exactly
the body's code (its scalar Copy result is discarded via `(void)`), and the
Wasm core lane emits exactly the body's instructions followed by `drop`. No
new evaluation order exists — the body's statements execute left-to-right and
checked failure statuses propagate unchanged. To guarantee that discarding
introduces no ownership or cleanup semantics, v1 requires the body's result
to be a checked Copy scalar value (`SPX-N104`). CleanupPlan v2 output for a
straight-line boundary is structurally identical to the equivalent plain
block-expression form (no slots, no finalizers).

## Capability gating

An unsafe block requires the enclosing module to declare `permit { unsafe }`.
The check mirrors function-effect checking exactly and runs in both
verification paths (source verification and HIR validation). Without the
declaration the program fails at compile time with `SPX-N101`; adding the
mirrored declaration clears it. Nothing else about the permit vocabulary
changes: programs that never write `unsafe` keep their exact previous bytes,
diagnostics, graphs, and cleanup plans.

## Diagnostics (family SPX-N1xx)

| Code | Meaning |
| --- | --- |
| `SPX-N101` | Module contains an unsafe block without declaring `permit { unsafe }`. |
| `SPX-N102` | Unsafe block lacks the mandatory `@audit("...")` summary annotation. |
| `SPX-N103` | `@audit` summary is not a non-empty string literal. |
| `SPX-N104` | Unsafe block body produces a non-scalar or non-Copy value. |
| `SPX-N105` | Unsafe boundary statement inside a contract expression (`requires`/`ensures` stay pure). |

Unknown attributes in boundary position keep the established unknown-
attribute diagnostic (`SPX-P102`).

## Graph serialization

Each boundary statement serializes additively as one explicit node:

```json
{"kind":"unsafe","audit":"<verbatim summary>","body":{...block...}}
```

Schema selection is untouched: boundary-only programs stay at
`semaprax.graph.v10`, the compact agent-context projection carries the same
node shape, and programs without boundary syntax serialize byte-for-byte
identically to pre-feature output. The pinned knowledge-AT digest from
Explicit Mutation v1 (`sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7`)
still holds, proving zero KAT drift.

## Layer behavior

- **AST**: `Statement::Unsafe { audit, audit_span, body, span }`; the body is
  the ordinary parsed block expression, so generic walkers see through the
  wrapper via `Statement::value()`.
- **Parser**: two-token recognition of `unsafe {` plus any statement-position
  attribute; precise `SPX-N102`/`SPX-N103` diagnostics.
- **Formatter**: canonical multi-line/inline rendering with byte budgets.
- **HIR**: `ResolvedStatement::Unsafe` in both the iterative resolver (one
  continuation frame, frame-size budget unchanged) and the recursive oracle
  path; validation mirrors the admission checks and rejects boundaries in
  contract contexts.
- **Graph**: additive node in both full and compact serializers plus the
  workspace expected-projection cost model.
- **Backends**: native C11 and Wasm core lower the body transparently; the
  aggregate lanes and scalar-export profile treat boundaries as pass-through.
- **Cleanup**: builder, independent replay skeleton, and inventory treat the
  boundary as its ordinary block body; no new structure.

## Evidence

`tests/projections/unsafe_boundaries.rs` covers: canonical round-trip and revision
stability under `permit { unsafe }`; regressions for `SPX-N101` (including
the positive control where adding the declaration clears it), `SPX-N102`,
`SPX-N103`, `SPX-N104`, and `SPX-N105`; deterministic Graph JSON with exactly
one `"kind":"unsafe"` node per boundary carrying the verbatim audit string
and unchanged schema selection; the pinned pre-feature digest for
non-boundary programs (zero drift); CleanupPlan v2 structural equality
against the plain block-equivalent form; and end-to-end execution of an
audited mutation-through-boundary program (native C11 O0/O2 probes plus
4,096-call Node/Wasm re-entry), including checked overflow inside a boundary
surfacing its failure status natively and trapping under Wasm instead of
wrapping.

## Nonclaims

This tranche claims ONLY boundary mechanics. It does **not** claim:

- Raw pointers or any memory operations — none exist in the language, and
  none were added; the block body cannot express them.
- Any safety property of block contents: the audit summary is recorded
  verbatim and uninterpreted, no review, lint, approval, or correctness
  claim attaches to it, and the compiler verifies block contents exactly
  like safe code (which is why nothing unchecked can occur).
- Lint coverage, platform conformance, undefined-behavior semantics, FFI, or
  `no_std`/embedded claims.
- New capability vocabulary beyond the single reserved `unsafe` name in the
  existing module permit set; no enforcement machinery beyond this
  compile-time gate exists.
