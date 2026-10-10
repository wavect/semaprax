# Project v32 Nested-Outcome Command v1

Status: Routing and carrier implementation are in progress; current-head execution qualification is pending.

Audience: Project profile authors, compiler contributors, and reviewers.

This document defines the additive table profile `language-command-io.nested-outcome.v1` and semantic schema `semaprax.project.v32`. The profile is a distinct successor to v31 so nested outcome results cannot widen the v31 collection-record route or older command profiles.

The command keeps the `argv-utf8+stdin-stream.v1` input, native64 target, existing stream and capability limits, and exactly the four existing adapter grants: `process.args.read`, `process.stderr.write`, `process.stdin.read`, and `process.stdout.write`. The entry and selected command remain independent explicit stable-ID roots with `fn() -> i64` ABIs.

The new closed outcome has one success case carrying one bounded, explicitly identified owned record payload and one error case carrying exactly `(i64, usize, i64)`. Record names, field names, and case order are authored; authority comes from the selected profile and checked carrier shape, never from names or declarations alone. The nested record carrier inherits the existing 64-level, 4096-field, and 256-owned-leaf limits. Existing vector element and capacity bounds continue to apply.

This route does not add recursive variants, dynamic-origin values, a general exception mechanism, or a new host capability. Project v31 and earlier remain frozen. The source changes and owning regressions do not establish current-head interpreter/native execution; that qualification remains pending.
