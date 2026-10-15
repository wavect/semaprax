# Exit prefix reference proof v1

Status: draft specification; qualification remains as recorded below.

Audience: compiler contributors, tool authors, and reviewers.

The ordinary receiving block loop checks each block's exact contiguous enumeration ID and region before any transition or terminator. Reaching a later block therefore proves every earlier block completed successfully. The function and its plan remain immutably borrowed throughout.

An exit key x was inserted by an earlier successful block exactly when its immutable declared owner o is an earlier checked block and that block's terminator is Exit(x). The original receiver immediately rejected an inserted exit whose declared owner differed from the inserting block. Earlier block IDs are distinct and index-addressable because their guards already passed. This proof uses the checked prefix, never a trusted incoming ID or an inventory count.

The current receiver retains the original bounds check first. It explicitly retains both constructor and per-exit zero-byte/zero-work authentication calls at their original positions and labels. If the declared owner equals the current block, no earlier block could have inserted that exit, so the original owner check suffices. A wrong owner triggers a bounded lookup of only the claimed earlier owner. The same exit terminator selects the original unknown-or-repeated diagnostic; a different terminator, future owner or out-of-range owner selects the original wrong-owner diagnostic. ExitTarget.id remains outside this phase's checks, exactly as in the original; later exit validation is unchanged.

Only malformed owners initialize a new probe. Before any new read, the receiver pays two complete initialized/receiving Probe layouts, the full optional lookup return, the separate previous/seen pattern bindings, comparison controls, and a source-derived ceiling of twenty primitive steps. No new Result-returning helper exists. Successful exits create no new carrier or positive allocation/work payment. Existing prices, canonical plan order, caps, floors, and sticky selection remain unchanged. No old Directory/header credit, replacement index, new Rc or allowance exists.

The old complete receiving function remains independently compiled. Its AVL operations and original exact boundaries remain its own contract. New work and physical boundaries are derived independently; truly tight budgets can refuse at different steps/labels because real work is removed and malformed probes have honest new payments. Non-budget diagnostics retain all six fields and their ordering.

Owning controls compare complete old/current outputs for prior/current/future/out-of-range owners, earlier same/different/non-exit terminators, forged IDs, repeated references and competing faults. They independently execute the removed AVL operations to verify the exact valid-route work/debit difference, observe full allocation/Drop, exercise no-exit zero authentication, and check valid and malformed exact/one-short/zero work and bytes with sticky retries. Evidence and exact selectors are under docs/evidence/exit-prefix-reference-proof. Rust execution and actual whole-route retry recovery remain pending.
