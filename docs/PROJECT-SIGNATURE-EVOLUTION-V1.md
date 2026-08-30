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
| `{"from":"old_name","name":"new_name"}` | Retain its exact type and mode and rename the original lexical parameter binding. |
| `{"name":"new_name","type":"scalar","argument":literal}` | Add a fresh by-value scalar parameter and supply the explicit matching scalar literal at every migrated call. |
| `{"name":"new_name","type":type_selector,"argument_expression":expression}` | Compute a new scalar or checked Copy nominal argument from the original staged parameters after all original arguments, then fully revalidate each migrated caller. See [Argument Expressions v1](PROJECT-SIGNATURE-ARGUMENT-EXPRESSIONS-V1.md). |

A retained parameter can appear only once. Original parameters omitted from
the array are removed from the declaration, but their caller argument
expressions are still evaluated. An empty array therefore removes every
Copy parameter while preserving argument evaluation at existing calls. Every
owning parameter must be retained exactly once; it cannot be removed or copied.

New parameter names must be distinct from all original names, including names
of removed parameters. They cannot reinterpret an existing body binding.
New scalar types and literal kinds are the existing `i64`, `i32`, `u8`, `usize`, and
`bool` vocabulary with exact numeric bounds. Unknown fields, combined
`parameters`/`append_parameters`, inferred defaults, nonliteral `argument`
values, duplicate mappings, and unknown original names reject. Computed values
require the separate explicit `argument_expression` form above.
Only that computed form additionally admits stable-ID nominal type objects;
provider and caller bindings are resolved independently and rebuilt checked
signature facts must prove exact-identity Sized Copy admission.

Every original parameter must have value mode and a built-in Copy type
(`i64`, `i32`, `char`, `u8`, `usize`, `[u8; N]`, `f32`, `f64`, or `bool`),
have value mode and an admitted concrete Copy record/variant type, or be
exactly `own Bytes`. Named admission uses the retained checked HIR parameter
identity and compiler TypeFacts: `copy` and `sized` must be true, while
`contains_resource` and `needs_drop` must be false. Display names and source
field shapes do not establish those properties. The compiler retains exact
nominal parameter and return facts for admitted modules even when a function is absent
from the entry/test closure. Concrete generic instances, including admitted
compiler-owned variants, use their complete ordered type-argument identity;
generic target functions remain excluded.

The mapping keeps the original source type spelling, type arguments and mode.
Retained mappings do not convert a record to another record,
alter fields, widen a target profile or add an owning parameter. Named non-Copy
types, classes/resources, borrows and shared modes remain excluded. Full
candidate admission still rejects a removed parameter referenced by the body
or contracts, and any caller that cannot be legally staged.

For eligible named parameters, `change/catalog` adds `type_identity` and
`type_provenance`, including the nominal declaration identity, ordered argument
identities and exact checked Copy/storage facts. Both application and discovery
use the same retained-HIR eligibility routine. Existing scalar parameter
descriptors and the intention's `{from[,name]}` shape remain unchanged.

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
Display renames are simultaneous: swapping two parameter names is supported.
The substitution follows the original admitted AST's lexical scopes across
requires/ensures, sequential let initializers, assignments, blocks, loop bodies,
match binders and guards, and nested record patterns. A conflicting local or
pattern binder receives a fresh `spx_sig_bind_N` name, and its references follow
that binding. Field labels, call names, types, and persistent declaration IDs
are not renamed. The postcondition `result` binding cannot become a parameter
rename destination. A reference to a removed parameter cannot silently become
a reference to a retained parameter renamed to that spelling. Unknown future
binding-bearing or-pattern forms reject until their binding rules are supported.

For direct `own Bytes`, each original expression moves into one owning local,
in original argument order. The final ordinary call receives every retained
owner once in mapped order; the real verifier and cleanup-plan builder own
transfer and atomic CallCommit semantics. This changes the placement of moves:
a later original argument borrowing an owner already moved into an earlier
staging local may fail ordinary verification. Such candidates reject; this
operation does not bypass loans or promise admission of every previously valid
call shape. Other owning types remain unsupported because observable resource
finalizers and general settlement order need additional treatment. No custom
cleanup, physical finalization authority, or hidden settlement-model action is
introduced here.

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

Retained nominal facts additionally allow at most 4,096 distinct
concrete parameter/return and checked body-value type identities per module,
under the existing builder-byte budget. Extraction shares this inventory with
signature admission. This counts concrete instances, not only source declarations. Removing
that extra cap was rejected by automatic security review as weakening a resource
boundary; it remains enforced and can reject a larger otherwise admitted type
inventory. This limit is not evidence of general unbounded signature support.

`SPX-G225` rejects unsupported mappings, unsupported type/mode subjects, unknown or
duplicate parameter selections, inconsistent provider bindings, and name/type
reinterpretation. `SPX-G226` rejects existing structural capacity excess.
`SPX-G259` rejects unsafe binding substitutions, including contract-result
capture and a still-referenced removed parameter during renaming. `SPX-G260`
rejects omitted owners. `SPX-G261` bounds the additional substitution traversal
and fresh-name allocation with the same depth/node ceilings. Real Project
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
and rejection of type changes, omitted owners, and unsupported borrowed modes.
Additional authored regressions cover simultaneous display renames, contract
references, local mutation, match guard capture avoidance, and removed-binding
capture. [`tests/project_candidate_signature_ownership_v1.rs`](../tests/project_candidate_signature_ownership_v1.rs)
authors full Project candidate/replay checks for reordered and renamed owned
byte arguments, exact original evaluation order, duplicate/removal rejection,
and unchanged live source files. These tests have not been executed.
The pure reference-interpreter probes are authored executable evidence; no
interpreter, target, compiler check, or local test was run for this change.
Declared-effect ordering is a structural regression, not hosted effect-runtime
evidence.

Additional staging changes expression identities, local storage, generated
code, and interpreter fuel consumption. This is not exact operational-cost
equivalence, external consumer migration, a full semantic merge, type/return
conversion, arbitrary ownership-sensitive migration, or physical owned-call
settlement support. Direct byte-owner staging can also change cleanup storage
and internal trace labels; it does not claim identical runtime traces or costs.
Those wider cases remain open in the graph-operational roadmap.

`tests/project_signature_named_copy_v1.rs` and
`tests/project_signature_catalog_v1.rs` author named aggregate staging,
retention/removal, alias/identity, catalogue and independent candidate replay
evidence. Rebase signature fingerprints additionally bind retained nominal
type identities: unchanged source spelling cannot conceal a different record
or variant identity on a concurrent base. The regression in
`tests/project_candidate_rebase_v1.rs` authors that conflict and unchanged-source
failure behavior. These cases are unrun; neither runtime equivalence nor the
full signature-evolution objective is promoted.
