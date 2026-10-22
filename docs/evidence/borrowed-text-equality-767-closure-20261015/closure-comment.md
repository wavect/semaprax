# borrowed-text-equality-767-closure-20261015 closure-comment

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Implemented and qualified exact borrowed `str` Eq/Ne on the owning checked profile. [Evidence archive](https://github.com/wavect/semaprax/tree/wavect/v0-10-0/docs/evidence/borrowed-text-equality-767-closure-20261015).

All five required owning bodies have actual PASS evidence: source/canonical graph and hostile type/root/loan controls; interpreter plus native O0/O2 exact-byte, allocation and late-failure cases; aggregate Wasm tagged-owner/failure/reentry cases; public text distinct-address, length, NUL/UTF-8, invalid second-root, exact/over-limit and reentry cases; and the independent owned/borrowed Vec-slot no-copy receiver. All four language owners now have filtered PASS runs on the authenticated successful6548 artifact with explicit clang/Node availability, no Cargo and confirmed-empty process groups. Aggregate Wasm was rerun specifically because its older receipt lacked backend-availability provenance. The Vec-slot owner has no silent backend skip: it requires clang O0/O2 and Node to execute successfully. The first public-text attempt was resource-admission refused before execution; that record remains intact alongside its later successful run.

The nine complete dedicated receiver/test/JS/Vec-slot sources and interpreter/native receiving kernels match the audited current source. Five shared caller/root files have separately recorded full differences; this is not a claim that the whole compiler is unchanged or current-head qualified. Exact byte equality preserves rooted loans, left-to-right evaluation and normalized failures, and adds no Str ordering, owner clone or ABI widening.

Agent quick-reference guidance and CLI authoring help are present. The implementation replaced the two live Shift example length/prefix workarounds with direct inequality; its feature commit did not change the immutable application benchmark tree. Later patient-ID changes belong to the separate #786 work and are not attributed to this feature. No benchmark source, resource limit or oracle was reduced for acceptance.

This closes the scoped equality receiver feature after publication. It does not claim positive complete-application cap fit, universal current/future-MAIN qualification, a performance multiplier, or measured agent/token advantage.
