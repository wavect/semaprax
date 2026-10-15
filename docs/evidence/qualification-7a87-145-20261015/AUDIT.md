# Conditional closure audit: #833 and #835

Subject: `7a87b33115c0cd45b14cac64d7b37e5742503159` only.
Live issue bodies/comments are archived as `live-issue-{833,835}.json`.
The completed focused round ran 145 bodies: **120 PASS, 25 FAIL**. All five owning
build commands passed (unit first, production library second, then selected
harnesses); scoped core CLI/library and full CLI Clippy both passed with
`-D warnings`. No new execution occurred in this audit.

## Exact predicates

Both live issues require complete independent old/current receivers, all six
diagnostic fields/order and semantic bytes/work, actual new payments before
acquisition, whole-lifetime physical accounting, zero/one/multiple cases,
byte/work frontiers, malformed/duplicate/interleaved and relevant affine-owner
controls. Neither permits caps/refunds/exemptions/padding. Both explicitly say
“Run unchanged application owners and disclose failures”; neither requires
universal application success. Both require actual producer regeneration of
moved pins. Physical helper results do not prove agent/token or app-fit gains.

- **#833:** exact submitted owner list has 30 selectors, all executed once and
  **30/30 PASS**, with source binding unchanged. Its seven new controls include
  the complete source/old receiver gate; required original layout/capacity/work
  and cached-route controls also pass. The full per-selector/log-hash map is
  `closure-audit.json`.
- **#835:** the immutable 22-entry submitted list executed **22/22**. It has
  **20 PASS and 2 application FAIL**, not 22 PASS. All eight new direct
  comparator/cold-preflight owners and twelve other positive original/source/
  cache owners pass. The two failed entries are retry and normalizer application
  owners; their live-issue predicate is run/disclose. All old/current behavior,
  byte/work/sticky/floor, full-source and actual Drop controls passed.

## Actual physical evidence, including unfavorable cases

#833 owner79 logs total reservation and Metrics through Drop:

| Cold callers | Old/current reservation | Old/current allocations | Old/current allocated bytes | Old/current peak live |
|---|---:|---:|---:|---:|
| 0 | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| 1 | 34600 / 37870 | 14 / 15 | 34272 / 34408 | 34272 / 34408 |
| 4 | 137143 / 42049 | 41 / 33 | 50559 / 36391 | 35991 / 36215 |

All reallocation counts are zero; all freed bytes equal allocated bytes, final
live bytes zero, invalid flags false. One caller pays more. Four callers save
reservation/backing but peak rises 224 bytes. This is a fixture-specific result,
not a universal peak reduction or application recovery.

#835 isolated owner129, 64-comment case: reservation unchanged at4416;
allocations96→86, reallocations14→5, allocated22909→9370, peak17072→7408.
Whole cold preflight owner132, 32-comment case: reservation unchanged at1187762;
allocations2064→2051, reallocations251→240, allocated722681→718473,
peak181160→181160. Zero/one comment cases also reduce actual output allocations;
whole-cold peak is unchanged. Every case ends live0, freed==allocated,
invalidfalse. No reservation reduction or application cap recovery is claimed.

## Material failures retained

- Retry owner4 now reaches its actual retry work (old stale-source guard fixed),
  then G171 at HIR final validation: requested200, remaining143, floor0 under
  unchanged67108864 cap.
- Normalizer owner5 G171 at HIR final validation: requested4520, remaining2600,
  floor0 under unchanged cap.
- Starter ShiftSim owner1 G171 uncached synthetic AST peak reservation:
  requested406201, remaining386779. Selected starter owner2 G171 HIR final
  validation requested4520, remaining3125. These apps ran; not recovered.
- Strict #831 owner3 still fails its reservation inequality: width0 old10829 /
  current12518; allocations12/12, allocated1517/1453, peak1501/1453, live0.
  No waiver/baseline change; #831 stays open.
- Other actual failures, including #836 loop/source guards, cache foreign,
  SourceA, help and browser pin, are retained in the complete 25-failure map
  and original logs. No full suite/current whole-system green claim.

## Remaining closure predicates

1. Actual moved-pin capture plus strict admitted update/consumer proof remains
   **PENDING**. The archived producer receipt is not a complete pin chain.
   Do not close either issue on this packet alone until parent establishes
   unchanged pins or completes necessary actual producer→patch→consumer proof.
2. Parent must publish the exact receipt/controller archive and source binding;
   then decide closure against the live issue bodies. The source is already
   pushed; this external audit itself is not published.
3. Disclose all application/strict failures above. Do not describe 835 as22PASS,
   fullsuitePASS, app fit, token advantage, or an all-positive global source audit.

## Receipt integrity and publication preparation

`closure-audit.json` records full SHA256 for runtime/build/Clippy/manifests and
every exact owner log. Runtime receipt SHA:
`6e8f85c6d9d5742e7e4fea362d0b28f66c90b9ed7475f7d18afe3510bb15efe4`.
Build receipt SHA:
`bb75fb1129932443aa131300a5c10b7abe008bdae7229c2980cd26bed6269806`.
Core Clippy SHA:
`b878c57cbd5fa7c52b5c88396f129560f71da43ae5078947626df2b3a739ba4b`.
Full CLI Clippy SHA:
`b30ff161011e5605f67d360fb743e3a3943136bed799c3e189ad47902bff522b`.

`round-archive/` is a byte-exact snapshot of323 current-round files,6492723bytes:
all complete receipts/manifests/build/runtime/Clippy logs, telemetry, drivers and
controllers. It excludes runtime-tmp, __pycache__, executable artifacts and
unrelated duplicated old histories. Its manifest hashes every archived file.
`archive-publication.py --destination <NEW-PATH>` can reproduce this bounded
snapshot for parent publication; it runs no tests/Git/GitHub actions and refuses
to overwrite a destination. Existing round-relative and absolute source paths
remain original provenance; no receipt fields were rewritten.
