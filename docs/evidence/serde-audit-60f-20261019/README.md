# Bind the parser allocation audit to the already locked dependency

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

The manifest and Cargo.lock already select serde_json 1.0.154. The native builder allocation audit still expected 1.0.151 and its old package checksum, causing the original test to fail before exercising its near-limit escaped-string payload.

The audit now pins the actual 1.0.154 version and registry checksum. Its complete near-limit string construction, recursive owned-string capacity helper, all assertions, and every descriptor/scratch limit remain unchanged. The owning capacity comment records the reviewed version. No dependency, production calculation, or runtime quota changes.

Both cached parser source versions were authenticated against their original registry archive SHA-256 checksums. The full deserializer source changes only an existing negative-zero numeric-buffer condition. Both complete parse_str_bytes implementations, escape/unicode/codepoint routines, and the complete owned Value deserializer are byte-identical for the allocation behavior this bound owns. The current parser archive and all inspected source bytes agree exactly.

A focused Rust component invokes the actual complete audit function with the unchanged near-limit body, the actual recursive capacity helper, and the already compiled locked parser. Clippy with warnings denied and the assertion body pass. A larger libtest harness was stopped by the local memory cap and is not counted as a pass; the successful smaller harness removes only libtest registration and invokes the same complete function directly. This proves the focused source component, not the full native builder or current-head CI suite.

All files and outputs remain on the external SSD. This task uses a tighter 160 MiB process-tree cap, 35% minimum initial available memory, and a 30% stop threshold; the original 200 MiB guard file is unchanged. Source-role publication retains all original classes, roles, 342 owner rows, source views, reader caps and hostile controls. Completed historical writers are not rerun.
