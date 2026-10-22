# opt885-facade-dependency-20261018 README

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The a110 owning build reported E0433 because the WASM test harness registered main_e177_final_inputs without its existing next_big_batch_inputs dependency. Exhaustive literal search across src, tests and crates found three compiled registrations: library and workspace already contain the dependency; only the WASM facade needs the same registration. Both original module bodies and all existing tests remain unchanged. The complete old harness and exact one-edit inverse are retained. No new Rust module file, fallback weakening, test skip, cap change, Cargo or runtime execution. Both direct physical174 and FINAL6 table intersections are empty.
