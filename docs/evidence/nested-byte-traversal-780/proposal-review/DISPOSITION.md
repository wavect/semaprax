# Raw proposal blockers and approved bounded response

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The single 9,939-byte tool-disabled proposal review exited0 in40.542531666s,
peak280,920,064 bytes (limit805,306,368), no resource refusal. Raw result,
verdict, payload and receipt retained unchanged. Provider list USD0.064162;
actual billing unknown. No compiler or runtime execution. Four SPEC blockers
were raised; this record does not relabel the raw verdict as approval.

Parent reviewed the following explicit narrower decisions and authorized a
bounded private implementation, with issue780 remaining open for residuals.

1. Preserve existing early mixed/non-byte rejection, including first full
   diagnostics. The initial nested profile requires every nested named
   immutable Slice<u8> source to be available in the OUTERMOST byte traversal's
   entry scope. A view constructed/aliased inside that body's lexical scope is
   still refused at the original nested-for boundary. This restriction lets
   an outer preflight classify all source-for forms from ordinary established
   binding types before evaluating any body prefix. Fresh byte item names
   cannot shadow entry bindings; mixed/unknown/mutable/computed/row/own forms
   keep the old rejection. Inner checking independently validates its current
   typed binding too. No text-only classification grants admission.
2. Actual paid compiler invocations use their existing caller authority for
   any census acquisition and reuse one invocation-local borrowed census.
   Standalone no-owner semantic APIs retain complete checked replay through
   the ordinary legacy allocation policy, explicitly unqualified/unpaid.
   They acquire no invented payer, allowance or persistent authority. Repeated
   standalone calls may repeat validation; no linear-total/performance claim
   is made for arbitrary client call sequences. Hot compiler callers must not
   rebuild full ancestor subtrees for each nested wrapper.
3. A census consumer checks all exact borrowed bindings: the same Program /
   ReplayContext, DeclarationIndex, ResolvedFunction root reference and
   FunctionExecutionId, and exact region expression reference plus its full
   locally reconstructed shape. An ID/pointer alone or stale cached flag is
   insufficient. A proof is never serialized. Borrowed lifetimes prevent
   mutation while the census exists; foreign function/closure/root requests
   refuse rather than substitute a lookup. No self-referential owner struct:
   construct the census outside and pass a borrowed reference/lifetime.
4. Track source-FOR ancestry independently of nearest generic loop target.
   Byte→while→byte can compose only under exclusively byte-for ancestry.
   Vec/row/own→while→byte and byte→while→Vec/row/own stay refused. Bare while
   does not gain a new nested-for admission merely because no for is active.
   Closure/function boundaries clear traversal admission and target context;
   evaluated captures retain current checking, separately compiled bodies do
   not inherit outer receiver authorization. Existing view-capture refusal
   remains. Nearest lexical break/continue target is still independently checked.

Additional review requirements retained for implementation/tests: actual
iterative lowering frame scheduling ("recursively" in the original proposal
means compositional source paths, not a new Rust recursive builder); exact
existing depth refusal and deepest admitted backend cases; full old v1 scanner
and source archives; stable SPX-T209 shadowing; counted same-root loans and
physical node/Vec/result/Drop payment; full same-function cross-level swapped
receiver/index/provenance hostile controls; current cache semantic admission
binding including pre-change refused outcomes if that cache persists them.
No automatic cache/wire compatibility assumption or schema exemption is granted.

The original PROPOSAL.md remains the exact reviewed text; these decisions amend
its broader inside-body view admission and unspecified ownership rules.
Implementation must not claim completeness before actual checks.
