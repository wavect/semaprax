# Linked nominal borrowed metadata v1

This source candidate changes storage operations inside the existing final-link
closure reconstruction. It retains every Phase-A kind/owner/membership check,
its source order, partial receiving-map contents, and complete diagnostic fields.
Field metadata borrows existing fields instead of cloning recursive types only
to discard them. A case-field coordinate retains the existing outer immutable
owner only when `NominalPayload::is_shared()` is true. Owned and decoded case
payloads retain the original complete independent Vec copy behavior.

This compatibility entry has no byte payer. No shared owner is created or
promoted here, and no paid-constructor, budget, work, cap or complete-funding
claim follows from this change. Receiving map/name allocations remain the
existing unfunded compatibility boundary. Public no-owner construction, decoded
fact coalescing and genuine paid final-link activation remain separate work.

The complete prior linker is independently compiled under cfg(test). Its three
associated method names and module imports differ only to coexist with current
production. The archive is verbatim c237 source. Logical Eq and cache wire
inspect complete values; representation and pointer identity grant no semantic
authority. Returned case views retain the actual shared owner beyond input Drop;
its last-owner Drop drains recursive types with its own prepaid scratch.

Four authored controls cover complete old/current maps and wire for Owned,
Shared and decoded inputs; complete selected errors and partial maps; physical
clone removal and complete lifetime; and synthetic 512-level last-owner disposal.
They have not been compiled or executed in this private lane.

## Legacy coordinate successor

The original scope above describes the retained predecessor. The source-only
`legacy-final-link-sharing-827` packet adds a separate Legacy coordinate branch
after the same complete fact checks; funded provenance still uses is_shared.
Decoded Owned inputs still copy. Full original archives and the complete prior
receivers remain unchanged and independently compiled. No new execution claim
is supplied by this additive note.
