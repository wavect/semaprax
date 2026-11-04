# Complete Change and Structural Change wire bindings

The original public API/CLI assertions retain their complete current wire comparisons, parsed field checks, replay verification, real application, locking, source inventory and hostile-input checks. Their expected whole-document hashes now distinguish the two measured native graph layouts, following the existing public graph test convention. No production implementation, fixture, admission limit or report payload is normalized.

All 28 complete report bodies are preserved. macOS bodies come from the original fixtures at e85c1a4941d69dff460711fe1c2e45acbe68ca6d. Portable Structural Change bodies and the portable Change preview, verification and application bodies were captured from Linux CI job 126103071589. The remaining portable Change context, impact, review and evidence bodies are reconstructed using the actual cross-host graph bindings and dependent report digests. Every reconstructed full body independently matches its domain-separated digest and exact byte length in the actual Linux verification and application receipts.

`verify.py` authenticates all preserved raw hashes and canonical bytes, all 20 report digest anchors and receipt lengths. It compares every field across hosts: only graph-digest references and their dependent report-digest references differ. All semantic and accounting fields must remain equal. The manifest preserves all eleven predecessor expectations for audit; it makes no claim that unavailable historical report bytes were recaptured.

Operations diagnostic receipts are emitted before its existing assertions so CI can reveal its complete remaining native report bindings. They grant no verdict or execution authority. Current-head CI remains authoritative, including the portable layout expectations.

Run: `python3 docs/evidence/change-wire-bindings-20261104/verify.py`.
