# Project Field Place Constructor v1

Status: authored, unrun constructor and regression coverage. No completion
promotion or target execution evidence.

Audience: compiler contributors, agent builders, and reviewers.

The closed typed expression selects one direct source-record field through its
persistent identity and an existing lexical root:

```json
{"kind":"field_place","target":"packets.packet.payload","root":"packet"}
```

The request has exactly these three keys. `root` is a bounded local identifier,
not source text, a dotted path, a recursive base expression, or an arbitrary
value ID. `target` identifies an explicit field of an authenticated source
record. The compiler derives its field spelling and owner from checked source
declarations. A same-spelling field on a different record cannot satisfy the
selected identity.

## Exact owner and direct source lowering

The expression constructor carries optional nominal type facts for lexical
bindings. Existing expression selections supply their checked HIR scope;
function parameters supply their authenticated declared types. Constructor
bindings propagate exact nominal identities from their initializers. A field
selection must agree with that root's record identity and ordered type
arguments. A type cannot be inferred from the requested field merely to make
the selection succeed. Missing or ambiguous facts reject with `SPX-G225`.

Propagation follows the actual constructed AST: aliases retain their known
type; record/variant constructors retain the exact nominal instance; a call
uses its checked callee's return type; record updates retain the base type;
and field selection substitutes the owner's exact generic arguments. Scoped
bindings use their initializer facts. Conditional and match results require
every branch to agree on the complete type. An unknown initializer or join
does not become known from a compiler-generated local annotation. Imported
call results use the provider's checked type identity, independent of local
type aliases. This is conservative discovery within the existing constructor
grammar, not a second general source type checker.

These facts select source syntax; they do not replace type inference, loan
checking, ownership liveness, or candidate admission. The compiler lowers a
successful selection directly to the ordinary field expression, conceptually
`packet.payload`. It creates no temporary, typed value binding, copy, loan,
move, or cleanup operation of its own. The surrounding source expression
determines whether that field is copied, moved, or borrowed.

The existing [value projection constructor](PROJECT-AGGREGATE-CONSTRUCTORS-V1.md)
remains available for a recursive `base`. It deliberately stages that base
once into an exactly typed local. `field_place` instead requires an already
named root and leaves that root in its original storage.

## Borrowing and source verification

A typed builtin call can use the selected field directly:

```json
{
  "kind":"builtin_call",
  "target":"core.bytes.as-slice",
  "arguments":[
    {"kind":"field_place","target":"packets.packet.payload","root":"packet"}
  ]
}
```

This composition uses the existing
[projected owned-byte field borrow profile](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md).
That profile requires an exact named owned root, a flat monomorphic owned-byte
record, one direct owned `Bytes` field, and `bytes_as_slice`. Immutable shared
borrowing does not require the local to be mutable. Borrowed roots,
temporaries, deeper projections, mutable loans, and escaping views do not
become legal because the field selector is authenticated.

The ordinary verifier rejects moves or mutations that overlap a live loan and
retains the stable field identity in loan provenance. A sibling field and a
parent place follow the existing prefix-overlap rules. Cleanup inventory and
canonical CleanupPlan order remain unchanged. The constructor does not claim
that every visible field can be borrowed or that a lexically visible owner is
still live.

Every intention still materializes canonical source in memory, reparses it,
and rebuilds the complete candidate. Body and expression holes reuse that
path; an unresolved draft cannot be materialized. Failed requests leave the
original candidate, draft, and authoritative source unchanged. Constructor
discovery, schemas, and successful candidate validation grant no publication,
filesystem, build, or test authority.

## Discovery, rebase, and bounds

Target-specific catalogues and full hole contexts expose eligible field
descriptors in optional nonempty `field_places`, separately from value
projections. They identify the field and
owner, checked field type, source provenance, and existing record binding.
Rows retain the projection descriptor's source monomorphic or source generic
shape, with `kind: field_place`,
`base_evaluation: direct_named_place_no_staging`, and
`root_requirement: authenticated_lexical_nominal_binding`. Generic metadata
describes the checked template; the actual root supplies its ordered arguments.
They describe possible selections, not root-type, last-use, expected-result,
or borrow-admission proofs. Compact hole constructor pages include the kind;
agents can obtain full descriptors from the ordinary context or catalogue.

The self-contained [constructor schemas](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md)
describe the closed recursive expression alternative. Generated clients use
the compiler-owned schema; no new transport authority is introduced.

Semantic rebase authenticates the selected field and complete owning-record
descriptor at each original and rebased history step, then reconstructs and
verifies the candidate source. Reidentified members or incompatible owner
shapes conflict with `SPX-G235`; display names are not an identity fallback.
The field-place dependency fingerprint excludes owner and field display names
so a stable-ID display rename can be replayed with the new source spelling.
It retains identities, declaration order, types, type parameters, and source
provenance. Other descriptor changes can still conflict conservatively. The
older value-projection fingerprint is unchanged. This is not general
structural compatibility or behavioral equivalence evidence.

The existing target-body, signature, effect, and contract conflict checks still
apply before dependency replay.
If the concurrent field rename also changes an expression in the original
target body, its source-body fingerprint can conservatively reject with
`SPX-G235`. A newly requested field use can follow a display rename when that
original target region is otherwise unchanged. General stable-field
normalization of pre-existing body expressions remains separate work.

Existing request byte, expression depth/node, and enclosing catalogue/context
bounds remain in force. Nominal fact propagation does not reset those budgets.
The wire field-place node lowers to a projection plus one variable node and
one additional depth level; both count against the shared expression budget.
Unknown root types and shapes outside the constructor's admitted nominal
profile fail closed. Source diagnostics retain their existing type, ownership,
provenance, and target-specific codes.

The focused [candidate regressions](../tests/project_candidate_field_places_v1.rs)
and [transport regressions](../tests/image_field_place_transport_v5.rs) are
authored but unrun. They cover direct source storage and loan provenance,
nominal root mismatch, checked local and constructor scopes, hole lifecycle,
recovery, rebase boundaries, and closed schema/discovery surfaces.
No compiler, runtime, generated client, or editor execution is claimed by this
document. Broader
ownership-aware constructors and general nested borrowing remain open.
