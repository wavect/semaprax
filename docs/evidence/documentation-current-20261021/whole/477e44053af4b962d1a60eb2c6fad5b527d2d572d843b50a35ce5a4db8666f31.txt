# Explicit module-loader paths for retained Function alias tests

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Static registration found six authored847 alias tests were unreachable: #[path]-included typed.rs searched its sibling alias.rs, while the actual producer is typed/alias.rs. The latter also needs explicit alias/tests.rs because it is now path-included. This successor adds exactly those two path attributes. All constructor/test/assertion bodies remain byte exact. No new test body; no Rust build or runtime execution occurred. This is a static registration failure caught before starting the next build, not an observed new Rust compiler diagnostic.

The three complete current-to-340 inputs precede both independent final assembly fronts at combined_ready_input. All original runtime tables remain unchanged. The new current-to-340.json is itself protected runtime input: future composition must prepend an authenticated complete physical successor, not update its old input domain. verify.py checks the exact two path edits and replays the unchanged full previous assembly proof in its named340 view. Actual static test registration and executable qualification belong to the parent consolidated run.
