# OPT implementation batch: focused local verification

Verification is incremental: previously passing cases are retained, and only failures are rerun after repairs. These are selected local checks, not full quality gates or a claim that every passing case was rerun at the final head. `focused-checks.json` records the latest outcome for each selected case. All 54 recorded selected cases have passing latest outcomes. Native Map and Project v25 passed their final focused rerun on source commit `94fadd14c`.

The selected clippy invocation is `cargo clippy --locked -p semaprax --lib --bin semaprax -- -D warnings`; round11 passed on source commit `94fadd14c`. Test builds use one Cargo job, disabled incremental compilation and stripped debug info, with a private worktree target directory. No full test suite, full quality script or all-target clippy was run.

Passing evidence includes source/canonical graph controls, frozen-profile refusals, interpreter execution, native C11 O0/O2 allocation settlement, and selected physical Wasm Node execution. Three generated Toolkit collection package gates passed in round8. These checks do not establish browser or hosted execution. Webapp v3/SG focused library cases passed in round4; separate pure and HTTP Node runtime checks passed after the prototype-safe migration repair.

Rounds3/4 precede the final repair series. Round6 used `4805025ab`, round8 used `b18427eaa`, round9 used `c1224c4c8`, and round10 used `b58386ab0`. Documentation and subsequently merged help/KAT changes are separate from those executable source snapshots. Follow-up repairs are recorded below when their focused checks pass.

[Compiler graph timings](OPT602-GRAPH-TIMINGS.md) compare three runs per version on two large fixtures. They are whole-version, prior-head debug CLI measurements, not live-agent cost or isolated optimization results. Live benchmark/receipt/context follow-ups remain separate.

Final repair build: `cargo test --locked -p semaprax --lib --test language --no-run`, followed by exactly the two failing-case filters. Both passed in round11: `map_collections_v2::map_set_v2_native_balances_all_owners_at_o0_and_o2` and `project::tests::stdin_stream_command::text::collections::v25_collections_transport_executes_retained_and_native_routes`. Native Map exercises all 18 cases four times each at O0 and O2 with exact output, zero live owners and equal allocation/free counts. Project checks retained entry/test/prepared paths and the native stdin adapter. No source expectations or ownership guards were weakened.

Follow-up [OPT #660](https://github.com/wavect/semaprax/issues/660) is verified
by [ten focused checks](opt660-verification.json): dormant and active temporary
anchor controls, source/graph round-trip, interpreter, native O0/O2 allocation
settlement, Wasm settlement, installed bundle pin and diagnostic help. Selected
clippy covered the compiler library and CLI with warnings denied. No full gates
ran. The ongoing LogLens campaign retained compiler94fadd14c; this fix applies
to subsequent rounds and cannot improve the recorded running campaign.
