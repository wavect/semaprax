# a417 joined purity / Session mirror correction

Private source-only correction from a41774c0b. No Cargo/rustc/runtime or ROOT
source write. Actual a417 failure logs14/124 are retained, never relabeled.

The complete private-layout audit includes56 sources. Its25 recently joined
compiler/test files contained28 exact direct wildcard imports. All are replaced
with explicit imported names; no audit join, assertion, test or branch is
removed. Names used by sibling/descendant tests retain their original lexical
ancestor, with test-only imports scoped accordingly. Algorithm and string bytes
remain exact after removing only use declarations. No production algorithm,
carrier, cap, work or semantic output changes.

The Session source mirror now spells its field `semantic_refusal::Receipt`, as
its complete historical archive does. A tiny explicit local reexport binds
that path to the already independent prior_layout::Receipt. This retains the
historical Receipt layout and every passing original sum/type equality; it does
not accidentally substitute the new selected-static Receipt. The exact source
comparison is unchanged, including its existing FactReceipt qualification
handling. All three existing source mirror assertions are replayed exactly by
Python. This is source equality, not a Rust test PASS.

Own25 complete a417 source archives and whole-source inverses retain all prior
source bytes. Active25 packets/311 rows are composed against exact immutable
a417 metadata, including all41 #780 originals, Memo9/alias10 and Session19.
Original archives, original hook fields and old proofs remain unchanged.
Whole original projections and current bindings verify; all36 existing source
reconstruction sections also verify. Fresh combined-source-proof.json records
this source-only result, without replacing any prior proof with a subset.

The pending selectors are the same existing purity and Session source owners.
No additional test harness/gate is added. Actual compilation and the unchanged
runtime/type/layout controls still require the parent's combined campaign.
