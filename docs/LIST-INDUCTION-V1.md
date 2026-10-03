# Source-bound list induction v1

Status: bounded LAW-08 proof profile with local real-Lean evidence. It is not a
public collection ABI or a proof that backend executions terminate on every
mathematical list.

## Admitted source and denotation

The compiler now has a checked persistent `List<i64>`
source value with `list_nil`, `list_cons(head, tail)` and
`list_uncons(list) -> ListStep<i64>`. `ListStep::Nil` and
`ListStep::Cons { head, tail }` are ordinary explicit match cases. The v9
prelude contract binds constructor/case IDs and a maximum physical length of
8192. Both operations receive immutable values; a retained tail cannot be
modified, and constructor refusal leaves the input value live. The reference
carrier in `src/immutable_list.rs` uses shared immutable cons nodes and
iterative release of long unique spines. HIR and graph preserve the exact
declaration identities; the interpreter, native C and closed pure Core Wasm
execute them. The native
carrier allocates immutable cons nodes for the duration of one root invocation,
so copied lists share tails and all nodes are released at the root return.
Each constructed spine is limited to 8192 nodes; this is not a total
allocation limit across several shared lists. Core Wasm uses private immutable
16-byte cells, enforces the same per-list bound, and resets its heap at each
root invocation. Public adapters, effects and mixed profiles retain the named
`SPX-W130` refusal.

The additive `semaprax.immutable-list-induction-i64.v1` proof profile accepts
only the exact checked pure `List<i64>` `append(left, suffix)` and
`reverse(input)` definitions in
[`law08-immutable-list.spx`](../tests/fixtures/law08-immutable-list.spx).
`append` matches the left list and constructs `head :: append(tail, suffix)`;
`reverse` matches its input and calls `append(reverse(tail), [head])`.
Every constructor, case, recursive argument and call is authenticated against
the checked source before Lean is invoked. The generated definitions use
structural recursion on the visibly smaller tail and translate exact `i64`
elements to Lean `Int`. The separate
[`immutable-list-lemmas.json`](../proofs/law08/immutable-list-lemmas.json)
proves the same five fixed append/reverse laws under this distinct profile.
Source drift, a changed proof module, or a theorem association mismatch refuses
replay. The selected Project/LawSet route records this profile in its method
evidence and requires a real pinned Lean run. Neither the mathematical theorem
nor native C execution proves backend lowering correspondence.

The initial carrier is the existing compiler-owned `Iter<i64>` and
`IterStep<i64>` pair over a `Vec<i64>`. `iter_next` consumes one iterator and
reveals either `Done` or `Yield { item, rest }`; `rest` is the visibly smaller
tail. The exact pure, monomorphic source functions with IDs `list.append` and
`list.reverse` are checked before export. Their aggregate-valued owned match
arms may return only `Vec<i64>` in the narrow source/HIR profile. Parser,
formatter, ownership checking, HIR replay, graph, interpreter, and native/Core
Wasm emission retain the ordinary source paths.

The exporter recognizes every body expression, its ordered constructor cases,
owned parameter modes, calls and type arguments. `reverse` must return an
empty `Vec<i64>` with capacity 8192 in `Done` and push the current item onto
`reverse(rest)` in `Yield`. `append` must return its left vector in `Done` and
recurse on `rest` with the item pushed onto the left vector in `Yield`. The
fixed capacity is part of the exported source identity and is not erased from
the runtime claim. Extra requirements, effects, generics, hidden calls,
mutual recursion, non-tail recursion arguments and alternate bodies refuse.
Other declarations remain listed in complete coverage as unsupported.

`SemapraxLaw08.append` and `SemapraxLaw08.reverse` are the direct `List Int`
denotations of these authenticated implementations under **successful**
execution. `IterStep::Done` maps to `[]`; `Yield` maps to `item :: rest`;
`vec_push` maps to append-one at the end. Elements map from exact `i64`
values to Lean `Int`; there is no element arithmetic in either admitted body.
The spec list is immutable and finite but unbounded in length. The pinned
kernel accepts structural recursion on `rest`, with an explicit length
termination argument. No bounded enumeration, finite unrolling, or trusted
plugin result substitutes for that induction.

## Fixed laws and separate proof module

The exporter fixes five statements and their source-law IDs: `append_eq` and
`append_empty` bind `list.append`; `reverse_eq`, `reverse_length`, and
`reverse_involution` bind `list.reverse`. `reverse_eq` states element order,
not just length. Identity would satisfy length and involution but fail order.
The separately authored [proof module](../proofs/law08/list-lemmas.json)
supplies only proof tactics under these generated statements. It cannot change
the theorem, add a declaration, admit a hole or extend the kernel axiom set.
The existing kernel-report parser requires one `#print axioms` response per
theorem and allows only Lean's standard `propext`, `Quot.sound`, and
`Classical.choice`. A cyclic proof dependency is rejected by Lean. The proof
module is not a source of runtime authority.

The versioned `semaprax.list-induction-certificate.v1` binds canonical source,
generated definitions, separate proof module, exact Lean document, fixed
theorem-to-law associations, full declaration coverage and reported axiom
sets. Replay rechecks source/HIR and all bytes **before** another pinned
kernel run. A changed list body, element profile, proof tactic, theorem
association or document refuses. `verify` checks the embedded envelope;
`verify_against_module` additionally requires the current separately held
proof module and refuses a stale authored lemma before kernel invocation.
The direct API accepts a caller-held `LeanKernel`; the physical gate supplies
an explicitly installed pinned Lean 4.34.0 executable through the existing
bounded held-process provider. A fabricated caller capability is not physical
kernel evidence.

## Selected Project law attachment

A `list_induction` LawSet selector names one of the five fixed theorem names
and its required `list.append` or `list.reverse` declaration ID. Its law module
source path must resolve in the retained Project. The installed Lean route
replays the Project and LawSet, checks the selected HIR and exact source body,
and checks the caller's current separately held proof module. Only a real
pinned kernel result creates an opaque `VerifiedLawProof`. The versioned
certificate replays against current Project source and proof module before a
fresh kernel run; it cannot create law evidence by structural parsing alone.

Strict protected coverage requires the distinct `pinned_list_induction_lean`
policy with exact toolchain, proof-module digest and accepted axiom set.
Missing proof, changed module/source, altered association, wrong method
profile and rejected axioms leave coverage open. The attached theorem remains
a source denotation under successful execution, with `proved_lowering=false`.

## Boundaries

The kernel theorem is over mathematical lists. Runtime `Vec` capacity and
immutable `List<i64>` spine length are each at most 8192 elements; a push or
cons may fail at capacity or allocation, and the ordinary interpreter/native
call-depth limit is 256. A runtime length or
index is checked `usize`, whereas Lean `List.length` is `Nat`. Consequently
the theorem does not establish successful execution for every mathematical
list, overflow freedom of unrelated element arithmetic, backend lowering
preservation, ABI representation, foreign implementation equivalence,
publication authority or whole-program law completeness. The exported
coverage names unsupported declarations rather than treating them as proved.

Focused gate: `language structural_list_match::monomorphic_owned_iterator_tail_can_return_sequence_from_match`
checks source, formatter, HIR, graph, interpreter and deterministic native/Core
Wasm emission. `language structural_list_match::pinned_lean_replays_source_bound_unbounded_list_laws`
requires explicit `SEMAPRAX_LAW_LEAN` and exact version, runs real pinned Lean,
replays the certificate, and tests source/definition/association/axiom/cycle
refusals. Separate native C O0/O2 and real Node Core Wasm selectors
execute the bounded list program and check owner cleanup; emission alone is
not runtime evidence. Broader source forms and public ABI
remain separate admissions.

The `language structural_list_match::immutable_list_source` selector passed
3/3: source/formatter/HIR/graph/interpreter checks, real C11 O0/O2 execution
with the root release and 8192-node boundary, and real Node Core Wasm execution
with a private carrier and root reset. The selected Project/LawSet
`installed_immutable_list_source_proves_and_replays_selected_project_law`
gate passed 1/1 with installed Lean 4.34.0. It also refuses a wrong reverse
source and a stale authored proof module before kernel authority. The new
certificate binds the actual `List<i64>` source; both source-to-Lean profiles
remain success-denotation claims, not runtime/lowering proofs.
