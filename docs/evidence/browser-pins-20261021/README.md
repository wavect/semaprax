# Linux browser fixture graph pins

Status: prior-head source-only graph capture evidence.
Audience: CI and source-custody maintainers.

The successful graph-capture step of CI run 39540011351, job 119026036286, records the actual Rust 1.88 Linux compiler at 85f70a7cb7599a228b6800358ea7d1d386df7785. The raw log, API metadata, compiler provenance and both complete graph records are retained here. Each capture envelope byte length and SHA256 is verified, as are its exact head and the successful capture step.

Compared with the complete prior records in ../ci-contracts-49ded-20261020, only budget.used_builder_bytes and graph_digest change. The builder bill falls from 3,760,906 to 3,684,292 bytes. Both graph digests are independently recomputed using the production domain, little-endian u64 payload length and exact wire bytes. Replacing only these two fields reconstructs each captured graph byte-for-byte. Project and workspace revisions, declarations, edges, order, capabilities and all other fields remain exact.

The two fixture pins bind those observed graphs. Every runtime, artifact, rename and hostile assertion remains. This is prior-head capture evidence; it does not claim current-head compilation, Chromium execution, or a complete CI pass.
