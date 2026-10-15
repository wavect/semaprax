# Projection explicit imports correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Actual5b56 workspace owner68 failed because complete joined projection includes borrowed_imports/inline.rs with a wildcard parent import. Replace that single import with an explicit list of its actual referenced parent helpers, AST carriers and BTreeMap. Function body is byte-identical; all source children and the boundary assertion remain. Current constructor childSHA refreshed; historical review payload and original archives preserved. No Cargo or runtime execution by this packet.
