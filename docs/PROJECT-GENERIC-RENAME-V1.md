# Project Generic Template Rename v1

Status: run. This is a narrow semantic-change surface, not general generic
signature evolution. It carries no runtime claim: the regression exercises
compiler admission only, and native, Wasm, and interpreter execution of a
retained instance remain unproven here.

Audience: compiler contributors, agent authors, and candidate reviewers.

## Contract

`rename_declaration` accepts an explicit, non-entry generic function template
that belongs to the authenticated Project revision. The compiler authenticates
the source declaration against the retained checked template before changing
source. It inventories at most 4,096 retained concrete instances owned by that
template and rejects ambiguous, malformed, or duplicate retained identities.

The source transformation changes only the provider display name and calls in
the provider module whose existing binding resolves to the same stable
declaration identity. Imported aliases in other modules stay unchanged. Calls
with explicit type arguments retain those arguments and their evaluation order.

The candidate then follows the ordinary full-Project path: canonical format,
reparse, Phase-A build, ownership and cleanup verification, linkage, profile
admission, target admission, and an independent source replay. After replay,
the compiler locates the same retained template owner and compares normalized
checked HIR. It ignores the template and instantiated function display names,
plus source spans that necessarily move after a width-changing rename. Stable
structural expression/value identities are not ignored. Template identity and
structure, concrete instance identities, exact type arguments, signatures,
bodies, effects, contracts, ownership, cleanup plans, and loan plans must
otherwise compare exactly.

The operation summary records `generic_instances.preserved = true` and names
its evidence as exact retained checked template and instance HIR. This is a
compiler admission fact. It does not claim that an instance executed, that all
possible instantiations are retained, or that external and dynamically loaded
consumers are compatible.

## Diagnostics and bounds

- `SPX-G503` rejects missing, ambiguous, mismatched, or malformed authenticated
  generic template and instance facts.
- `SPX-G504` rejects a retained instance inventory above 4,096.
- `SPX-G505` rejects candidate replay that loses or moves the template, or
  changes its normalized checked template or concrete instance inventory.

All other ordinary parser, resolver, ownership, cleanup, profile, target, and
candidate diagnostics remain authoritative. A failed change leaves the base
candidate and authoritative source files unchanged. In particular, every
candidate intention other than the display rename keeps the pre-existing
`SPX-G225` monomorphic-target grammar rejection; a generic template never
reaches a profile or linker gate through those routes.

## Linkage

A Project owned-data profile links one exact closure, not whole modules, so the
generic surface has to be linked the same way. The workspace linker walks the
entry and selected public roots over `(callee, type arguments)` call sites
rather than bare callee identities: a call that names an authored template
selects exactly the instance its type-argument vector derives, that instance is
retained, and the walk continues through the instance body's own callees. A
template whose call site has no authenticated instance fails closed rather than
being dropped. An unreferenced template or instance is not linked at all.

Retained templates carry their Phase-A declaration facts and canonical
type-parameter metadata into the linked program, and retained instances are
materialized in exactly the order an independent replay of the linked
monomorphic bodies discovers them, which is the sequence canonical HIR
validation reconstructs. Because that sequence is derived from monomorphic
bodies alone, an instance reachable only from another instance body is not
representable and is rejected, matching the resolver's existing refusal of
generic-to-generic relays.

The private Wasm lowering treats a retained instance body as an ordinary
executable function under its generic execution identity; a template is never
executable. Shadow-stack and arena accounting charges a generic call edge the
largest frame among that template's retained instances, because the physical
callee is one of them.

## Evidence

The authored regression
[`tests/project_candidate/generic_rename.rs`](../tests/project_candidate/generic_rename.rs)
uses two distinct concrete instances (`i64` and `bool`), checks exact retained
instance descriptors before and after the rename, checks same-module typed call
migration and a preserved cross-module alias, and keeps unsupported generic
body replacement rejected with the ordinary `SPX-G225`.

The compared descriptor is the instance identity, type arguments, signature
counts, effects, and facet names. It excludes the opaque facet handles, which
bind the image revision by a separate standing contract:
`image_protocol::function_instances_v1::
opaque_references_bind_template_instance_facet_page_shape_and_image` requires a
surviving instance's prior-image handle to be rejected with `SPX-G229`. The
regression asserts both — descriptors compare equal and handles necessarily
rebind — so the two contracts cannot silently diverge.

The regression is compiler-admission evidence only. Native, Wasm, and
interpreter execution of a retained instance remain required before any runtime
support claim.
