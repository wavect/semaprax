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

The planner admits value-mode `i64`, `i32`, `u8`, `usize`, and `bool`, ordinary
String ownership, direct owned Bytes, and authenticated record/variant values
whose exact checked TypeFacts establish Sized resource-free storage. Nominal
values must be either Copy without drop obligations or non-Copy with drop
obligations. The same checks apply to parameters, results, body values and
local/pattern bindings. Bare source `string` parameters retain their checked
owning mode; explicit `own` remains required for Bytes and admitted owned
nominals. A source type's spelling or field shape does not establish eligibility.

Planning reuses the Project's bounded cross-module admission. An explicit
nongeneric resource-free record or variant containing owned `Bytes` may cross
the boundary as a stable-ID type import, so an owning nominal function can move
when no surviving caller needs a forbidden callable import. Imports of
functions exposing owned nominal arguments retain `SPX-G172`; borrowed storage,
generic types, resources and dependency cycles remain closed. Direct-Bytes and
String helpers retain their existing behavior.

Bodies and contracts can contain admitted scalar expressions, local bindings,
whole-binding assignments, loops, explicit monomorphic calls, Copy record and
variant construction, field reads, record updates, plain Copy matching and
admitted `match own` forms. String and byte-array literals, owned locals and
internal byte/string views are also planned. Direct source callees must
themselves have explicit top-level monomorphic resource-free signatures. At most
64 combined distinct direct callable and nominal type
dependencies are admitted, including dependencies from contracts. Matching
locals participate in alias hygiene.

Nominal type planning and nominal source/pattern rewriting each have a
4,096-node budget; the type-planning budget also charges each retained builtin
occurrence. Checked HIR traversal admits at most 1,048,576 visited items
and depth 256. These bounds do not charge ordinary scalar annotations as new
nominal syntax.

Borrowed/shared parameters, borrowed results, explicit borrowed matching,
resources and resource-containing values, field mutation, propagation, methods,
host calls and audited unsafe boundaries remain excluded. Internal `str` and
`Slice<u8>` views use ordinary checked loan provenance and cannot escape through
the relocated signature. Generic
function calls and generic source-type imports also remain closed. Fixed
compiler-owned Option/Result instances keep their direct `i64`/`bool` argument
rules. No type declaration moves with the function and no type argument is
inferred or converted by relocation.
Existing cross-module function-signature admission also remains unchanged:
moving a prelude-typed signature can still reject if surviving callers would
require a currently unsupported generic-signature import. A body-local prelude
value does not itself require a synthetic type import.

Compiler byte and string operations are recognized from retained checked HIR operation
identities at the exact original source occurrence. Their source spellings stay
unchanged and never become authored function imports. Source identity/binding
collisions and destination shadowing reject; dependency aliases reserve those
spellings. The operation inventory does not widen nominal admission:
`byte_get` still fails this planner's direct-`i64`/`bool` nominal argument gate
because its result is `Option<u8>`.
The same restriction preserves the current scalar type vocabulary: a
`string_from_char` call still encounters the planner's unsupported `char`
type, even though typed candidate expression construction can select that
operation. Other String operations can relocate only when every existing
signature, body, ownership, source namespace and import check succeeds.

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

Nominal dependencies bind their authenticated type owner and provider module.
The destination reuses an unambiguous existing local/imported type binding or
receives a deterministic type import from the actual provider. Newly selected
aliases avoid destination declarations/imports and moved lexical bindings.
Ambiguous or conflicting existing bindings reject rather than being silently
retargeted. Source type imports remain unchanged, including ones that become
unused; type-import pruning is not part of this operation.

An inferred nominal result or projected value need not have a source type alias:
its retained checked owner and complete declaration shape authenticate the
provider. Any actual type spelling in the moved source still requires an exact
authenticated source binding before it can be rewritten.

The moved signature, explicit local annotations, constructors and type-qualified
patterns use the destination binding for each stable type identity. Field and
case labels, local names and type arguments remain unchanged. This includes type
syntax inside contracts. A source-local type dependency can create a real
module cycle after relocation; full Project admission rejects that candidate.

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
The moved function's checked nominal identities are also independently compared
after rebuilding; a same-spelled destination type cannot replace the original
owner. This is semantic identity preservation, not a runtime-equivalence claim.
Place roots and projections, view-operation identities and byte-range operation
identities are compared after rebuilding. Unlike signature reordering, a move
introduces no argument staging or additional local copies. Ordinary source
verification still reconstructs loans and cleanup plans; the planner never
sorts or repairs cleanup vectors, performs a finalizer, or treats proof data as
runtime permission. Source locations and generated artifacts can change.

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
`tests/project_candidate/movement.rs` cover destination alias removal,
cross-file callers, body dependencies, source callers, obsolete import pruning,
hygienic alias collisions, contracts, exact replay without source writes,
fixed-export/main/path rejection, cycles, unrelated rename/body merges,
competing locations, and stale handles. None has been run in this change.

Additional authored, unrun nominal cases are in
`tests/project_candidate/nominal_movement.rs`. They cover destination type
bindings, aggregate syntax, replay, and rejected relocation shapes. Discovery
advertises checked nominal identity and type-binding migration constraints;
an advertised destination still requires full candidate admission.

`tests/project_candidate/owned_movement.rs` and
`tests/project_candidate/nominal_movement.rs` cover String call/import
migration, scalar-signature internal byte work, unused owning Bytes relocation,
one exact owning-nominal type import, replay and unchanged-source assertions.
Owned callable-import and cycle failures remain negative cases. These are not
physical execution or cross-module owning ABI evidence.

General declaration kinds, owned callable-import admission, public-export
origin migration, audited boundary relocation, broader expression syntax,
runtime equivalence evidence, and full graph-operational programme completion
remain outside this bounded slice.
