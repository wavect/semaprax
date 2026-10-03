# Source-bound list induction v1

Status: bounded LAW-08 proof profile with local real-Lean evidence. It is not a
public collection ABI or a proof that backend executions terminate on every
mathematical list.

## Admitted source and denotation

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

## Boundaries

The kernel theorem is over mathematical lists. Runtime `Vec` capacity is at
most 8192 elements, `vec_push` may fail at capacity or allocation, and the
ordinary interpreter/native call-depth limit is 256. A runtime length or
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
refusals. The separate native C O0/O2 selector is the physical backend gate;
emission alone is not runtime evidence. Broader source forms and public ABI
remain separate admissions.
