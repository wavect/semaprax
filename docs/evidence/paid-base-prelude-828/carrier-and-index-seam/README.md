# Concrete carrier and fixed-base Index seam correction

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Source baseline 18054f3e3. Review is manual source audit only: the earlier Claude request timed out with no verdict. No repeat request or Cargo/build/runtime ran.

The original bundle acquisition precharged the Arc allocation but omitted simultaneously live initializer and receiving carriers. The corrected first request is the checked sum of actual Contents, BasePreludeBundle and Result<BasePreludeBundle,Failure<E>> layouts; final Arc backing remains a separate actual-layout request. Exact carrier-only / one-short / zero controls require refusal before any producer allocation and preserve spent debits. Existing full-acquisition exact/short/work/each-site controls now cover this first request, and observer scopes additionally finish last retained-lease Drop and compare real highwater to paid bytes.

This follows the existing constructor contract's header/receiving-owner obligations (Selected Replay Recovering Constructor v1, Session header and affine paid setup) and Declaration Identity Backing v1's distinction between inline carrier and real backing. It introduces no observed-gap constant, quota increase, inferred peak saving, refund or diagnostic materialization.

The #827 owner explicitly requested raw bounded fixed-base borrowed queries for preexisting infallible Index APIs. Those source APIs cannot gain a new work authority. The new borrowed seam provides full slices and every base lookup with no allocation, while the separate paid overlay remains available to callers with an existing Work payer. The complete independently saved original index controls compare all facts and physical zero-allocation query behavior. No work-payment claim attaches to the infallible queries, matching Declaration Identity Backing v1's exhausted-budget pure-query rule.

The complete pre-correction module, controls and contract are retained with exact whole-owner source inverses. The new borrowed child relocates no existing audited body and changes no root registration. Production integration, nominal foundation rollback and public applications remain separate pending qualification; P stays unchanged.
