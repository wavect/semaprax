# closure-audit issue-838-closure-draft

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Ready for parent verification and archive publication before posting.

Implemented directly prepaid ordered numeric closure/closed storage and authenticated borrowed singleton views. **All 10 direct owning gates PASS**, preserving complete independent old receivers, singleton epsilon sharing, work/numeric/order/overflow behavior, exact growth/refusal/Drop and full source histories. Relevant direct authority/source dependencies also PASS.

Actual old/current (reservation; allocated; peak bytes):

| Fixture | Old | Current |
|---|---|---|
| Singleton | 10006;3773;2669 | 9654;2781;2061 |
| Mixed | 11206;4021;2917 | 11598;2997;2213 |
| Weighted overflow | 3390;1269;1221 | 4070;949;901 |
| Cycle diagnostic | 4310;1977;1757 | 5014;1609;1389 |

All finish live 0, freed=allocated, invalid=false. Reservation increases in three cases are retained; no universal reduction is asserted. Full measurement log SHA256 `46429591cf0721ef151f5007a80c3175597448ebe096e07828c37ee09849168f`.

The unchanged retry/normalizer owners both ran and FAIL. Retry stops at its historical whole-source equality guard before the production retry predicate; no retry recovery or phase measurement is claimed. Normalizer reaches the 67108864-byte live cap and returns G171 (first request 4520, remaining 210, HIR final validation). This issue requires execution and disclosure, not application cap PASS. Both native #808 and all five project #808 owners also ran and FAIL G171 at the unchanged cap (request 3992; native remaining 1699, project remaining 3878 or 3142). The library #808 diagnostic witness also FAILS with an independent factored-replay reservation panic; its preserved trace reports CleanupReplay 12288 with 2771 remaining. This is aggregate final-HIR validation including the test-only oracle, not cleanup-only spend or an identified production inner operation. #808 remains unresolved.

Execution source: `d4cab900edac07fdcc4de0cdcb1bf3daafbef5ef`. Five focused build roles PASS; runtime **124 PASS / 18 FAIL out of 142**, one genuine body per owner, unchanged source, no resource refusal and settled process groups. Runtime receipt SHA256 `39cac179bfa8cced54ce69f095f541b1680358cb9566026a41d81bbb9a30fd7a`. Evidence: [complete receipts and failure disclosure](https://github.com/wavect/semaprax/tree/wavect/v0-10-0/docs/evidence/qualification-d4cab-142-20261015). All 18 failures and earlier failed/not-retested outcomes remain disclosed. Core production lib/bin Clippy PASS (60.725s; receipt SHA256 `7edb38f1795af61dba676d0a86ffb106153ba37e09e4e6f1d0ec7904ee25b344`). Full-CLI production Clippy PASS (receipt SHA256 `9392cca232c032b97a1eb5a38ffbccfe13bf2f9b4190b80d6daa8fa61e88c1bb`). No pin was moved by these five changes. The separate producer-pin capture remains an actual outcome to disclose in the archive, not an inferred producer success.

No application cap recovery, compiler RSS or matched-agent token/cost gain is claimed.
