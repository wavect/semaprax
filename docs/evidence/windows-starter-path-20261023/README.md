# Windows starter-input parent spelling

Status: focused local repair; actual Windows execution remains pending hosted CI.

Audience: compiler maintainers and CI reviewers.

The original held-regular-reader test failed on Windows main `295c73b48f00a75eadf1f859c98bd41845dd7b0a`: ordinary absolute temporary paths did not equal the verbatim spelling required by the platform's strict held-directory entry. The starter now supplies lexical drive/UNC prefix and separator normalization to that entry. It does not resolve filesystem aliases, change the selected input spelling, or bypass the platform's canonical equality check. Final-component no-follow, regular-file bounds, named-file identity and original-parent identity rechecks remain mandatory.

Portable unit controls use the actual standard-library prefix/component values to exercise spelling construction. A native focused harness retains the complete current reader functions, original exact/short-bound test and actual failure type, links the genuine platform facade, and checks parent rebinding against real held handles. It contains no mock platform or compiler IR. This scope does not qualify Windows runtime behavior or the complete starter application; hosted Windows CI must establish those outcomes.

All work, temporary files, caches and build outputs remain on the external SSD. Device physical available RAM is monitored; process RSS is telemetry without a process memory cap. See the recorded commands, source hashes, test results and RAM observations.

The Rust 1.88 macOS job also reported a duplicate inherited formatting-trait import in the provider regression module. Its wildcard imports are replaced by explicit dependencies while the complete predecessor function remains unchanged. A real current cfg(test) compiler metadata lint strengthens only that target module with `forbid(unused_imports)`; registration-related allowances cannot mask its warnings. This records local Rust 1.97.1 lint evidence, with Rust 1.88 still requiring CI verification.

The module-path follow-up retains both actual `new_project.rs` registrations and relative child paths. The previous declaration reproduces `E0583` in the production and integration layouts on both Rust 1.88 and Rust 1.97. The explicit `application/held_input_path.rs` attribute passes the same seven real reader/prefix checks and warnings-as-errors lint in all four version/layout combinations (28 test executions). The earlier reader acquisition rebased the child path and therefore did not establish module discovery; these additional checks cover that gap. Actual Windows runtime qualification remains hosted.
