# OPT #903 paid loan reachability release

Status: Focused local planner observations. The workspace forecast gate remains open.

Audience: compiler contributors and performance reviewers.

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

## Mode-six forecast reconciliation

An instrumented library test at `59f58e375` printed the per-module results before evaluating the original strict reduction assertion. The forecast implementation and saved producers are identical in the current checkout. All four modules produced the same row.

| Forecast term | Saved producer | Current producer |
| --- | ---: | ---: |
| Full estimate before new receiving allowance | 1,603,611 | 1,657,122 |
| Retained estimate before scratch adjustment and new receiving allowance | 1,604,626 | 1,658,409 |
| Reachability scratch removed from each retained module | 0 | 21,534 |
| Receiving allowance added per module | 0 | 928 |
| Final retained term per module | 1,604,626 | 1,637,803 |

The current census covered one function and 291 expressions per module, at 74 bytes of scratch per expression. The resulting construction bound is 6,571,459 bytes, using three retained modules and one full estimate with its receiving allowance. The later validation bound is 6,573,050 bytes, using all four retained modules, one 21,534-byte scratch allowance, and a 304-byte directory term. Later validation determines the peak. The saved peak is 6,418,504 bytes and is determined by four retained modules.

The observed 154,546-byte increase reconciles to four 53,783-byte increases in the retained estimate, four 928-byte receiving allowances, four 21,534-byte scratch removals, and 21,838 bytes for later validation. This comparison keeps every current estimate and reservation intact. The original strict reduction assertion still fails.

A second instrumented library build at `59f58e375`, using the same four-module fixture, split the first module's estimate into the following components. The remaining modules produced identical totals. The four current and saved producer files used for this comparison are byte-identical at `138edb9a0`.

| Component in bytes | Saved full | Current full | Saved retained | Current retained |
| --- | ---: | ---: | ---: | ---: |
| Fixed HIR expansion | 1,301,390 | 1,301,390 | 1,301,390 | 1,301,390 |
| Call census | 77,238 | 130,093 | 77,238 | 130,093 |
| Shared local callee accounting | 3,231 | 3,887 | 4,246 | 5,174 |
| Prelude HIR | 72,480 | 72,480 | 72,480 | 72,480 |
| Identity carriers | 11,640 | 11,640 | 11,640 | 11,640 |
| Identity occurrences | 137,632 | 137,632 | 137,632 | 137,632 |

The call census accounts for 52,855 of the 53,783 additional retained forecast bytes per module. The shared local callee accounting supplies the other 928 bytes, separately from the 928-byte receiving allowance applied after the estimate. The fixture contains no authored calls or imports. The census still visits its 291 expressions to establish retention eligibility. The next comparison must price the classifier and its counter by actual role and lifetime against the complete saved producer. These figures are forecast terms. The separate allocator observation above covers requested heap layouts.
