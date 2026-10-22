# Exact paid constructor census

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The complete original filesystem scan and owning assertion reproduced the stale expected-list failure on main 8925503d5e. The scanner intentionally includes test-only Rust modules whose filenames do not end in tests.rs. It therefore sees both paid calls in declaration_tag.rs and the call in its independent historical decoder. The expected list now includes those exact three calls. Every prior row, scan predicate, production-domain check and refusal-carrier assertion remains. edit-proof.json checks the exact whole-source inverse.

The complete original helper and test were compiled unchanged as a standalone std-only harness before the edit, then compiled and run again with only the three expected rows and explanatory comment added. The original test failed before and passed after, scanning the actual entire source tree. This is qualification of this source-only census control, not a full compiler build or current-head CI claim. Both complete source-role domains and all six original hostile controls also passed under the unchanged 200 MiB guard. Full current-head quality profiles and Lean qualification remain required.
