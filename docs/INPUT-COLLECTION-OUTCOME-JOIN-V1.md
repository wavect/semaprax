# Input Collection Outcome Join v1

Status: locally qualified for the bounded v1 join.
Audience: compiler contributors, reviewers, and checked-source authors.

SUMMARY: An exact owned normalization Input may join into an independently admitted collection outcome at an ordinary aggregate match tail.

## Authority and scope

The input is a monomorphic, invariant-free, explicit-identity two-case variant.
One case contains exactly `Bytes, usize`; the other contains exactly
`i64, usize, i64`, in declaration field order. Case order and names carry no
authority. Source and HIR independently authenticate declarations, identities,
field types, ownership, and the owning match.

The result must already satisfy the existing collection-outcome classifier:
flat Copy-record vectors, owned-leaf vectors, or the retained nested owning
record outcome. Every arm has the same exact result type and Own mode. This
composition does not admit arbitrary nominal results, borrowed publication,
new return/Never contours, generic carriers, or new public ABI layouts.
The existing [nested outcome contract](OWNED-NESTED-OUTCOMES-V1.md) and
[stream command contract](STREAM-DATA-COMMAND-V2.md) retain their independent
bounds and profile authority.

Source iterative and recursive verification use the same source relationship;
HIR reconstructs it from current declarations. Project import replay must
reconstruct actual provider layouts and explicit roots. Cached facts, names,
generated IDs, and cleanup attachments cannot grant the relationship.
Fresh authenticated provider field identities are valid; stale HIR field IDs
must fail independent replay.

## Settlement and behavior

An arm may call a decoder with a view of its still-live Input Bytes. The exact
arm boundary settles that input before result publication. Existing canonical
transfer and finalizer order remains authoritative. A decoder failure retains
its normalized sticky status, settles input and staged output, and publishes no
result. Postcondition failure follows the same existing settlement protocol.
Allocation quotas, evaluation order, fuel, Vec/Bytes limits, and backend carriers
retain their existing contracts.

## Owning verification

`hir::collection_outcome::stream_input::flat_tests` covers source/HIR,
cache/graph, imported-provider, and forged-transfer checks.
`source_verify::iterative_verifier_tests::normalized_input_flat_aggregate_join_matches_recursive_oracle`
compares both flat families with the independent source oracle.
`owned_collection_outcome::normalized_input` exercises
both families on interpreter, native O0/O2, and private Wasm: normal/error/empty
joins differential against the old direct-callee workaround, decoder failure,
postcondition failure, settlement, poisoned publication, and reentry.
The existing Catalog 23-case gate executes both source routes without changing
wire cases; its unrelated projected-owned-record refusal remains required.
Malformed Input, unsupported result, wrong mode, missing/duplicate transfer,
foreign field/guard, and missing-finalizer controls remain refusals.

The ten focused checks passed. The complete Catalog23 gate ran both the
ordinary join and the retained workaround against the same input, status,
stdout and stderr expectations at O0 and O2. The retained flat and nested
stream cases passed at their ordinary limits. The owning probes verified
settlement through final Drop and reentry after failure. Agent token, turn
and cost effects remain unmeasured.
