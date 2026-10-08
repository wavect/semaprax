# Same-Owner Byte-Buffer Renewal v1

Audience: language users and compiler contributors.

Status: implemented with focused local success/failure, hostile replay, cache,
native and Core-Wasm execution evidence. See the
[OPT batch receipt](../benchmarks/opt-batch-verification-v1/opt680-682-verification.json).

This additive profile closes the cleanup-history gap for the existing
same-owner byte-buffer replacement admitted by
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md). It adds no
syntax, byte operation, capacity, capability, or public ABI.

## Exact admitted update

The compiler derives renewal sites from retained typed HIR. A site is an
assignment to a whole mutable `Bytes` binding whose right-hand side is exactly
one of these compiler-owned calls and whose first owning argument is that same
whole binding:

- `bytes_set`
- `bytes_set5`
- `bytes_set1_or5_from_slice`
- `bytes_set1_or6_or48_from_slice`

All existing source and HIR checks remain in force: the assignment and first
argument must name the same binding, the operation signature must match, no
projection qualifies, and no overlapping loan may survive the replacement.
The profile does not admit another byte carrier, growable storage, another
owning operand to the update, a field update, or a general owned loop.
Unrelated existing owners remain permitted.

## CleanupPlan v17

A function containing an authenticated update selects
`semaprax.cleanup-plan.v17`. Before evaluating the call, `reserve_renewal`
records the binding's live leaf and its canonical initialization history. The
ordinary left-to-right call boundary moves the old generation into its staged
argument. Successful publication uses `renew` and restores the binding to its
reserved cleanup position. Unrelated live owners retain their exact order, so a
conditional update and the unchanged branch have the same join state and a
bounded while body has the same entry and exit state.

If the element check or a later operation fails before publication, the
selected status remains sticky and the staged old generation is finalized by
the existing failure exit. Staging removes the old generation from its named
slot and appends the call-argument epoch after unrelated live owners, so the
failure exit's reverse order finalizes that staged generation first. Only a
successful `renew` restores the reserved named-slot history. Exactly one
generation is live and finalized. The builder and independent replay both
derive the update from HIR and reject a missing reservation, an ordinary
transfer, a different binding, a forged projection, an immutable binding, or a
downgraded schema.

CleanupPlan v17 composes the existing v15 Vec and v16 String renewal semantics
when those shapes occur in the same function. Functions without authenticated
byte renewal retain their exact previous CleanupPlan selection and bytes. No
backend reconstructs, sorts, or repairs initialization history.

## Graph and backends

Such a program selects `semaprax.graph.v70`, preserving all preceding graph
facts and adding:

```json
"byte_buffer_renewal": {
  "schema": "semaprax.byte-buffer-renewal.v1",
  "updates": [
    { "function": "app.main", "at": "...", "binding": "..." }
  ]
}
```

Updates use canonical function and expression order. Graph v70 remains outside
the frozen evidence routes that reject newer graph schemas.

The interpreter already executes the admitted byte operations. Native C11 and
internal Core-Wasm consume the authenticated `reserve_renewal` and `renew`
transitions and the existing checked byte-operation status boundary. This
profile introduces no runtime import or carrier change. The public byte adapter
remains closed as specified by the owning buffer profiles.

## Focused gate

The owning language harness covers a conditional update with two simultaneous
owners, success and checked failure settlement, native and Wasm emission,
interpreter/native execution, physical Core-Wasm success/failure reentry, all
four update operations, same-function v15/v16 composition,
canonical graph replay, and deterministic source, graph, C, and Wasm output.
Independent cleanup replay mutates reservations, transfers, destinations,
mutability, and schema identity. The cache gate round-trips an exact v17 plan
and refuses the next unknown schema.
