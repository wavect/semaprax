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
once in the helper parameter list. Every capture must be immutable and by-value.
Types may be direct `i64`, `i32`, `u8`, `usize`, or `bool`, or an authenticated
record/variant whose exact checked TypeFacts establish Sized Copy with no drop
or resource content. Nominal captures and results retain the exact stable owner
and ordered type arguments; same-shaped declarations are not interchangeable.

Ordinary expression extraction requires every visited value and internal
binding to be Copy. The nested-block lane below also admits internal owned
data without changing capture or result rules. Nominal helper types resolve through the selected source module's
existing binding, including monomorphic import aliases. Local generic and fixed
compiler-prelude instances support direct `i64`/`bool` arguments; nested type
arguments, new imports and generic target functions remain excluded. Body-only
nominal instances need not already occur in a function signature: their facts
are retained from checked HIR values and bindings, not inferred from AST shape.

A field read captures its entire immutable root by its authenticated ValueId,
using the root's actual type and name. Multiple projections of one root create
one parameter; the original field expressions stay inside the moved subtree.
This preserves first-use capture order without converting source field labels
into new parameters. Internal nominal locals and pattern bindings remain in the
helper body and are checked by the same Copy rules.

Owned captures and results, borrowed values, propagation, and compiler ownership lowering
are rejected. External mutable
captures and writes to enclosing bindings are rejected. Mutable locals and whole-binding
writes wholly inside the moved subtree remain inside it and are allowed; field
assignments remain excluded, even for internal roots. Unsafe
statements inside the subtree, and selections nested under an unsafe statement,
are rejected so that extraction cannot relocate an audit boundary or its owner.

The original AST subtree becomes the helper body. A single direct call replaces
it at its original evaluation point; block positions retain a block wrapper.
Reading immutable Copy captures adds no observable effects or checked failures.
The moved body retains its internal evaluation order and lazy branches, and
executes once when the original position executes. This does not promise
identical call depth, interpreter fuel consumption, stack usage, generated code,
performance, or function-labelled diagnostic traces.

## Nested blocks with internal owned data

A selected non-root authored block may contain owned String, Bytes and checked
monomorphic resource-free record/variant values created and settled entirely
inside that block. Its result and all external captures remain immutable Copy
values under the rules above. Nominal admission uses retained Sized, Copy,
drop and resource facts; source spelling or the presence of a constructor does
not prove eligibility. Ordinary source and target profiles still apply.

The compiler places the exact original block inside a fresh, empty helper-root
block. This preserves the original nested lexical cleanup boundary. It does
not flatten the selected block into a function root: root locals survive
through function postconditions, whereas nested locals settle before their
enclosing expression resumes. Selecting the original function root with owned
data is rejected even when its postconditions happen to be empty. Existing
Copy-only extraction retains its original source projection.

No owner or loan crosses the new call boundary. Internal owning lets must be
immutable. Owned assignments, owning pattern bindings, borrowed values and
views, resource-containing data, native-import/host-command call forms,
propagation and unsafe-boundary relocation remain excluded. Internal ordinary
calls, construction, cloning and consumption retain their authored order;
ordinary source admission still checks each transfer. On success, the nested
region settles before the caller resumes. On language-level failure it settles
before the unchanged failure reaches the caller's enclosing cleanup. The helper
has no contracts that could insert work between these stages.

Independent candidate replay checks the source wrapper and the rebuilt helper's
Copy signature. A separate HIR correspondence check binds the moved expression
types, ownership, stable call/member identities and lexical values to the
original subtree and Copy captures. The helper root must introduce no owned
parameter, result or root storage. Canonical cleanup plans are rebuilt by their
existing owner, never filtered, sorted, remapped or repaired for execution.
This is a constrained lexical transformation, not a general equivalence check
for arbitrary ownership plans or a physical runtime trace claim.

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
nominal type selectors admit at most 4,095 direct scalar arguments. Extraction
separately bounds distinct checked nominal type nodes and generated annotation
type nodes to 4,096 each, charging each owner and direct argument before cloning
or constructing its annotation. Existing
candidate/source byte bounds, declaration limits, and Project admission bounds
also apply. Unsupported requests use `SPX-G225`; operation capacity failures use
`SPX-G226`. Existing candidate stale/replay diagnostics remain unchanged.

The owned-block correspondence uses the same 4,096-node, depth-256 and pattern
bounds and at most 4,160 paired lexical identities (internal nodes plus the 64
captures). Its separate Copy/owned type inventories share the existing checked
type-node budget. An additional source wrapper can encounter ordinary source
or HIR bounds; extraction does not raise them to admit the transformed program.

The checked nominal inventory shares the existing per-module 4,096 distinct
type-identity ceiling and builder-byte charges with signature facts. Body,
contract, local-binding and pattern types now count in that same inventory;
the limit is not raised or replaced. Retention visits at most 1,048,576 combined
type/expression/statement/binding/pattern items per module at depth at most 256.
A fixed ancestor-cursor stack bounds scratch storage independently of sibling
list width. Its bounded HIR traversal and retained
facts do not change source meaning or give caches source authority. Graph schemas
remain unchanged, but newly retained facts and traversal storage are charged;
reported builder usage and consequently derived Graph/image bytes and digests
can change. Old evidence is not silently relabelled as evidence for a new image.
Previously admitted projects exceeding the expanded inventory can reject at
this resource boundary. This is not general unbounded nominal extraction.

These operations retain immutable in-memory candidates. They do not write
source, cache an image, publish a workspace generation, or commit Git changes.

## Evidence and remaining scope

Authored, unrun cases in `tests/project_candidate/extraction.rs` cover repeated
capture deduplication, internal let/match binders, lazy checked-failure placement,
mutable capture and contract rejection, identity/name collisions, exact replay,
stale changes, rebase after unrelated source movement, and unchanged disk bytes.
No local tests, compiler checks, or long quality gates were run, at the user's
request; these cases are not passing completion evidence.

`tests/project_candidate/nominal_extraction.rs` adds authored, unrun coverage
for nominal captures/results, whole-root field reads, body-only generic values,
rejection cases and exact candidate recovery. Discovery advertises the checked
Copy and whole-root constraints without claiming each expression is extractable.

`tests/project_candidate/owned_block_extraction.rs` authors the nested-owned
block lane and its rejection boundaries. These cases have not been run; actual
backend execution and cleanup/failure traces remain required evidence.
Private correspondence regressions first build ordinary extracted calculator
candidates, then reject a same-typed lexical-root substitution or changed
stable callee in a cloned helper HIR. They are also authored and unrun.

Owned captures or results, borrowed extraction, mutable capture copy-back, broader nominal types,
generic functions, contract extraction, propagation across function boundaries,
unsafe audit relocation, minimal effect inference, arbitrary extraction regions,
and runtime resource equivalence remain outside this version. The broader
graph-operational roadmap remains partial.
