# Admitted report fixtures: source-only correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Both actual35cc positive owners failed before generation because imported Point constructor expression types selected Graph v74. `workspace_graph/source_schema.rs:465` supplies module-local types to the classifier; `hir/collection_outcome/nested/profile.rs:85` conservatively treats an unknown noncompiler nominal as requiring that profile. Walking each imported record constructor therefore selects v74 despite the provider-only Copy-match rewrite.

The two corrected fixtures keep Point/Metric and field stable IDs, local construction and Copy record extraction, all original type imports/aliases, effects, call values, candidate +1 body change and rename, and structural create/delete/move/replace operations. Their cross-module function argument becomes i64 so consumers no longer construct an imported nominal. Local Point construction has a locally authenticated declaration; the unchanged base lattice selects v13 for its record match. This is a bounded positive fixture change, not production admission of v74. All base and replacement callers are transformed consistently, with unique-source occurrence checks.

The complete positive transaction/report comparison bodies, negative evidence bodies, six-field comparison helper, and original unsupported fixture tests remain byte-exact. Both original parent fixture producers remain untouched. Whole two-source current-to35cc inverses and complete beforeimages accompany this packet. No new test bodies/selectors. No Rust, Cargo, build or test execution; the expected admitted schema and transaction success remain pending actual qualification.

Required existing owners (workspace harness):
- semantic_change::reporting::typed_public_generation_verification_and_application_keep_full_legacy_bytes
- semantic_structural_change::reporting::typed_public_generation_verification_and_application_keep_full_legacy_bytes

Retain both existing `full_original_projected_record_*` owners and evidence-refusal owners in the affected selection. The compositor must preproject these two complete current files to35cc before prior full-source reads; old JSON packets are unchanged.
