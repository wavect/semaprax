# Transient owned-function directory v1

Status: private source implementation and focused regressions authored;
compiler/type checking and executable qualification pending. This is a bounded
OPT847 representation change, not renamed-alias support or application fit.

The existing transient imported-function route lends authenticated name-equal
Vec/Box wrapper payloads and owns every other Function through GeneratedRoots.
Its former owned directory acquired `imports + synthetic_main` Function slots
before the ordered import walk, even if every imported payload was borrowed.
The ordered FunctionEntry directory still needs that complete entry count.

The owned directory now starts empty. Immediately before an actual owned
fallback Function or synthetic main is formed, lexical acquisition prepays its
count/layout/receiving/reservation/refusal roles and the complete requested next
Function backing. `try_reserve_exact(1)` grows to exactly `len + 1`, checked
against the observed capacity, before any new payload is cloned or initialized.
Existing backing remains charged; reallocations receive the entire new request,
without refund, cap adjustment or forecast credit. Each successful push fills
that slot. Thus the final owned capacity equals the initialized owned count;
all-borrowed imports with an authored main acquire no owned Function backing.
GeneratedRoots still owns partial payloads through every failure and Drop.

A new count pass is intentionally absent. The existing imported wrapper
classification and membership checks produce no retained classification table;
counting would repeat deep wrapper authentication and introduce another paid
walk ahead of the old allocation frontier. This implementation leaves those
checks, their order, permissions, fallback signature/layout/default generation,
source identities, and final immutable HIR/cache/image boundaries unchanged.

## Explicitly changed physical failure frontier

The old Function-directory acquisition preceded the FunctionEntry directory.
It has actually disappeared. Entry backing can now be acquired earlier; owned
backing is acquired later, after that fallback's unchanged signature/teardown
payments and before its Function clone, or before synthetic-main fields.
New finite receiving charges can refuse at those points. Allocation failure is
an ordinary sticky receiving refusal; no old refusal is cleared or normalized
into success. Semantic diagnostic construction and priority are unchanged for
admitted resource budgets, but constrained physical-budget first sites are not
claimed identical to the predecessor.

Exact one-slot growth intentionally exposes an unfavorable all-owned case:
multiple reallocations and repeated full-backing reservations can exceed the
old single directory allocation and bill. The owning observer prints both
actual complete constructors' reservations, allocations, reallocations,
allocated/freed bytes and highwater through Drop. It does not demand a universal
improvement or infer runtime/agent-token/cost gains. A general geometric policy
or new fixed minimum would be a separate design with different honest bills.

## Focused owning definitions

The original six alias producer owners remain registered with all assertions.
A complete immediate-old producer is compiled separately; its full original
source/test files are archived. Only module-parent import paths differ in the
compiled copy, and a complete-body custody assertion proves that correspondence.
The new lexical acquisition and every new owner source are included in the
new complete source manifest, before the original847 predecessor tables run.
Those immutable tables, their hashes and original assertions are unchanged.

Four new semaprax lib bodies under
`workspace_graph::expected_projection::borrowed_imports::typed::alias::owned_growth_tests::`
cover exact borrowed/owned/mixed/synthetic-main capacity and complete HIR
parity; full old/current physical/paid/zero/exact/one-short/partial-Drop
observations; exact reserved-name diagnostic and sticky refusal; and complete
source/old-receiver custody. Existing foreign/restored ownership, mixed ordinary
signature/type fallback and complete core graph/image/cache owners remain the
independent surrounding gates. None was run during authoring. Original native/
Wasm/profile and application requirements are not waived; existing final212
application/core selectors retain their exact pending scope.
