# Residual855 independent-payer fixture correction

Source-only correction for actual344 index125, based on `a82e00cc83f99b16249181c45710d962598577db`. Runtime qualification remains pending. No macro, debit, production API, admission limit, or allocator behavior changed.

The original inline HIR test entered `with_limit_usage(debit)` beneath an already-refused outer budget with zero remaining bytes. That API deliberately clamps its child to `min(requested, parent.remaining)` and later debits the parent; it is not an independent payer. The child therefore had zero bytes. `resolved_residual_type!(I64)` correctly returned `None` before evaluating its argument, and the fixture's success unwrap was invalid.

The repaired control executes these domains in order:

1. The original zero-byte, first-refusal17 outer scope remains selected.
2. A clamped nested `with_limit_usage(debit)` explicitly refuses, leaves the argument unevaluated, uses zero bytes and allocates/frees nothing.
3. The existing `source_owner_scope(debit, ...)` independently owns the exact debit. Its formatting phase evaluates the I64 argument once, constructs the same unwrapped box, and fully drops it. Its receive phase sees the already-restored sticky outer account at zero remaining. Exact Type box allocate/free and zero live/reallocation assertions cover the complete construction/Drop window.
4. The original exact inner debit and restored outer refusal/zero-used/first17 assertions remain. Producer order and floor, zero, exact, short, prior-refusal and all four original payload controls are unchanged.

The source-owner module was already compiled under `cfg(test)` on every host. Its reexport now has that same test alternative; the original production platform predicate is unchanged. This preserves test coverage without adding a production entry point or changing its implementation.

`before/` preserves both complete original source files, including the entire original inline test body. `current-to-a82e.json` binds complete new input SHA256 to complete original SHA256 through unique exact preimages. No old source assertion, inverse table or archived predecessor was edited.

**Required final composition hook:** after joining this commit, stage the two rows (`src/hir/expr_nodes.rs`, `src/bounded_output.rs`) at the final current-to-historical adapter front before invoking older source predicates. Rebase only the rows' complete current endpoints if another authorized lane edits either source. Preserve the full a82e predecessor and every old assertion. This private commit intentionally adds no standalone adapter; parent owns the final joined Main043/Main44/a3/e47/authority refresh in a separate labeled phase, preserving prior receipts.

Pending exact owner selector:

`hir::expr_nodes::residual_payload_tests::lexical_residual_receiving_keeps_old_producer_order_floor_and_restored_sticky_owner`

Also retain the complete `hir::expr_nodes::residual_payload_tests::` owner group, whole-source guards, and ordinary cross-platform compilation. No Cargo, build or Rust test was run in this lane.
