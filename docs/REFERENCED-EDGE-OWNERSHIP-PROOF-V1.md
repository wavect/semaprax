# Referenced edge ownership proof v1

Status: draft specification; qualification remains as recorded below.

Audience: compiler contributors, tool authors, and reviewers.

The ordinary receiving replay may omit its write-only referenced-edge AVL directory only inside the complete guarded block loop. Every processed block must first pass its unchanged exact contiguous ID and region checks. Goto carries one edge; Branch must first pass the unchanged exactly-two-distinct-edge check. Each addressed edge must exist and have its exact addressed ID and the current block as owner.

Those existing checks imply uniqueness: one immutable edge cannot have two distinct block owners. Two references inside the only multi-edge terminator have already been rejected before address validation. Edge inventory counts, trusted incoming IDs, cached verdicts and sorting are not premises. The first complete edge loop still authenticates contiguous edge IDs and endpoints before block processing. The helper is private and relies on these caller-side preconditions.

All other checks retain their order. Unknown-edge and wrong-owner diagnostics retain all six fields. The exit-reference directory remains because its duplicate and wrong-owner diagnostics have different selection order. The tree branch and complete prior receiving body remain independently compiled.

The optimization eliminates real directory probes, insertion, growth, rebalancing and their associated allocations. Existing unit prices and all unrelated operations remain unchanged. No replacement index, proof carrier, allowance, refund or old-header credit exists. Every surviving directory authentication and the retained token's exact function/budget/current-ledger checks remain. Old and new exact work/byte gates are derived from their independently executed actual full receivers; old contracts are not relabeled as new boundaries.

Owning controls and source-only inverse proofs are listed in docs/evidence/referenced-edge-ownership-proof. Runtime qualification and actual owner17 recovery remain pending. The prior observed work failure named cleanup referenced edge directory; moving past that phase would not alone establish whole-route recovery.
