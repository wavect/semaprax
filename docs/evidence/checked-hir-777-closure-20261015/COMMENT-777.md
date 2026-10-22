# checked-hir-777-closure-20261015 COMMENT-777

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Implemented and qualified the immutable checked-HIR owner, including sparse local selections and final owning publication. Cold insertion moves HIR into the checked owner; warm lookup and core retention borrow its exact backing after full current replay. Public outputs still materialize at their owning boundary. Forecasts, limits, loan residuals and canonical cleanup order remain unchanged.

Evidence is deliberately split by source subject: seven direct retention/frontier/publication controls passed at `033caf93`, the complete cold/warm Project lifecycle passed at `7a87b331`, and the exact body-pointer/source-guard/forged-stub/drop-order owner now passed using the existing source-bound `d8fadfb7` lib artifact. The bounded ownership cut confirms the incremental/cache modules and all direct test/fixture inputs remain byte-identical from d8 to e93; the separate #847 appended uncached selector helper and projected AST representation are disclosed rather than called whole-compiler equivalence. Historical persisted RequestViews, omitted-private-layout restoration, nonempty-loan replay, cache/source drift and full graph/Image controls retain their original actual PASS receipts and dates.

The actual `7a87` lifecycle fixture reported requested allocation / peak-live bytes of 13,297,224 / 300,435 cold and 13,064,758 / 239,428 warm; both ended with live=0, freed=requested, invalid=false. Those scopes include their in-scope parity/report work and are fixture observations, not an application or live-agent saving claim. Original application failures remain disclosed in the published 145-owner round (120 PASS / 25 FAIL); no ceiling recovery or token advantage is claimed.

Two newly rerun #796 attribution owners still FAIL their separate warm identity-acquisition observer predicate (6→9); their graph/Image assertions were not reached. That observer now includes the real cached-prelude report acquisition, and #796 remains OPEN for its own distinction and rerun. It does not invalidate this ticket's passed full lifecycle or immutable body-ownership controls.

Supplement source: `d8fadfb7cd5ece54504cda542575e5a154d3657c`; executable SHA256 `17d553295d19f4249bd42812ac7c004560f48e7a77a08ffd3d5efb7e12393c3a`; supplemental results SHA256 `538b142a26110a0d7a3f725a23aa3333390c2b3ca4302f6e2349b0143a74fab0`. Prior complete lifecycle log SHA256 `03206851b9584d6444a7770c8a9d9386bc0311f2c24053441478bab48ef78a4e`.
