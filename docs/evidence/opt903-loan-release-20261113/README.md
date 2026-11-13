# OPT #903 paid loan reachability release

The focused paid-plan observer compared the complete saved planner against the current retained planner using 1, 3, and 17 overlapping loan uses. It used a library test build from `0a68f5420` with Rust 1.97.1 on macOS arm64. Each fixture ran with both its ordinary attached plan and a deliberately modified attached plan. The observed allocation tuples matched across those two variants.

| Loan uses | Requested bytes, both planners | Cumulative reservation, both planners | Peak live requested bytes, old → current |
| ---: | ---: | ---: | ---: |
| 1 | 5,960 | 22,369 | 4,234 → 4,122 |
| 3 | 13,628 | 52,997 | 8,946 → 8,738 |
| 17 | 85,272 | 285,361 | 51,034 → 50,154 |

Each planner freed all tracked backing on return. The peak reductions equal the released reachability row backing of 112, 208, and 880 bytes. The old and current receivers agreed on plans, exact work refusals, late overlap, and mixed work and physical failure order. The associated source audit authenticates the whole current retained planner and reverses its later canonical-output constructor before checking the original early-release edits.

The mode-six workspace forecast remains a separate open gate. On the four-module scalar fixture, the current forecast was 6,573,050 bytes and the independent predecessor forecast was 6,418,504 bytes. The early release measurements cover actual planner backing and do not establish a lower workspace forecast or application admission.

## Four-module public resolve and validation

The forecast assertion stops the owning test before its allocator checks. A standalone probe ran the same four authored modules through public HIR resolve, retained all four outputs during validation, and observed their complete Drop. The probe linked the production library built from `0a68f5420` with the [Project reporting cap patch](../opt785-application-preparation-20261113/current-builder-limit.patch). The probe calls no Project reporting code. Source parsing preceded observation.

| Run | Live after all four resolves | Live after validation | Peak through Drop | Cumulative requested bytes | Live after Drop |
| --- | ---: | ---: | ---: | ---: | ---: |
| Cold process | 734,544 | 734,544 | 830,815 | 27,314,222 | 6,420 |
| Warm process | 728,124 | 728,124 | 824,395 | 27,307,732 | 0 |

In each run, validation allocated and released additional backing with all four outputs held. The overall high-water mark had already occurred during resolution. The cold residual is reported as observed. Its size matches the original fixture's prelude expectation, while this standalone probe does not classify individual retained pointers. These are public HIR measurements. The mode-six workspace constructor and its separate forecast comparison still require their owning checks.
