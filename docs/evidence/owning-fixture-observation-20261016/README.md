# Owning observation and retained-context fixture repairs

Status: source-only; the three existing owners must be executed before these
changes are qualified. Production compiler code, budgets and diagnostics do not
change.

The actual a82e selected run failed the duplicated driver owner at
`test_allocation_observer.rs:305`: its outer construction-through-Drop observation
contained a second `measure` call. The observer deliberately refuses nesting.
The inner allocation-free interval now uses two fixed `current_metrics` snapshots
inside the same outer observation. Both snapshots must be valid, the allocation
counter must be monotonic, and its interval delta must remain zero. The original
outer live/freed, receipt, ledger, report-backing and six diagnostic-field checks
remain. All four lexical/profile combinations and the independent old diagnostic
are retained.

The boundary and certificate whole-driver owners failed with SPX-G410 before
either compared route ran. They used `SourceRecordReplayFixture::with_context`,
whose legacy retained-composition route is deliberately closed. These complete
positive/hostile driver comparisons now use the fixture's existing
`with_prepared_context`. That route authenticates the actual module slice and
provider directory with the original preparation budget. The source subjects,
hostile cleanup mutations, both independent drivers, all six diagnostic-field
comparisons and their budget constructors are unchanged. The legacy callback
and its separately owned closed negatives remain available and unchanged.

`source-inverses.json` binds the complete a82e preimages and new source files.
Assertion-count equality is only a source audit; it does not prove the new
context admits these fixtures or establish executable coverage.

Focused existing library owners:

- `cleanup_plan::replay::selected_entry::tests::driver_local_receivers::duplicated_driver_profile_and_owner_keep_raw_oracle_and_transfer_one_paid_report`
- `cleanup_plan::replay::flat_records::tests::receiving_boundary::boundary_complete_paid_driver_keeps_source_and_first_refusal`
- `cleanup_plan::replay::flat_records::tests::receiving_certificate::certificate_precision_complete_paid_drivers_keep_source_and_first_refusal`

Original failures are retained in the external actual run
`/Volumes/Semaprax/lane-prompts/final-joined344-a82e00cc8-20261016`, logs 89, 99 and
103 respectively. They are not relabeled as passes by this packet.
