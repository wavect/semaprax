# Project Function Extraction v1

Status: Partial; implementation and regression cases authored, not executed in this change.

Audience: compiler maintainers and agents using immutable Project candidates.

This operation moves one authenticated authored body expression into a new,
explicitly identified, monomorphic function in the same source module. Canonical
`.spx` remains authoritative. It does not accept source fragments, caller-chosen
spans, captures, types, effects, or an editable graph.

## Request and identity

The closed intention object inside the existing Semantic Change envelope is:

```json
{
  "kind": "extract_function",
  "target": "calculator.add",
  "expression_id": "<current expression catalogue identity>",
  "new_id": "calculator.add-core",
  "new_name": "add_core"
}
```

The candidate and source revisions are bound by the existing candidate API.
Expression selectors come from the [expression catalogue](PROJECT-EXPRESSION-CHANGE-V1.md).
The target must be an explicit top-level monomorphic function; `main` can be an
anchor, but the introduced helper cannot be named `main`. The helper identity
must be globally unused and its display name must not collide with the source
module's callable or declaration bindings. No public manifest export is added.

The compiler joins the selected checked HIR expression to a unique authored AST
origin using the retained source revision, digest, module, function, complete
span, and compatible expression kind. Contract regions, synthetic expressions,
ambiguous origins, and stale selectors are rejected. The structural path used
for rebuilding is compiler-derived and never a request field.

## Captures and evaluation

External captures are resolved HIR ValueIds from the actual lexical scope,
including preceding local lets and match bindings. Definitions inside the
selected subtree remain internal; matching names do not turn these definitions
into captures. Capture order follows first authored use, and each ValueId occurs
once in the helper parameter list. Every capture must be immutable, by-value,
and directly `i64`, `i32`, `u8`, `usize`, or `bool`.

Every visited expression value and internal binding must also be a directly
supported Copy scalar. Owned values, borrowed values, field projections,
propagation, and compiler ownership lowering are rejected. External mutable
captures and writes to enclosing bindings are rejected. Mutable locals and writes
wholly inside the moved subtree remain inside it and are allowed. Unsafe
statements inside the subtree, and selections nested under an unsafe statement,
are rejected so that extraction cannot relocate an audit boundary or its owner.

The original AST subtree becomes the helper body. A single direct call replaces
it at its original evaluation point; block positions retain a block wrapper.
Reading immutable Copy captures adds no observable effects or checked failures.
The moved body retains its internal evaluation order and lazy branches, and
executes once when the original position executes. This does not promise
identical call depth, interpreter fuel consumption, stack usage, generated code,
performance, or function-labelled diagnostic traces.

The helper inherits the anchor's compiler-checked effect budget, sorted and
unique. This is a conservative budget, not minimal expression-effect inference.
The helper has no new contracts; all original contract source remains unchanged.
Existing capabilities, source inventory, exports, and old declaration identities
remain governed by candidate invariants. A compiler-derived helper is the sole
permitted addition to those identity and effect inventories.

## Admission, replay, and bounds

The ordinary candidate pipeline canonically formats, reparses, independently
builds the Project, revalidates ownership and cleanup, and derives supported
native C and structurally validated Core Wasm target facts. Extraction then
independently reconstructs the exact helper and source splice from the old
revision and compares every canonical source with the admitted candidate. It
also authenticates the replacement expression at the compiler-derived AST path
and requires the original expected type and ownership.

Rebase rejects competing anchor body or signature changes and new-identity
collisions. It remaps the expression selector through the authenticated
structural path at the corresponding intermediate history step. Remapping alone
does not grant merge or publication authority.

Expression traversal is bounded to 4,096 nodes and depth 256; binding-pattern
traversal has the same bounds. At most 64 captures are accepted. Existing
candidate/source byte bounds, declaration limits, and Project admission bounds
also apply. Unsupported requests use `SPX-G225`; operation capacity failures use
`SPX-G226`. Existing candidate stale/replay diagnostics remain unchanged.

These operations retain immutable in-memory candidates. They do not write
source, cache an image, publish a workspace generation, or commit Git changes.

## Evidence and remaining scope

Authored, unrun cases in `tests/project_candidate_extraction_v1.rs` cover repeated
capture deduplication, internal let/match binders, lazy checked-failure placement,
mutable capture and contract rejection, identity/name collisions, exact replay,
stale changes, rebase after unrelated source movement, and unchanged disk bytes.
No local tests, compiler checks, or long quality gates were run, at the user's
request; these cases are not passing completion evidence.

Owned or borrowed extraction, mutable capture copy-back, aggregate captures,
generic functions, contract extraction, propagation across function boundaries,
unsafe audit relocation, minimal effect inference, arbitrary extraction regions,
and runtime resource equivalence remain outside this version. The broader
graph-operational roadmap remains partial.
