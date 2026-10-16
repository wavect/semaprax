# Checked-cache eligibility during bounded construction attempts

Status: private implementation; complete cold/warm/restored wire and current-head CI qualification pending.

Audience: compiler contributors and reviewers.

The existing private-layout audit observed equal complete graph/image fields
apart from failed-attempt builder usage and the derived digests. The prior
cold/warm records differed by 36 bytes. Source inspection finds a potential
frontier mismatch: a cache hit stages its owner and counters before revalidation
and aggregate residual debits, while fresh resolution can fail at a smaller
interior reservation. Attribution of the observed difference remains an
inference until the complete owning wire audit runs.

Before a bounded resolved-core attempt selects cached module or function HIR,
it now checks whether the entry's recorded resolver bill can fit the current
required account, including its reserved floor and sticky state. A candidate that cannot fit follows the complete existing fresh
resolver. The cache remains available for affordable entries and later attempts;
no persistent entry is removed, and AST reuse is unchanged. Eligibility spends
nothing and grants no storage or validation authority.

This is a proved lower bound: actual local validation plus the original
resolver residual equals the recorded resolver bill. Retained loan capacity is
not added to that minimum because its original residual subtracts the held
owner's current capacity. Shared-prelude work and loan residuals retain their
original later reservations; eligibility is not a guarantee that the full
attempt will fit.

The existing checked-source equality, source Agent association replay, checked
owner staging, HIR validation, prelude replay, loan capacity and residual debits
remain unchanged. Fresh resolution keeps all of its original reservations.
Failed attempts still contribute to the original maximum phase receipt; no
charge is clamped, rounded, refunded or omitted. Explicit-owner and Oracle
receiver bodies outside this production resolved-core selection are unchanged.

Owning tests compare the read-only query against actual required reservations
at zero, exact, short, floor, overflow, nested and sticky boundaries, with zero
observed allocation. Controlled entry-cost tests cover inert misses, real
checked-hit telemetry after an affordable decision, source drift and overflow.
Their cost literals test eligibility rather than a complete graph bill.
Complete source archives and exact inverses retain every byte of the original
receivers; the historical root bridge composes only two named routing edits
before all original hashes and inverses.

Local static-std controls execute the actual ledger, System observer, actual
entry/receiver layouts and complete new test bodies. Thin AST/HIR parser,
source-key and empty-Agent adapters exclude semantic authentication and full
fresh-resolution behavior from those component claims. The unchanged complete
cold/warm/restored wire audit and all current-head CI/full quality gates remain
required before this is described as fixing the observed wire discrepancy.
