# Same-source paired transport correction

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

This supersedes only the earlier11a paired comparison proposal. Keep that full packet unchanged as historical evidence. The comparison MUST use the SAME final source/lock/devdependency/helper/sourceguard fixes and CODEGEN64 profile. Derive both variants from the final common commit, changing exactly one include_bytes path literal in compressed_fixture.rs: gzip sidecar versus the immutable raw JSON. There is no851-raw versus unrelated-new-code comparison.

The shared helper authenticates either transport. Compressed uses the existing bounded decoder; raw checks the exact output cap, length/hash and UTF-8 before copying. Both variants execute the same tiny raw/gzip positive/negative controls and all41 original assertions, with identical dependencies and no extra owner selector. Only one payload is compiled in either variant. This is a test-only transport switch, not a production feature or arbitrary fixture path.

source-inverses.json archives the complete immediate old/new helper. verify.py authenticates this full stage, proves the entire original11a packet unchanged, and executes its original unmodified Python verifier under the exact old helper input. It does not assert raw current bytes equal an old hash. Both original and successor Rust qualification remain UNEXECUTED. No RSS, wall-time or artifact improvement is claimed.

paired-comparison-proposal.json specifies the corrected common-source serial experiment. The external pair-static-proof will bind an exact clean common HEAD, a one-literal raw overlay, the complete Git entry set and all other unchanged file modes/object IDs. Re-derive after final assembly so both sides include identical merged source views. Use separate private targets, identical pre-staged cache/toolchain/profile and the unchanged7GiB guardian; no full suites or --all-targets/Clippy-tests expansion. If sampled RAM differs too little to resolve, report that honestly; an artifact delta alone is not compiler-RSS attribution. Keep841OPEN until actual meaningful paired measurement and the one owning test complete.
