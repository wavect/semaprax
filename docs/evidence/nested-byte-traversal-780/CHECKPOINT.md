# Private source admission checkpoint — not integration ready

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Basis a2311ae7. Existing verifier frames/scopes supply byte-only ancestry and
outer-entry typed view lookups; no new verifier/frame field or owned snapshot.
Non-byte/mixed/computed/mutable/unavailable/inside-body sources retain their
old early rejection. Closure boundaries clear admission ancestry. Source freeze
acquire/release remains unchanged and each nested loop has a distinct old ID.

Complete original loop-admission implementation is archived and independently
compiled in before_nested, with only exact method-name/visibility adaptation.
Three source-only controls are authored: positives/canonical round-trip, complete
old diagnostics on closed routes, unchanged nonnested outcomes/test-scope unwind.
No Cargo/test execution.

This checkpoint is INCOMPLETE: HIR still has the original nested-receiver
refusal; contextual census, receiver consumers, source profile spec/help,
physical/backend/cache controls and full joined source custody remain pending.
Do not integrate or claim #780 implemented from this checkpoint.

## Unwired HIR building blocks (not a validated route)

The private nested/shape.rs child reconstructs the complete original local
receiver shape without granting a body/provenance verdict. The original public
region() remains unchanged and strict. nested/storage.rs borrows the exact
ClassifierLedger, prepays actual Vec/Result/backing/initializer layouts, retains
its first typed refusal, rejects foreign/restored use and never grows census
vectors. Its no-ACTIVE policy is explicitly unobserved, not paid. Copy-only row
insertion prevents recursive/unpaid Drop of a rejected item. nested/intervals.rs
implements private ID/interval prefix maxima so the inner same-root interval
ending cannot release an outer freeze. None of these new HIR children is yet
registered or used as admission authority. The producer/binding/consumer joins
and actual controls still must be completed before integration.
