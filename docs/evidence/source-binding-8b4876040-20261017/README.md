# source-binding-8b4876040-20261017 README

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only first attempt: the two endpoint binders completed; the common refresh refused missing CLI arguments before its body. The wrapper now consumes its own `--repo` argument before forwarding to the unchanged refresh parser, and the documented invocation includes the original apply/head/memoization flags. No Rust compilation or test body ran. Original four programs and their exact finite transformations remain unchanged.
