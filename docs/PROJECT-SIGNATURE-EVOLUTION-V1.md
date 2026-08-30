# Project Signature Evolution v1

Status: authored, unrun; the graph-operational programme remains Partial.

Audience: agent builders, compiler contributors, and reviewers.

This additive intention shape extends [Project Candidates and Semantic Change
IR v1](PROJECT-CANDIDATES-V1.md). Canonical source remains authoritative; an
intention constructs candidate ASTs and cannot publish source, supply trusted
HIR, or bypass the complete Project verifier. Existing `append_parameters`
requests, canonical bytes, argument treatment, and limits remain unchanged.

## Ordered parameter mapping

`change_function_signature` now additionally admits an object with exactly
`kind`, `target`, and `parameters`. The target retains the existing requirement:
an explicitly identified, monomorphic, top-level function other than `main`.
The array order becomes the candidate declaration's parameter order.

```json
{
  "kind": "change_function_signature",
  "target": "calculator.add",
  "parameters": [
    {"from": "right"},
    {"from": "left"},
    {"name": "offset", "type": "i64", "argument": {"kind": "i64", "value": 0}}
  ]
}
```

Each element has exactly one of these shapes:

| Shape | Meaning |
| --- | --- |
| `{"from":"old_name"}` | Retain one original parameter with its exact existing name, type, and mode at this position. |
| `{"name":"new_name","type":"scalar","argument":literal}` | Add a fresh by-value scalar parameter and supply the explicit matching scalar literal at every migrated call. |

A retained parameter can appear only once. Original parameters omitted from
the array are removed from the declaration, but their caller argument
expressions are still evaluated. An empty array therefore removes every
parameter while preserving argument evaluation at existing calls.

New parameter names must be distinct from all original names, including names
of removed parameters. They cannot reinterpret an existing body binding.
New types and literal kinds are the existing `i64`, `i32`, `u8`, `usize`, and
`bool` vocabulary with exact numeric bounds. Unknown fields, combined
`parameters`/`append_parameters`, inferred defaults, expressions as new default
arguments, duplicate mappings, and unknown original names reject.

Every original parameter must have value mode and a built-in Copy type:
`i64`, `i32`, `char`, `u8`, `usize`, `[u8; N]`, `f32`, `f64`, or `bool`.
Named records/variants, owned values, borrows, and shared modes are rejected
before mutation even if a particular named type would be Copy. This layer has
no authority to infer ownership, transfer, or settlement rules from AST names.

## Evaluation and lexical hygiene

Reordering call argument expressions directly would reorder effects and
checked failures; removing one would silently skip its evaluation. This route
instead transforms every existing direct call into a block. For example:

```text
choose(left(), right())
```

with the mapping `[from right, from left]` becomes the structural equivalent
of:

```text
{
    let spx_sig_stage_0 = left();
    let spx_sig_stage_1 = right();
    choose(spx_sig_stage_1, spx_sig_stage_0)
}
```

The real implementation constructs AST nodes, moves the original argument
subtrees into the initializers, and delegates source projection to the
canonical formatter. It never rewrites text. Every original argument executes
at most once, in its original left-to-right position; the first failing
argument still prevents evaluation of later arguments. A removed argument
still executes when its original position is reached. Newly supplied literals
are pure and cannot introduce an additional effect or checked arithmetic
failure. The original call's enclosing lazy operand, branch, loop, or contract
retains the generated block in place; staging is not hoisted out of that scope.

Fresh staging names avoid a conservative inventory of names across all
candidate modules: function names, parameters, import aliases, local bindings,
assignment targets, variable/call references, and nested match binders.
Previously generated staging names are reserved as well. This prevents an
introduced `let` from capturing references in a later original argument,
including references to deliberately adversarial staging-like source names.
No body parameter references are renamed. A mapping requesting parameter
rename or type conversion is rejected instead of applying a lexical guess.

Stable-ID provider bindings determine which direct calls migrate. Existing
import aliases stay unchanged, and provider module identity is checked.
Traversal includes local calls, imported calls, generic caller bodies, class
method bodies, contracts, match guards, unsafe blocks, loops, and nested
expressions. It does not rely on the six-family cross-file graph, which omits
local calls, and it does not find external consumers.

## Admission, bounds, and diagnostics

After the mapping, ordinary candidate application canonically formats and
reparses all sources, independently rebuilds the full Project, checks the
existing identity/effect/contract/profile requirements, and preserves admitted
core target lanes. Removing a parameter still used in a body or contract fails
real verification. A caller may first submit a separately admitted body change
that removes that use; the signature change must then bind that candidate's
new revision. External API compatibility and behavioral equivalence are not
implied by preserved exported stable IDs.

The mapping permits at most 4,096 original and resulting parameters before
ordinary profile admission imposes its existing narrower export limits.
Expression traversal is bounded to depth 256 and 1,048,576 expression nodes,
including generated expression growth; lexical pattern traversal has the same
bounds. Staging-name allocation is bounded to 1,048,576 attempts. The enclosing
Semantic Change node/depth/byte limits and Project source/output limits also
remain active. These are deterministic structural bounds, not a total heap
memory limit or a performance guarantee.

`SPX-G225` rejects unsupported mappings, old non-Copy/mode subjects, unknown or
duplicate parameter selections, inconsistent provider bindings, and name/type
reinterpretation. `SPX-G226` rejects structural capacity excess. Real Project
verification and candidate stale/replay checks retain their existing
language and `SPX-G222`–`SPX-G224` diagnostics. Failed candidate construction
never mutates a previously returned candidate or live source files.

## Evidence and non-claims

The unit evidence in
[`src/project/candidate/signature.rs`](../src/project/candidate/signature.rs)
is authored but unrun at the user's request. It covers reordered Copy results,
retained first-failure selection after dropping or reordering arguments,
parameter/local name capture attempts, imported and declared-effect call
ordering, canonical source round trips, removal of a still-used parameter,
and rejection of guessed rename/type changes and owned/borrowed modes.
The pure reference-interpreter probes are authored executable evidence; no
interpreter, target, compiler check, or local test was run for this change.
Declared-effect ordering is a structural regression, not hosted effect-runtime
evidence.

Additional staging changes expression identities, local storage, generated
code, and interpreter fuel consumption. This is not exact operational-cost
equivalence, external consumer migration, a full semantic merge, general
parameter renaming, type/return conversion, or owned-call settlement support.
Those wider cases remain open in the graph-operational roadmap.
