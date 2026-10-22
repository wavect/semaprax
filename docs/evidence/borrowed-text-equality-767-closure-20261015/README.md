# Borrowed text equality: #767 closure evidence

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Exact borrowed str Eq/Ne has actual acceptance evidence for all five owning bodies. Four supplemental language checks passed against the authenticated654846173bb90874779dfee44ab0832c03715e84 artifact: source/canonical graph/hostile modes, interpreter/native O0/O2/late failure, public text Wasm roots, and aggregate tagged-owner Wasm roots. Explicit clang and Node version probes succeeded on the actual PATH, preventing silent backend skips. All successful process groups were confirmed empty; no Cargo ran for these supplements.

The first public-text attempt was NOT executed because available memory was28%, below the unchanged30% runtime admission floor. Its refusal and failing controller assertion are retained in actual-three/. actual-public/ records the separate successful attempt. actual-aggregate/ addresses a concrete provenance gap in the earlier aggregate PASS: the earlier receipt did not record Node availability. It does not erase or relabel that old receipt.

The original independent owned/borrowed Vec-slot receiver also genuinely passed at6548. Its Node invocation is unconditional and unwraps the child result, so that result cannot be a missing-Node early-return PASS. historical/ preserves the complete original receipt and both relevant raw logs, including the earlier aggregate PASS. original-build/ preserves the successful original build receipt, Cargo metadata and raw log; all hashes were independently rechecked.

The nine complete dedicated source/test/JS files are byte-exact from6548 to the audited current source. The checked interpreter equality arm and native dispatch kernel remain exact. Five broader caller/root files differ; their full diffs and complete before/after hashes are disclosed in audit/. Parent independently rechecked the nine complete sources and five complete shared-file hashes/diffs after the848 merge atdf9718f64732e827d805ff1c8545f09ef5a8b7c7. The audit snapshots describe intermediate states where appropriate; actual receipts and this final disposition distinguish all four successful supplements from the retained refusal.

Guidance and live Shift example adoption are bound in the audit. Immutable benchmark originals were not changed by the equality feature; later patient-ID changes are separate786 work. Exact equality does not normalize Unicode, clone owners, admit Str ordering or widen the ABI. Source-bound feature acceptance is not whole-current compiler qualification, complete application budget recovery, complete funding closure, or measured agent/token advantage. Those broader requirements remain open.

SHA256SUMS.json authenticates every other file in this archive. No full quality gate or suite ran.
