# #836 missing backend controls: source-only follow-up

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Immediate basis: `186854fff7cc556ac1852019f39420094adc9996`. This is a
coverage correction only: every production backend byte, public import index,
private helper ABI and frozen old route is unchanged. The five original test
bodies and complete clone/reconstruct/whole-replace oracle remain; original
success owners additionally invoke the multi-Copy corpus.

The additive six-field record contains String and Bytes leaves, the selected
scalar, and three unrelated Copy fields: i64 minimum, negative f32 zero and an
f64 NaN. Both row zero and row one retain every unrelated source-level value.
Existing native probes compare every unrelated raw row byte, and the strict
Wasm host compares every unrelated word/owner; those predicates now actually
observe other Copy fields. The legacy three-field corpus remains unchanged.

A new native owner constructs real vector/row/payload ownership by calling the
ordinary checked source `field.seed` function. It invokes the actual generated
`spx_leaf_update_field` helper over twelve exact f32 and twelve exact f64 words:
positive/negative zero, both infinities, three distinct signed/payload NaNs,
minimum and maximum subnormals, minimum normals and positive/negative maximum
finite values. Each store also has independently chosen exact f32/f64 guard
words and an i64 guard. It checks exact selected bits, every unrelated byte,
identical backing/leaf identities, one generation renewal, zero program
allocation/free in the store, then complete real runtime Drop. It runs O0/O2
when the parent executes the owning test.

Eight native hostile cases use fresh source-created valid owners: stale
 generation, wrong nominal descriptor object, invalid authority, interior raw
pointer, forged length, generation exhaustion, owned ordinal, and wrong scalar
kind. Only the generated invariant fail-stop is intercepted with setjmp. The
fixture checks unchanged row bytes, input/output sentinels, status count,
generation state and live allocation count before settling the original valid
owner once. Program allocation/free during the checked helper still aborts the
fixture rather than being accepted as an expected invariant trap. Snapshot
objects potentially affected by the hostile call are static, avoiding automatic
object indeterminacy after longjmp.

The strict private Wasm host independently runs the same exact float-word
matrix through `spx_vec_update_field_v1` on freshly constructed genuine private
host owners, using nominal metadata witnessed from the checked compiled source.
The store does not mint authority or replace row/backing objects; each old
handle is rejected after renewal and every owner/payload settles afterward.
Guard-word changes and fixture snapshots occur outside the observed store.
JS BigInt/array fixture bookkeeping is not a zero-JS-heap or public-host claim.

These raw word controls concern the existing trusted native/private-host scalar
ABI. They do not invent a source bitcast intrinsic or claim arbitrary NaN payloads
can be authored as language literals. All words transported by the actual
helper/import preserve their exact payload bits. Ordinary source tests retain
the eight admitted scalar lanes and computed source NaNs/negative zero.

A public `interpreter::interpret` owner now runs the identical eight-lane,
multi-Copy, legacy, mutable-index/RHS-order, bounds/RHS/index-failure and old
oracle source corpus and verifies the complete public envelope. It does not
widen visibility of the crate-private resolved runner. Separate interpreter
alias, generation-overflow and trusted float payload/store controls are owned
by frontend follow-up `ad673fb8f9c1a4584c483b341ffa554665fc0661` after
`6a3f309d101f84ad8a6673233eec474b83877156`; those dependencies are not copied
into this backend worktree.

All Rust/C/Node/Wasm bodies remain **authored and unexecuted**. There is no
runtime pass, measured allocation saving, full issue completion, application
acceptance or token/cost claim. The parent owns the single heavy qualification
job and will execute the original five plus three new exact bodies, alongside
the retained original controls and frontend packet.

`source-inverses.json` binds four complete immediate-before files and two new
full sources. The old packet's complete metadata is archived byte for byte;
its thirteen original archives and all original inverse edits remain intact.
Only a local current-host preinverse and three child bindings were refreshed,
with original child hashes authenticated against the complete fix archives.
The shared source guard joins and binds both new native Rust/C sources.
`pending-selectors.json` registers exactly three new bodies and ten retained
bodies, including all original five. `source-only.py` proves these bindings,
identical native/Wasm raw bit matrices, preserved original test bodies/oracle,
unchanged production bytes and module budgets. Prior proof documents keep their
prior-checkpoint meaning; no global Source36/67-packet composition was run.

No Cargo, compiler, Rust/C/Node/Wasm execution, tests, Clippy, Claude, review
execution, paid call, push or remote writes occurred. ROOT, #833 and #835 were
not modified.
