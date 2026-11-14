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

The call census accounts for 52,855 of the 53,783 additional retained forecast bytes per module. The shared local callee accounting supplies the other 928 bytes, separately from the 928-byte receiving allowance applied after the estimate. The fixture contains no authored calls or imports. The census still visits its 291 expressions to establish retention eligibility. These figures are forecast terms. The separate allocator observation above covers requested heap layouts.


### Call census components

On Rust 1.97.1 for macOS arm64, the actual tuple in `Names::insert` adds 432 bytes per insertion to maintain the packed byte-length filter. The current binding scan calls it once for the `let` and once for each of the 96 assignments, although this fixture has no call lookup that reads the filter. The saved scan still pays its original insertion charge. The retention visit adds a 32-byte counter tuple, two classifier booleans, and one retention predicate per expression. These two components explain 52,089 of the 52,855 additional census bytes per module.

| Additional component | Reached count | Bytes per module |
| --- | ---: | ---: |
| Name-filter updates | 97 | 41,904 |
| Retention classifier, predicate and counter | 291 | 10,185 |
| Remaining census difference | | 766 |

The all-features library test image at `98b1aee91` passed seven of the eight retained-scratch tests, including generic and imported-default fallback, exact/short/zero, and real CFG point counts. The four-module reduction assertion was the failure. The same image passed the existing name-filter equivalence, direct-cost, and whole mode-six tests. On the 64-call fixture, the filter reduced each full and retained forecast by 524,352 bytes in both the local and imported variants, with identical requested heap layouts and complete Drop. On the direct fixtures, its charge increased by 6,624 bytes for the empty table and by 8,720 bytes for hits. It fell by 9,312 bytes for negative lookups.

Building the filter only when a lookup needs it is a candidate for the binding-heavy, call-free case. If the current fixture omitted all 41,904 bytes per module, the later peak would be 6,405,434 bytes before any replacement cost, leaving 13,070 bytes below the predecessor across four modules. That arithmetic only defines the available margin. A replacement must execute the original bounded name scan, keep insertion capacity and refusal order, and pay its own setup, controls and lookup before any forecast change is accepted.


### Lazy name-filter candidate

An isolated default-feature Rust 1.97.1 library build based on `6f1db7ec4` passed all eight retained-scratch tests. It maintains the original bounded name slots and builds the negative filter on the first lookup, using a stamped prefix so subsequent lookups index only newly appended names. The unchanged four-module test measured a forecast of **6,405,786 bytes**, below the complete saved producer's **6,418,504 bytes**. The later phase sets the peak with 6,383,948 bytes retained, 21,534 bytes of scratch, and a 304-byte directory. Warm whole-window allocation peaked at 824,395 bytes and returned to zero. The cold peak was 830,815 bytes and the 6,420-byte survivor matched the test's authenticated process prelude owner.

The call-heavy mode-six test also passed in its local and imported variants. Both forecast terms were 7,560 bytes lower than the prototype's eager-method comparison, which shares the prototype's expanded scratch layout and setup costs. Actual requested layouts, reservation receipts, and final Drop agreed between those arms.

The prototype has not been adopted. The wider call-carrier run passed 27 of 36 tests. The two existing allocation-versus-reservation checks still failed, the historical tariff and source checks need exact predecessor handling, and the Catalog, ShiftSim, normalizer, and CLI application gates remain unverified for this candidate. These measurements establish a reduction for the specific four-module forecast and preserve its original acceptance assertion.


The broader prototype run passed 365 of 390 projection tests. The baseline all-features image passed 381 of the 389 corresponding registered tests. Seven failures appeared in both runs, 18 appeared only in the prototype run, and the four-module test failed only in the baseline. The builds used different feature sets, so this comparison identifies candidates for follow-up. In the narrower call-carrier group, five tests that passed in the baseline failed in the prototype. Those failures included source authentication and independent invoice checks for the old name-table layout.

### Rebuilt prototype and shipping application baseline

The regenerated shared-table prototype passed 28 of 36 call-carrier tests. The previously failing closed-root source binding passed. Three failures also appeared in the 35-test all-features baseline: the two allocation-versus-reservation checks and the pre-898 comparison. The other five cover independent tariffs or exact source ancestry. This result leaves the shared-table prototype unqualified. The separate scoped candidate keeps the original name-table representation on the other routes, including the old controls.

The shipping CLI built from `138edb9a04f66c1509673c14589bd4bb87a13ffe` returned exit 0 and `status: verified` for each command below. Its SHA-256 is `33cd423478fd9b752b72daa8eebdd45d27d49fd640bc314656c5021e25e370ef`. All three authored application directories match that source revision byte for byte.

| Manifest passed to `semaprax check <manifest> --json` | Reported revision (SHA-256) |
| --- | --- |
| `examples/catalog-scoped-record-successor/semaprax.toml` | `59f773a25c160e58cfb19f7afe0b9a1093124068fb8889eefe4433f2b5cad422` |
| `examples/shiftsim-typed-record-successor/semaprax.toml` | `59de9eaf9bdf3d945b2eb10c64eb4b1b4a9e1b11204c3c74b1703681c463ec0a` |
| `examples/catalog-normalizer-project/semaprax.toml` | `1c76cc13569dac44ac364bf5840962e35792deedf886227a0e51339c00758cbb` |

These are baseline checks at the shipping 128 MiB construction limit. The same CLI checked and built the unchanged selected-file LogLens fixture in a temporary directory. Its native binary passed all 33 historical checks and 16 SPEC boundary checks via the checked-in wrapper, and the wrapper suite passed all 207 checks. The qualification report SHA-256 is `e154633d933429082fa5d9685c8aaec091e15efd26636f795c6e7569542f3796` and the native binary SHA-256 is `30bb2619a0e138af6ffec58e588664b02f43f50022476ab7c0801c4145b9d248`. The 64 MiB application and candidate gates remain pending.

### Scope of the four-module forecast

`uncached_output_layout_with_registry` calls `modular_pair::selected_estimates`.
The four scalar modules have empty import lists, so the selector reaches
`paired::estimates` and its `count_joint` receiver. The scoped prototype
changes `count_modular`, which is the separate route for modules with imports.
A joint receiver change is required to reproduce the earlier shared-table
reduction on the unchanged four-module control. The joint directory also
serves other callers, so that change must retain its independent eager
controls and cover both the linear and indexed directory paths.

The modular-only build confirmed this routing on the original assertion. It
passed seven of eight retained-scratch tests and left the four-module forecast
at 6,573,050 bytes against the saved 6,418,504 bytes. The separate joint
prototype preserves the historical controls and has started its owning Rust
projection tests. Its source-pin regeneration and all 54 overlay tests passed.
