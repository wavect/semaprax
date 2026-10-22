# Actual hosted Project graph records after retained-tail changes

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The successful capture step in current compiler-head 7ce0a24f3a job 117647268942 produced both complete 9895-byte graphs, exact base64 envelope pins and compiler provenance. The binary SHA and Rust 1.88.0 details are retained. Both complete prior/current records and their entire recursive comparison are preserved. Every field, vector position and wire byte is identical except graph_digest and budget.used_builder_bytes. Both digests are independently recomputed from the exact wire without its self digest, using the compiler's domain, u64 little-endian payload length and SHA-256. Project/workspace revisions, all fixture checks, artifacts and host policy stay exact.

The actual calculator builder bill rises from 12252909 to 12316396 bytes, a 63487-byte increase. This is observed current compiler behavior, not an application savings claim. The retained-tail change can fund views in sort pairs that order on earlier segments before reading a tail. Full native/STD-08 outcomes are required before deciding its application tradeoffs. The fixture pin update binds the actual current graph record without masking any other semantic or wire difference.

This capture is not a Chromium runtime pass: exact pin mismatch stopped the worker suite before execution, and HTTPS steps were skipped. The derived fixture/control commit must run the complete original browser suite, full CI and required native/kernel gates. No current-head binary or runtime authority is inferred from these prior compiler-head records.
