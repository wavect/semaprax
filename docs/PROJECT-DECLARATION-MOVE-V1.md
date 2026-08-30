# Project Declaration Move v1

Audience: compiler contributors and agents relocating checked declarations.

Status: authored implementation and unrun regression evidence. The user's
instruction explicitly skips local compiler, test, executable, and long quality
gates. This is not verified completion or target-execution evidence.

The additive `move_declaration` intention relocates one existing top-level
function between already authenticated Project modules. It preserves the
function's explicit stable ID and display name, reconstructs imports and calls
by stable identity, and passes through full candidate source and Project
admission. Human `.spx` files remain canonical; this operation constructs an
immutable private candidate and has no filesystem publication authority.

```json
{
  "kind": "move_declaration",
  "target": "application.helper",
  "destination": "application.destination-anchor"
}
```

These are the exact required keys. Both selectors are existing function stable
IDs of at most 4,096 UTF-8 bytes, without NUL. The destination selects a module
through an explicit monomorphic top-level anchor, including an existing `main`.
It supplies neither a source path nor an insertion position. The function is
appended after the destination module's existing functions.

## Admitted boundary

The target must be explicit, monomorphic, non-`main`, and absent from the fixed
manifest's selected Web exports. All selected exports are conservatively
excluded because relocation would change their authenticated source origin.
The destination must be a different module, permit every existing target
effect, and have no conflicting declaration or import alias with the moved
function's name. Importing this exact target is permitted: those imports become
the local declaration. Neither module permits nor function effects are widened.

This version admits value-mode `i64`, `i32`, `u8`, `usize`, and `bool`
parameters/results. Bodies and contracts can contain those scalar literals,
variables, unary/binary expressions, conditionals, scalar local bindings and
assignments, loops, and explicit monomorphic function calls. Direct callees
must themselves have explicit top-level scalar signatures. At most 64 distinct
direct callable dependencies are admitted, including calls from contracts.
Named/owned/borrowed types, generic instantiation, record/variant operations,
matching, propagation, field mutation, methods, host or compiler-builtin calls,
and audited unsafe boundaries fail closed. They are not silently rewritten or
treated as module-independent syntax.

These restrictions do not restrict unrelated functions in the Project. Caller
migration uses the existing bounded exhaustive AST walker, including contracts,
generic declarations, methods, guards, and loops in those callers. The full
Project verifier retains responsibility for all source/profile rules and
module dependency cycles; structural discovery does not pre-approve a move.

## Binding migration and replay

The compiler resolves the original target's call names through its admitted
local and imported function bindings. Dependencies already present in the
destination reuse a stable-ID binding; otherwise an import is constructed from
the existing provider's identity/module. The original alias is retained when
available. On collision, the compiler chooses the first available
`_spx_move_N` identifier, in ascending order bounded to 65,536 attempts, avoiding
destination declaration/import names and moved local names. A dependency whose
existing destination bindings all conflict with moved local names is rejected.
No submitted alias or source fragment gains authority.

Existing destination calls through imports of the moved function are rebound
to its unchanged local display name, and those target imports are removed.
Other consumers keep their aliases while their target module is updated. Any
remaining caller in the source module receives a function import for the moved
identity. Source function-import aliases used by the moved body/contracts are
removed only when no surviving source call uses that alias. Unrelated and
originally unused imports remain unchanged. Dependency imports are added in
stable-ID order; pre-existing retained imports keep their relative order.
Self-calls retain the moved stable identity. Real module cycles still reject
through ordinary complete Project admission.

The compiler-owned `DeclarationMove` fact contains only the moved ID and its
original/new path and module. Parent candidate checks transfer exactly the
existing function effect/contract inventory between those modules and permit
only that explicit identity's path/module fields to change. No new declaration
identity is allowed. The manifest, source inventory, other identities, effect
budgets, and existing contracts remain subject to their normal invariants.

After full candidate construction, `movement::validate` independently parses
the original retained sources, reconstructs the move, formats them under the
existing aggregate source limit, and compares every canonical source exactly.
It additionally compares the retained per-module HIR call inventory by caller
stable ID, phase (`requires`, `body`, `ensures`), callee stable ID, and call count.
That comparison includes local and cross-file calls from declared functions and
templates, with a 65,536-call bound. It ignores source paths and revision-scoped
expression IDs that relocation necessarily changes; it does not infer runtime
coverage or dynamic calls. Exact source reconstruction plus admitted HIR
bindings prevents an accidental alias change from silently changing a callee.

`destinations(revision, target)` supplies sorted structural anchor choices for
constructor discovery. Unsupported targets produce no choices. Namespace and
permit checks apply, but graph cycles and full semantic admission are still
decided by applying the exact request. Existing candidate replay reconstructs
the full history; rebase and merge retain their conservative location/conflict
checks and revalidate moved dependencies against the new admitted source base.

Malformed/unsupported constructors, selectors, namespace conflicts, and
prohibited relocation shapes report `SPX-G225`; local movement bounds report
`SPX-G226`. Source verification, profile admission, and cycle failures retain
their owning diagnostics. Stale candidate handles remain `SPX-G224`, and
competing semantic move histories remain `SPX-G235`. Failure leaves the
original candidate and canonical source files unchanged.

## Authored evidence and remaining work

`src/project/candidate/movement.rs` owns construction, discovery, and exact
post-validation. Parent candidate dispatch/invariants, catalogue, schemas, and
semantic rebase own integration. Five authored tests in
`tests/project_candidate_movement_v1.rs` cover destination alias removal,
cross-file callers, body dependencies, source callers, obsolete import pruning,
hygienic alias collisions, contracts, exact replay without source writes,
fixed-export/main/path rejection, cycles, unrelated rename/body merges,
competing locations, and stale handles. None has been run in this change.

General declaration kinds, named and owned type relocation, public-export
origin migration, audited boundary relocation, broader expression syntax,
runtime equivalence evidence, and full graph-operational programme completion
remain outside this bounded slice.
