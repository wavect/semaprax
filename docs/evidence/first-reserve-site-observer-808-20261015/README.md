# Test-only first reserve-site observation for #808/#837 discovery

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base9f5d03967075a2250041344dd277fd4668ac039b includes only the separate model canonicalization on35cc. This commit changes two existing test-only files. Trace receives one fixed Option<&static Location> field. Its Guard::drop copies the already recorded Budget.first_site only AFTER its existing active-budget acquisition and Rc owner equality assertion. No new budget/Rc/TLS query, allocation, string clone, dynamic directory, IO, payer/authority or reserve is added. The field and its copies are explicitly unfunded test-observer stack/TLS controls, not a paid compiler carrier claim. Release diagnostics, first receipt, all six Diagnostic fields, fees/caps/work, max-consumed selection and LAST selection are unchanged.

Three existing owning tests retain every old assertion and add exact first-location checks across a later sticky retry, a foreign nested refusal followed by restored parent refusal, an unrelated stale ledger, a fresh successful owner, and existing success/max/LAST zero-allocation observations. No new test body/harness. No Rust/Cargo/build/test/Clippy execution occurred. Full immediate beforeimages and inverses retained.

The existing codec808_frontier owner prints Debug Trace only after the actual ordinary ledger settles, so both maximum and final attempt logs can expose this static site without any new in-measurement output or altered failure publication. Existing track_caller propagation on ReplayBudget reserve_vec/reserve_bytes and required active reservation can carry the real call site; untracked alternate ledger wrappers may reveal only their wrapper site. Inspect actual evidence before selecting an optimization. No performance, cap-fit or new root-cause claim is made. Existing #808 frontier diagnostics cover this work; no duplicate issue is recommended.

Source projection: prepend current full stage_debits.rs/tests.rs→this immediate predecessor before historical source readers. Keep old protected JSON and source assertions unchanged. No new Rust source owner is created; existing source-audit input lists are unchanged.
