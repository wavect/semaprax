# Exhaustive selected Bytes classification correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The actual unit-first 23e8 build failed at the exhaustive `VecOp` consumer mapping. The new record-only scalar field update is now explicitly classified as the existing owned-leaf-only operation for the distinct selected `Vec<Bytes>` carrier. The existing refusal remains exact; neither selected carrier admission nor the frozen public ABI changes. Ordinary #836 record update remains owned by its own checked descriptor and native/Wasm implementations.

Two existing owning controls are extended: the complete VecOp classification retains every previous expected refusal and adds UpdateField, and the exact-blocker admission table adds VecOwnedLeafOnly. No old case, assertion or diagnostic is removed. Complete immediate originals and unique full-source inverses are retained. Source inversion passed. Rust compilation and executable verification are pending the consolidated corrected batch; no test or build ran for this individual change.
