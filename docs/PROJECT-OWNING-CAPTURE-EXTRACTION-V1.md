# Project Owning-Capture Extraction v1

Status: Partial; library implementation and regression sources authored, not
executed in this change.

Audience: compiler contributors, semantic tooling authors, and reviewers.

This is a narrow lane of the existing `extract_function` operation. It moves
one authenticated authored expression into a fresh helper when that expression
consumes exactly one whole local owning value. The only admitted owner types are
`Bytes` and bare `string`. The request and selector remain those of
[Project Function Extraction v1](PROJECT-EXTRACTION-V1.md); callers cannot name
captures, parameter modes, types, source spans, cleanup slots, or effects.

The compiler joins the selected AST expression to retained HIR before changing
source. It requires one immutable local owner, one unprojected owning `Place`
occurrence in the complete provider body, the same sole occurrence inside the
selection, no occurrence in `requires` or `ensures`, and one matching binding
slot in both cleanup inventory and the canonical cleanup plan. Parameters,
borrowed or shared roots, projections, multiple owners, multiple uses,
conditional or lazy uses, internal owning storage in the selection, resources,
and unsupported nominal owners fail closed. The owning lane also excludes
entrypoints and manifest exports.

The helper receives the exact owner with owning HIR semantics (`own Bytes`, or
the language's bare owning `string` parameter form). The caller evaluates the
new direct helper call at the selected expression's original position and
transfers the local there. Immutable Copy captures retain their existing
first-authored-use ordering; evaluating them introduces no effects. The helper
body is the exact selected subtree, so its internal left-to-right order remains
unchanged. The caller no longer cleans the transferred owner; the ordinary call
commit and helper entry cleanup state own that responsibility. An already
supported Copy or resource-free owned result may cross back through the
existing result publication rules.

After canonical source reconstruction, ordinary Project rebuilding must admit
the complete revision. A separate correspondence check maps each old capture
ValueId to exactly one rebuilt helper parameter, compares expression types,
ownership, projections and stable operation identities, and requires exactly
one whole live owning helper parameter with no conditional owner or crossing
loan. Candidate replay and recovery reconstruct the same source, semantic graph,
ownership, cleanup, interpreter closure, native C closure, and Core Wasm closure
under their existing profiles and bounds. This is static compiler admission;
the authored regression sources were not executed here.

Rebase remains conservative. Any concurrent change to the target body,
signature, or effects conflicts, as does reuse of the fresh helper identity.
An unrelated accepted change may rebase only after the selector maps through
the authenticated structural path and the entire transformed Project replays.

The operation performs no filesystem write, publication, package rewrite,
external-consumer discovery, ABI migration, or automatic call-site migration.
It does not establish runtime equivalence, performance equivalence, deployment
compatibility, or support for exported ownership boundaries. Existing source,
HIR, cleanup, candidate, native, Wasm, interpreter, node/depth, capture, and
output-byte limits are unchanged. `SPX-G506` owns unsupported owner shapes,
`SPX-G507` owns retained owner/cleanup authentication failure, and `SPX-G508`
owns rebuilt helper ownership correspondence failure.

Authored, unrun cases in
`tests/project_candidate/owned_block_extraction.rs` cover local Bytes and String
transfer, exact helper entry cleanup, canonical replay/recovery, interpreter and
native/Wasm emission reachability, and fail-closed parameter, projection,
conditional, and multiple-owner shapes. They do not constitute executed gate,
host runtime, external package, or production evidence.
