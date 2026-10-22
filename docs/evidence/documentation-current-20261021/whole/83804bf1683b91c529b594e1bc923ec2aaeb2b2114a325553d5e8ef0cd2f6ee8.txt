# Current identity scratch geometry

The existing scratch test still required a three-word current DeclarationId and 390 words of saved key payload. All 14 type definitions match the previously qualified native layout witness byte for byte. Rust 1.88 and the current compiler measure the current identity as two words (16 bytes) and the independent historical identity as three words (24 bytes); a stale three-word current assertion is rejected with E0080.

The test now checks the current 195-key payload difference as 195 words. It also retains the historical 390-word census through the unchanged old producer's statically checked equality with String; historical-layout-contract.json binds that equation to the complete real old source. The whole 195-key fixture, saved owning construction, detached lookups, error ordering and allocation assertions remain unchanged. edit-proof.json supplies the exact whole-source inverse. No compiler behavior, tariff or quota changes.

These are type-layout and source-role checks. The complete original role verifier and all six hostile controls passed. The full owning allocation/lookup unit case, full current-head CI, browser and Lean qualification remain required; this evidence does not claim those gates passed.
