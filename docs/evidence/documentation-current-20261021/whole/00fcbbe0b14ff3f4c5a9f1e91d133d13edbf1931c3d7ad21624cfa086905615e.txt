# producer-17b-compile-attempt-20261016 README

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Compiler source `17b12c909916ce8671b43f208b59f3432144709a` did not qualify. The first three launch attempts were refused before a controller or Cargo command started: two for memory headroom, one for internal disk headroom. Their original records are retained independently.

Retry3 passed unchanged resource admission. Its first library-test compilation failed with three test-module import/privacy errors and one unused-import warning. No test body ran. The production-library and workspace commands were not executed; no pin capture or Clippy command ran in this attempt. The actual Cargo process group settled empty, with no resource refusal or observation error.

All raw Cargo metadata, build/result/progress records, source registration, manifest, contract, controller sources and memory/guardian observations are retained byte-for-byte. `retained-files.json` binds their original paths and SHA256 values. This is failed-build evidence, not a current-head PASS or an efficiency measurement. The identified source defects are being corrected together with the next implementation batch.
