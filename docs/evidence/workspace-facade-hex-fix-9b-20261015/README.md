# Actual frozen9b facade digest compiler correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The archived actual round next-f045-final-qualification-20261015 built the unit and production library targets successfully on9b174d852798c99ce2d14b8170ec05e6c0f94b88, then failed integration compilation with two E0277 errors in historical_projection_root_facade. The SHA digest output array does not implement LowerHex. The full original JSONL, textual log and terminal101/462.475387-second result are retained here. Zero test bodies executed. These prior successful build phases are not new-head passes.

Exactly two digest expressions now use the public semaprax::digest_hex::LowerHex adapter already used by document_digest in this same integration file. All original assertions and whole-source comparisons remain unchanged. A strict two-source current-to9b predecessor is installed before both final branch input gateways; typed_cost.rs reads this complete integration helper, so the correction requires unit recompilation too. All old source packets and runtime JSONs remain immutable.

verify.py proves the entire helper restores exactly after removing only the two wrapper spellings, binds complete current/predecessor bytes for both source owners, and replays the unchanged frozen4/57/87/92/93 source verifier in authenticated9b bytes without rewriting its output. No Rust compiler, tests, Clippy or heavy commands were executed. New test bodies and production changes: zero. Future successors must preserve this packet and route their physical source through the full new predecessor before its protected domains.
