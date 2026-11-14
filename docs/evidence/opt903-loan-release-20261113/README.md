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

### Scoped joint forecast and historical 64 MiB reference

The joint-only candidate passed all eight retained workspace-scratch tests in
a default-feature Rust 1.97.1 library build from `07d83c1c4` plus the candidate.
The unchanged four-module fixture reproduced the **6,405,786-byte** forecast
against the **6,418,504-byte** saved forecast. Its warm requested-layout peak
was **824,395 bytes**, with zero live bytes after Drop. In a fresh process the
peak was **830,815 bytes** and the authenticated static prelude retained 6,420
bytes. The direct census control passed all six cases across the linear and
indexed directories and preserved roots, retained counts, complete callee
costs, and requested-layout tuples.

The wider call-carrier group passed 32 of 36 tests. Three failures already
occur in the baseline. The candidate adds one failure in the historical
closed-root invoice comparison: its old-to-current delta omits the candidate's
48-byte lazy-filter initialization. The existing comparison still asserts the
original invoice, and the candidate remains outside the shipping compiler.

A subsequent focused rebuild separates the immediate eager invoice from the
lazy successor's cold overhead. The unchanged historical equation passed
against the eager receiver. The new lazy control derives the layout and the
per-function reset charge and compares both against the live joint receiver.
Both lazy controls and all eight workspace-scratch owners passed. The wider
group passed 33 of 37 tests before refreshing the modified test file's
historical inverse. After the refresh, the complete affected Rust source
reader and inverse test passed in a standalone build. The other three
failures also occur in the baseline, including two whole-forecast allocation
checks and the pre-898 directory comparison.

A standalone allocator probe executed the unchanged local generic-instance
cost body with the same non-generic function fixtures as the closed-root test.
The measured table row is 48 bytes. Each of the full and retained estimates
requested and freed 48 bytes for one function and 3,072 bytes for 64 functions.
Both 64-function fixtures therefore allocate and free 6,144 bytes for empty
template tables across the two estimates. This is one measured component of
the baseline allocation mismatch. The probe isolates the local template
estimator. A paired probe starting with an empty vector preserved all four
zero result fields and made no allocation on any of these fixtures. It saves
3,072 requested bytes per estimate on the 64-function fixtures. The experiment
does not exercise actual generic templates or establish a funded growth
strategy for them.

A second isolated probe ran the call-identity discount on the call-free
scalar fixtures, using the same AST child order for their literal and empty
block shapes. Its declaration tree requested and freed 368 bytes per
estimate for one function and 3,408 bytes per estimate for 64 functions.
Together with the template table, this accounts for 832 bytes across both
estimates on the one-function fixture, matching its observed complete
allocation. The same two owners alone account for 12,960 requested bytes
across both estimates on the 64-function scalar fixture, above the 5,688-byte
explicit reservation reported on the smaller fixture. The 64-function
whole-estimate total and reservation still need to be recorded together.
The scalar fixtures exercise no call or binding lookup. On the 64-function
fixture with one local binding per function, the same identity discount
requested and freed 61,264 bytes per estimate. Its 393 allocations consist
of nine declaration-tree nodes and six allocations per function. The
local-binding tables and per-block bookkeeping account for 57,856 bytes per
estimate above the scalar fixture. Per function, the observed requests were
three 192-byte layouts, one 200-byte layout, and two 64-byte layouts. The
one-function binding fixture had the same six local requests, alongside its
368-byte declaration-tree node. Together with the empty template tables,
these two isolated owners request 128,672 bytes across the full and retained
estimates on the binding fixture. An allocation-free declaration lookup
alone would still leave the binding costs. The probe used the repository
AST child and scalar-initializer classifiers and no call or import route.

An isolated successor stored the first newly active binding in each Block
inline, retaining the existing Vec for later bindings and removing the
first and subsequent names in their original order. The 64-function
binding fixture kept the same identity discount and requested and freed
57,168 bytes per estimate, compared with 61,264 bytes in the original.
It omitted one 64-byte allocation per function. The call-free scalar
fixture was unchanged. A follow-up probe preserved the computed discounts
for one binding, multiple bindings, nested Blocks, duplicate spelling, and
an unproved String initializer. It removed a 64-byte request in the
single-binding and nested cases. The multi-binding case still needed the
same 64-byte Vec backing and showed no heap reduction. These probes do
not account for new inline storage and control roles in a complete payer.

A separate production profiling build from
`0a68f5420260999ef7e57a1a3641e41a69a95a4a` applied only the
[64 MiB Project reporting patch](../opt785-application-preparation-20261113/current-builder-limit.patch)
and ran the existing [Project check probe](../opt785-application-preparation-20261113/current-project-probe.rs.txt)
on the unchanged application manifests. This comparison establishes the
historical reference for a subsequent candidate run at the same cap.

| Application | Check at 64 MiB | Mode-one cumulative reservation |
| --- | --- | ---: |
| Catalog | Succeeded | 48,615,173 |
| ShiftSim | Live construction refused after the 66,704,189-byte static forecast fit | 67,104,702 |
| Catalog normalizer | Static admission refused at 67,122,386 bytes | No core attempt |
| LogLens | Succeeded | 21,430,497 |

ShiftSim's first sticky live refusal requested 8,576 bytes with 4,162 remaining.
The normalizer's static refusal combined 66,248,903 retained bytes and an
873,483-byte module forecast. LogLens completed all 159 profiled comparisons.
The bounded detail buffer retained 128 rows and reported incomplete detail.
The shipping application test image independently passed the unchanged Catalog
23-case and ShiftSim 15-case functional owners at its 128 MiB limit. The
same shipping test image passed all 13 Catalog normalizer owners, including
the interpreter/native/core-Wasm comparison and the frozen framing oracle.

The joint-only candidate was rebuilt with the same Rust profile, Project cap
patch, and probe. Its joint census patch SHA-256 is
`d666ed26e7d6f61b71f04bf8fb35d606db2ee007bdbb188fd61af3f2c0c7afe7`.
The linked probe SHA-256 is
`469b8853c41961b8ff3c269354fa27d21d301fd13bc9a4723ce0aa45bda01f6c`.
All four receipts agreed byte for byte after replacing the checkout path.
This includes each outcome, the full refusal diagnostic, the mode-one debit,
and every emitted comparison count.

The Project reporting wrapper selects a single live attempt at this
explicit cap. A static forecast that fits enters mode one, and the retry
gate admits only the default builder limit. Consequently the joint mode-six
reduction cannot address ShiftSim's measured live refusal at 64 MiB.
The normalizer reaches the mode-six static forecast and still exceeds the
cap by 13,522 bytes at its first refusing step. Earlier retained modules
account for 66,248,903 bytes at that step. That first-step gap does not
bound the reduction required for the remaining modules or later validation.
The combined joint and imported-module census candidate was checked through
the same Project entry point and cap. Its production patch SHA-256 is
`3218d9670e2700cc3b9b4f50ad10b1f22657c08b2648a06a67a51daf29fb9e9e`.
The linked probe SHA-256 is
`dd56d7ebe5516cdd911d020bd1f9218deb8b9c154d0acbb1f73a326e4101292e`.
The Catalog, ShiftSim, and LogLens receipts matched the joint-only receipts
byte for byte. The normalizer progressed one step further. Seven earlier
modules retained 66,869,729 bytes before the full 290,366-byte
`std.io.lines` estimate refused, at 67,160,095 bytes. That is 51,231 bytes
over the cap at the new first refusal. The normalizer again made no live
core attempt.

A separate profiling build captured the returned full and retained terms for
all twelve modules in the actual mode-six order before the original check
could refuse. Its application receipt was byte-identical to the uninstrumented
combined run after removing only the observer lines. Replaying the recorded
order and arithmetic reproduced these totals:

| Mode-six forecast component | Bytes |
| --- | ---: |
| Peak through the final module's retained publication | 74,701,378 |
| Later validation scratch | 10,582 |
| Later validation directory | 1,024 |
| Complete peak at later validation | 74,712,984 |
| Complete peak above the historical cap | 7,604,120 |

The largest retained rows were `src/app.spx` (25,398,534 bytes),
`src/record.spx` (11,794,188), `src/batch.spx` (9,611,847),
`std.data.json.dec` (9,030,737), and `src/tests.spx` (8,587,569).
These are static forecast values. The regular 64 MiB application route kept
its original refusal, and the observer executed no live construction for
this input. The log SHA-256 is
`7f11977fcd4f044b3486a2dff96d7c4a9b06da380b56a936ef84ec424e284523`.

The same cap and observer on the same source baseline, with only the two
census changes reversed, recorded a 74,938,824-byte peak. The combined
census lowered the complete forecast by 225,840 bytes, with identical
later scratch and directory terms. Nine modules reduced their retained
forecast and three increased it, by 48, 96 and 408 bytes respectively.
The matched baseline log SHA-256 is
`d4ca4df0fe85b1108fb002aa8e76200e8fa470cdd0762b6a1b818213873bfd0b`.
Scoped production Clippy with the workflow-profiling feature passed for
the joint-only census. The combined application run still refuses at
the historical cap.

The existing whole-normalizer library owner passed at the default 128 MiB
cap on the retained `6f1db7ec4` test image. All twelve authored and dependency
inputs matched the current files by length and SHA-256. The actual mode-one
attempt prebound 117,144,618 bytes and refused after consuming 134,217,649
bytes. Its first sticky debit requested 160 bytes with 79 remaining during
HIR loan-plan construction. Mode two prebound 81,369,005 bytes and completed
at 107,349,690 bytes. The first failed attempt's debit remained in the maximum
phase receipt. The owner reached no mode-six forecast at this cap. Log SHA-256
is `16318553222c1ae43c828b93aa7c19450cf90866d2ea57845b4b85a5311c3e51`.

### Hosted shipping-limit checks

The [hosted production reservation census](https://github.com/wavect/semaprax/actions/runs/43585128348)
used `main` at `1fe13fc7955d3335ee32742a12c4d0905117400b`, with Rust 1.97.1
on Linux x86-64. Production Clippy with workflow profiling and warnings denied
passed. All six unchanged manifests returned a successful Project check with
identical complete products before and after enabling observation. The
retained report's SHA-256 is
`df32af082ca187c969888152402b292f6706192a9286bad82f0094a881712706`.
Every recorded manifest and declared module source digest was independently
checked against the Git tree, together with both logs for each subject.

| Project | Successful mode | Cumulative reservation in the attempt (bytes) |
| --- | ---: | ---: |
| Int decimal | 1 | 19,652,251 |
| Catalog scoped record | 1 | 48,592,503 |
| ShiftSim typed record | 1 | 80,284,328 |
| Catalog normalizer | 2 | 107,348,032 |
| LogLens adapted matcher | 1 | 98,380,966 |
| LogLens selected file | 1 | 21,408,907 |

At the same 134,217,728-byte cap, the normalizer's first attempt refused a
160-byte reservation at `src/bounded_output/cfg_reservation.rs:116:8`, with
113 bytes remaining and zero reserved floor. It had consumed 134,217,615
bytes. Its complete second attempt succeeded with 26,869,696 bytes remaining.
The hosted run used the default cap and reached no mode-six attempt. The
unchanged 64 MiB comparison above still refuses. This run performed Project
checks; CLI49 and application execution require separate owning runs.

### Dense-call allocation and mode-six follow-up

The retained Rust 1.97.1 macOS arm64 test image at `6f1db7ec4` passed the
unchanged #898 dense 64-call public resolve, validation and Drop test. The
owning test and production source match the `1fe13fc79` checkout. The observed
requested-layout peak was **283,482 bytes** against a **553,732-byte** mode-five
retained-HIR forecast. The complete window requested and freed 13,345,964 bytes,
with zero live bytes after Drop. The log SHA-256 is
`5df04e963b282796b4bb1aa3e724a4668d380c769f7d00c75668d6a7a9f18496`.

Fourteen of the fifteen main call-carrier tests passed on that image. The
mode-six reuse test failed the assertion that its complete requested heap
bytes fit the explicit reservation. The separate closed-root whole-mode-six
owner failed the same assertion. The #903 cold public overlap test passed,
and its four-module mode-six owner reproduced the 6,573,050-byte forecast
against the saved 6,418,504 bytes. These measure different validation and
forecast routes. Both #898 and #903 still require their remaining owning
checks and application qualification.

The [hosted installed-compiler job](https://github.com/wavect/semaprax/actions/runs/43586136369)
then ran both unchanged native CLI49 references at `1fe13fc79` using the
Linux x86-64 bundle compiler without workflow profiling. The selected-file
and adapted matcher projects each passed the 33 historical checks and the
16 independent boundary checks through their native wrappers. The runner
kept the checkout and the compiler executable byte-identical through both
runs. The qualification receipt SHA-256 is
`eba31b8404d5ea0b3b2d1743670fbd2b02072d21d5919ee8ecb2cf0c096fd8c1`.
The selected-file and matcher qualification report digests are
`064916427b10cc5025fe0d58fc9c51fd0d2e2ac31e71e591ec44357e88334071` and
`90d8fbd3e9694b1e4ff5e9d9bf43309793a1cce28306f63d2b98c7c0fdf087aa`.
This used the shipping limit. A separate macOS arm64 rerun of the two
owning application tests at `6f1db7ec4` passed the Catalog 23 cases and the
ShiftSim 15 cases, using production and fixture sources unchanged in the
hosted checkout.

A focused allocation probe compiled the complete old and revised local
`generic_instance_source_cost` bodies with the existing parser. On both
64-function monomorphic fixtures, the old receiver requested and freed 3,072
bytes and the revised receiver requested none. Both returned zero for all
four cost fields. The revised receiver skips this table only when the module
has no local generic declarations. This probe covers the local estimator;
the mode-six call-identity and layout allocations still need their own
complete accounting.

A separate Rust 1.97.1 allocation probe delayed the call-identity declaration
index until the first eligible ordinary-callee lookup. It reused the current
AST parser and the complete eager declaration-directory builder. The eager
and deferred versions agreed on the scalar and binding fixtures, ordinary
local and unknown calls, an imported call, an alias collision, a mismatched
provider and a callable parameter that shadows a local function. Both import
settings ran for every fixture. The probe used ordinary non-intrinsic call
names and did not exercise the compiler-operation branch.

| Fixture | Eager requested bytes | Deferred requested bytes |
| --- | ---: | ---: |
| 64 scalar functions | 3,408 | 0 |
| 64 functions with a local binding | 61,264 | 57,856 |
| Shadowed local call | 1,424 | 1,056 |

Every observed allocation was freed. The other fixtures requested identical
allocation totals in both arms. On the 64-function scalar fixture, deferring
the index and omitting the empty generic-template table would eliminate
12,960 requested bytes across the full and retained estimates. The binding
fixture still requests 115,712 bytes from the two call-identity scans. The
candidate needs an integrated reservation and Drop comparison before admission.
The probe log SHA-256 is
`63101ec849d592fc418f8126a0ee909b2af9c7d48e9e885282c06ccedf23d45e`.

A follow-up probe reused the existing local declaration count for uniqueness
and active-scope state. A block activated a unique name after its initializer
and cleared that block's entries after its tail. The first declaration and
first shadow name used inline storage. Later distinct names retained the
original tree-backed fallback. The deferred declaration index was enabled
in the same probe.

| Matched fixture, one estimate | Eager requested bytes | Candidate requested bytes |
| --- | ---: | ---: |
| 64 functions with one local binding each | 61,264 | 0 |
| Nested distinct locals | 1,336 | 408 |
| Contract roots with local bindings | 1,784 | 408 |
| One function with 4,096 distinct locals | 799,272 | 275,160 |
| One function with 4,097 distinct locals | 130,736 | 129,984 |
| 256 declarations of the same spelling | 760 | 0 |

Both import settings agreed in every comparison. Recursive scope exit, nested
duplicate and pattern shadowing, use before declaration, assignment, calls
and unproved String initialization retained their original discount results.
Both versions returned a discount of one at 4,096 distinct locals and zero at
4,097. The collector observed equal requested and freed bytes in each arm.
This isolated probe covered ordinary callee resolution. The resulting patch
still needs a paid production receiver and complete mode-six execution.
The wide-boundary log SHA-256 is
`9f4fe3cabd37d3dfd33701a15630d0e494015894fee64d85634930c90ab78ce2`.

The Rust 1.97.1 library test build containing `193f746db` passed the new
monomorphic zero-allocation regression, both existing generic/default cost
controls, the closed-root source inverse, the dense whole-allocation control,
and the exact/short/zero closed-root forecast control. The four-module peak
still reports 6,573,050 bytes against the saved 6,418,504. The two separate
mode-six whole-forecast tests still request more heap bytes than their
explicit reservations, so the generic-template change alone leaves these
three owning assertions open.

A deterministic generated-AST comparison expanded the local-scope prototype's
parity coverage. It parsed 2,048 input programs with nested blocks, typed and
untyped local declarations, assignments, branches, matching, ordinary calls
and repeated names. Both import settings ran, for 4,096 equally sized old and
candidate observations. All discounts agreed, including 921 nonzero results.
The probe exercises parsed input for the identity estimator, including
unresolved arithmetic and calls. It grants no HIR admission and does not bind
a production workspace reservation. Generator SHA-256
`7f666fa1550fb918eedd27cb6dc67762c0ad3f14cbb5a0996c2c70a491bf92be`.

The [Linux production census after the generic-template change](https://github.com/wavect/semaprax/actions/runs/43589935233)
passed at `cfb533d37`. All six plain and profiled logs match the earlier
`1fe13fc79` census byte for byte, including the normalizer's initial mode-one
refusal and successful mode-two retry. Production Clippy passed with
workflow profiling enabled and warnings denied. The retained provenance
SHA-256 is
`8a351dc33aa498d80731852159cf65771dd3d4a8354b24d008699d789d11117e`.
Both runs used the shipping 128 MiB limit and reached no mode-six attempt.

The joint change at `7e2c0e343` passed both deferred-filter controls and
all eight retained-scratch owners in the combined library build. The
four-module test reported a 6,405,786-byte forecast against the original
6,418,504 bytes, a warm allocation peak of 824,395 bytes, and zero live bytes
after Drop. In a fresh process it observed an 830,815-byte peak including
6,420 bytes of authenticated static prelude storage. Two pre-existing
mode-six tests still observe cumulative allocation beyond explicit payment.
The combined executable embedded a closed-root manifest from before its
final pin refresh. The unchanged custody test body, compiled separately with
the final manifest and repository reader, passed the complete inverse and
physical-source checks.

A separate identity-discount prototype used a sorted exact-capacity vector
for declarations, built at the first ordinary lookup. The six application
source sets from the Linux census, with their bundled dependency closures,
gave 112 matching old and new discount results using both import settings.
The probe requested and freed 1,831,424 bytes in the original and 1,074,944
bytes in the prototype. The normalizer and its dependency closure accounted
for 644,592 original bytes and 385,712 prototype bytes. An isolated
reservation control with a simulated ledger requested 64 bytes at the exact
bound and allocated nothing at one byte short or at zero. These observations
cover the identity discount alone. The complete fallback and production
ledger still need qualification.

The [Linux production census at `b0a2e6248`](https://github.com/wavect/semaprax/actions/runs/43592009022)
passed with profiling and production Clippy. All six plain and observed
logs match the preceding corpus byte for byte. The unchanged applications
again select modes one and two at the shipping cap. The report SHA-256 is
`e767b9e601cfc40e1cb2f6ca1e3dbcddca8a3ade38c77b4c041c6f102aee2f82`.

The next prototype also reserves each local-name backing replacement, keeps
the first entry inline, and reuses the declaration row for active-scope state.
It matched all 4,096 generated comparisons and 112 application and dependency
comparisons. Its isolated requested and freed bytes were 611,552 across the
six application closures, down from 1,831,424. The normalizer closure used
208,864 bytes, down from 644,592. The 4,097-name refusal case used 131,040
bytes versus 130,736 originally. Both call-free 64-function fixtures,
including the one-binding-per-function case, required no heap backing in
this prototype. These measurements use a simulated reservation ledger.
The candidate's complete production forecast controls are pending.

The workspace preflight usually runs before the live builder ledger exists.
The production receiver therefore also needs a bound when no ledger is
active. A separate patch threads one local scratch counter through the
identity census at the unchanged builder limit. An already active ledger
remains the payer, including its sticky refusal. The isolated probe compared
the exact, one-byte-short and zero bounds with and without an active
simulated ledger. The reverse-ordered 4,095-name case requested and freed
327,600 bytes at the exact bound. The independently retained old discount
agreed in all ten fixture/import comparisons. Probe log SHA-256
`8a95447b41f2cb8ce2a87b87bb1e82c5a90f8841310d312b9173efc7391e4789`. The standalone patch has not yet
run through the complete workspace forecast.

An independent AST census of the twelve unchanged normalizer sources counted
10,915 nodes in authored top-level function bodies and contracts, before
dependency pruning. Even if all of those nodes qualified for the existing
74-byte reachability subtraction, the gross reduction would be 807,710 bytes
before overlapping validation. That upper bound for this authored class is
below the 7,604,120-byte excess in the earlier matched complete forecast.
Actual constructor eligibility covers fewer sites. The count does not
describe generic/default expansions or another retained backing class.
The source lengths and SHA-256 hashes match the prior normalizer observation.
Census log SHA-256
`050177b3b4fa66b7c81031162d19e3210605fa00e170e52dd38cc04d493c78a1`.
