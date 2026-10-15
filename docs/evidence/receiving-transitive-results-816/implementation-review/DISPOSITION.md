# DISPOSITION (implementation-review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Review completed once55.6822s on concrete6290 place helper, Sonnet5.5, provider listUSD0.082446, actualbillingunknown. No build/runtime or approval of later revisions claimed. It found no definite semantic/typing blocker; full raw text/receipt retained.

Actual by_storage value is Vec<LivenessFlagId>; projections are DeclarationId with full exact identity equality and as_str. Both are verified in owning production declarations. reserve_bytes is cumulative with no refunds; reserve_vec prepays complete replacement capacity while old backing stays live, checks exact capacity before push and never refunds. The complete observer includes every output and Drop.

Its stack-control question is addressed with explicit full sized iterator/zip/option/reference/scalar controls plus new function/budget argument transport. No assertion that those stack values are exempt or that old headers provide credit. Receiving/result Vec/Result controls remain fully prepaid. No final ABI proof is claimed.

The lower-level leaf helper now itself routes LegacyUnobserved through the complete original under, preserving the old nonpaying branch rather than depending only on the facade. Positive growth keeps the existing strict-capacity failure policy, no new allocator guarantee. Inherited map lookup and Diagnostic backing are not claimed paid by these vector fees. Differential malformed/projection tests, isolated actual backing work/byte exact-short-zero/sticky and whole original receiver Drop controls are authored, awaiting root qualification.
