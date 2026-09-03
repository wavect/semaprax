# Project Generic Template Rename v1

Status: authored, unrun. This is a narrow semantic-change surface, not general
generic signature evolution.

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
checked HIR. The template display name and instantiated function display names
are the only ignored fields. Template identity and structure, concrete instance
identities, exact type arguments, signatures, bodies, effects, contracts,
ownership, cleanup plans, and loan plans must otherwise compare exactly.

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
candidate and authoritative source files unchanged.

## Evidence

The authored regression
[`tests/project_candidate/generic_rename.rs`](../tests/project_candidate/generic_rename.rs)
uses two distinct concrete instances (`i64` and `bool`), checks exact retained
instance descriptors before and after the rename, checks same-module typed call
migration and a preserved cross-module alias, and keeps unsupported generic
body replacement rejected. The regression is intentionally unrun; native,
Wasm, and interpreter execution remain required before any runtime support
claim.
