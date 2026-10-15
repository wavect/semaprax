# Interrupted integration recovery

Source-only recovery of `403d8a409417baa8e99e6d488c567e6b9b6acc74` and typed constructor producer packet `bb6e11a1eb33c8604a9bd16005399d624cf68447`.

Both sides of each metadata conflict are archived byte-for-byte in `inline-stage{2,3}.json` and `report-stage{2,3}.json`; every archived chain independently reconstructed its complete fixed original before resolution. Active chains retain all prior hooks and receive additive complete-source preinverses. No originals, older proofs, dispatcher hooks, or source guards were removed. Current constructor child hashes and prior bindings remain explicit.

The only compiler corrections address two erroneous test import paths, the missing `app_census` import in the unchanged complete old receiver, and the exhaustive test adapter's new `SelectedChild` variant. The adapter constructs all six expected diagnostic fields independently and covers prefixed and unprefixed child refusals. Newly split typed projection producers are joined into their owning existing source audits.

`combined-source-proof.json`: 54 packets, 565 rows, all 36 complete source reconstructions. This includes all 21 typed-constructor inverse rows and four recovery correction rows. `static-integration-checks.json`: 91 existing private-layout purity sources, 6246 Rust files under the exact owning size-budget roots, all three session mirrors, and all four typed projection children joined without new exclusions. All evidence here is source-only.

The actual earlier build failure is preserved intact in `../failed-build-403d-20261015/`: production passed in 98 seconds; lib test compilation failed with seven compiler errors and zero tests executed. These are prior-head facts. Recovery ran no Cargo, compiler, tests, runtime gates, or reviews.

Required owning unit selector for the modified exhaustive adapter:
`cleanup_plan::replay::semantic_refusal::tests::every_atom_matches_complete_original_diagnostic_under_finite_account_through_drop`.

Constructor #825 residual legacy registry/layout/cost/rewrite/provider work and public/no-owner publication remain open. No whole-issue closure or runtime gain is claimed.
