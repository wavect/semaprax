# Validator frame ABI adaptation for #891

Status: SOURCE ONLY. No current compiled size, saving or successful gate is claimed.

The saved owned iterative validator still executes with live HIR. Its full enum
contains direct ResolvedType in NativeNext/NativeAfterArg, Cow<ResolvedType> in
CallNext/CallAfterArg and Option<(ResolvedType, OwnershipMode)> in match frames.
VariantMatchNext/VariantMatchAfterArm carry the full scope, id, arm-scope and
covered-set headers beside that result; a declaration-tag payload change can
change this maximum without changing any variant or operation. A fixed historical
==248 assertion against the live type therefore cannot remain its ABI premise.

The executable enum, all dispatch files and behavior remain byte-identical. Only
its const assertion now checks exact size AND alignment equality to a complete
current-type frame witness. The witness macro contains every original variant,
field and order. A second instantiation uses the complete genuine530 ResolvedType
shape and its genuine fat-static-string/Arc DeclarationId shape. It keeps exact
historical frame ==248 and identity ==24 compiled. It is a layout control, never
a substitute semantic producer or padded fake baseline. The old complete producer
and old exact assertion remain archived and the whole producer is restored by
the finite inverse.

The current whole-frame size plus the historical Option result size must exactly
equal historical whole-frame size plus the current Option result size. This typed
equation introduces no guessed current width or offset and fails if maximum-variant
selection or padding does not obey it. The current exact witness and historical
alignment checks remain independent of the unchanged <=248 funding ceiling.

CallParameters and its held Call/Slot dependencies, and call_signature's Cow alias,
are complete byte-equal530/current sources (receipt hashes retained). Other generic
collections and references carry their elements behind headers; typed Vec/set
size and alignment controls accompany the complete frame checks. The current
witness uses the actual current types everywhere. No compiler layout is inferred
from a truncated largest-variant sketch.

Production validation.rs, hir.rs's248 tariff and all four saved dispatch modules
remain byte-identicalfc085. No debit, work limit, cap, producer error order or
physical allocator measurement changes. The historical530 owning build receipt
is retained by exact path/hash and successful compile status only; it does not
qualify this repair. No production assertion is changed.

Two focused owning tests print actual old/current frame and ResolvedType sizes,
alignments and Option result costs, and bind full old source/enum/type/id shape.
Existing scope_checkpoint::fresh::tests owners for exact/short, sticky zero,
complete old/current function corpus and unwind Drop remain required on their
original ledgers; they are unchanged and no pass is inferred here. The inline
witness adds no physical module, role or top-level harness. Final source custody
for this existing changed module belongs to the combined corrective batch.
