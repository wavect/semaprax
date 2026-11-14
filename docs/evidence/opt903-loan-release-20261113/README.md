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

The first integrated identity receiver passed all eight retained-scratch
tests, all seventeen paired and modular census tests, and the whole mode-six
name-census allocation test. The call-free 64-function fixture allocated no
heap during either estimate. The closed-root test reached its separate
fallback-invoice assertion, where the shipping comparison included both the
closed-root selector and deferred names. The follow-up preserves that
assertion on matching eager-name inputs and checks the shipping arm alongside
them for forecast parity of retained structures and complete teardown. The
earlier pre-898 dense directory comparison still fails on the same
64-header duplicate fixture in mode six.

The preflight receiver now uses its local checked counter where no builder
ledger is active. A standalone probe compiled the repository's real ledger
and reservation decision alongside the preflight candidate. Exact, one-short,
zero, positive floor, first refusal and complete Drop matched its expectations
on all ten fixture/import combinations. At the exact bound, the largest row
requested and freed 327,600 bytes without changing the old discount. The probe
log SHA-256 is `39c7b9efd16ed97f0c91b46cb9fb45487c9d7085756ad8c1a657032e146bd822`. The new in-crate preflight and three-arm whole
forecast checks still require their own executable results.

### Published preflight and matched application receipts

The external-SSD library test build from `776d76d01` passed all eight retained
workspace-scratch tests and all seventeen paired and modular census tests. Its
four-module mode-six construction and later-validation forecast was 6,405,786
bytes versus 6,418,504 bytes from the independent saved forecast. The current
bound includes 6,383,948 retained bytes, 21,534 bytes of later validation
scratch and a 304-byte validation directory. The warm public resolve,
validation and Drop observation peaked at 824,395 bytes and freed all
44,093,332 requested bytes. The two new identity tests covered checked
preflight, one-byte-short and zero refusal, and an active ledger's floor.
The Rust 1.97.1 macOS arm64 retained-scratch log SHA-256 is
`9ee41c6430c7573c240de21b947f6afc7e0755d3078737a0688a8e5ca691b2d1`.

The same build passed 36 of the 37 call-carrier tests, including the 64-call
whole public resolve, validation and Drop test. The separate complete pre-898
comparison still failed on the dense duplicate-header case. In the legacy
mode-six forecast, the 128 call groups and 64 duplicate-bearing headers
produce 500,330 AST bytes and 134,470,181 HIR bytes, including a 118,148,794-byte
standalone local-callee certificate. The combined total is 752,783 bytes above
the 134,217,728-byte limit. The production paired forecast uses its own joint
census, whose dense receipt requires a separate comparison.

The [Linux run on `776d76d01`](https://github.com/wavect/semaprax/actions/runs/43596298515)
passed production Clippy and all six unchanged Project check captures. An
independent comparison verified the receipts and each log hash, then compared
the complete source lists, products and log hashes with the
[`b0a2e6248` baseline](https://github.com/wavect/semaprax/actions/runs/43592009022).
Each observed and plain project log was identical, including the normalizer's
first refusal and successful mode-two retry. The six checks reached modes one
and two at the shipping 128 MiB cap. These receipts do not exercise the
mode-six fallback or the historical 64 MiB acceptance. The current artifact's
provenance SHA-256 is
`c1ed0b95ff324bf722c95cbe0f32d6028b2505c7ecfa68fa479e31980273188e`.

### Functional owners and dense mode-six comparison

The macOS arm64 test image from `790a5d3ca` passed the exact Catalog23 and
ShiftSim15 functional owners and all 13 Catalog Normalizer tests. The
normalizer tests include the independent original framing and indexed-decoder
comparisons, native and Core Wasm execution, and the maximum valid outputs.
The same image passed all four NDJSON package tests, including authenticated
checked-cache reuse and forged-range refusal. At `776d76d01`, the [hosted
installed compiler](https://github.com/wavect/semaprax/actions/runs/43596819582)
passed the 33 historical checks and 16 boundary checks in each of the two
unchanged native LogLens references. The hosted receipt binds the checkout
and original compiler through both runs. Its SHA-256 is
`99db40622fbb0d7db8956ca4b06f3e13b5c6dbc46bacda92b548facc1ac4104c`.

A test-only probe at `776d76d01` used the same four authored dense fixtures,
the complete pre-898 producer, the legacy mode-six producer and the production
paired mode-six estimates. All arms kept the 134,217,728-byte limit. The
success and refusal columns below are clone-plus-HIR estimates for one
module. They precede the workspace's later validation ceiling.

| Fixture | Qualified auxiliary roots | Pre-898 bytes | Legacy mode six | Production full | Production retained |
| --- | ---: | ---: | ---: | ---: | ---: |
| 512 headers, distinct arguments | 2,048 | 21,594,620 | 55,972,057 | 41,673,169 | 41,674,456 |
| 64 headers, duplicate arguments | 0 | 14,473,218 | Refused | 19,813,640 | 19,814,927 |
| 64 headers, warm statement calls | 2,048 | 13,031,816 | Refused | 19,359,355 | 19,360,642 |
| 64 headers, balanced warm calls | 2,176 | 12,733,712 | 34,601,680 | 16,583,051 | 16,584,338 |

The legacy refused rows reconcile to 134,970,511 and 135,223,686 bytes using
the independent auxiliary and local-callee fee and identity credit. The
balanced production row still exceeds the pre-898 estimate by 3,849,339
bytes. Admission of the production module estimates does not establish the
strict reduction or the historical 64 MiB application acceptance. The direct
probe log SHA-256 is
`cc23ab149a6932ef9706e746bd71337cf6b7a1c96f83b091998c36ab9da4c79e`.

### Dense workspace forecast and repeated-callee experiment

The four dense fixtures also passed the complete production workspace forecast
at `776d76d01` with the original 134,217,728-byte ceiling. This run used the
active builder ledger and the ordinary `uncached_output_layout` entry. The
complete per-module observer reconciled every HIR term with the returned cost.
The older component summary omitted both proof terms and could not reconcile
these same rows.

| Fixture | Workspace forecast | Active ledger forecast debit |
| --- | ---: | ---: |
| 512 headers, distinct arguments | 40,803,469 | 9,122,529 |
| 64 headers, duplicate arguments | 19,314,238 | 148,882 |
| 64 headers, warm statement calls | 18,859,947 | 3,779,457 |
| 64 headers, balanced warm calls | 16,082,639 | 3,759,137 |

The balanced row's HIR term is 16,081,711 bytes, comprising 7,306,884 fixed
bytes, 3,824,291 for the auxiliary census, 3,750,360 for the local-callee
certificate, 72,480 for the prelude, 271,280 for identity carriers and 856,416
for identity occurrences. The captured observer log SHA-256 is
`95ba3d3fe036e954468e2e75d67bb69b7cfb9c4b6150b4671f6add22ccc7b639`.

An unpublished prototype paid for a fresh owner or callee publication only
when the sealed loan changed. The ten paired and repeated-callee tests passed,
including exact, short, sticky floor, foreign lender and Drop checks. The
three 64-header forecasts fell by 23,890, 329,641 and 329,641 bytes in the same
order. The 512-header forecast increased by 265,464 bytes. The balanced
clone-plus-HIR result was 16,253,410 bytes versus the complete pre-898 result
of 12,733,712. The original strict comparison still fails. The prototype log
SHA-256 is `cdbd285ce6c20f948c5f1bb9081bb227bfecd395b43bef108926afb8899c4da2`.

The test forecast observer now records both proof charges alongside the
existing component fields. The summary checks the full HIR equation and the
raw-clone-plus-HIR equation. Live mode-six receipts also report the charges
for each full and retained component row, with the original formula result
available as `historical_component_equations_valid`. The compiled writer
replayed all four captured dense fixtures using the separately captured
proof charges. It reconciled all eight component rows and reported no
reconciliation errors. Replay log SHA-256
`2fed3b18896a45a003d49c558e8739d7db0a4988099873437a817cb07b525c1e`.

### Resolver path arena qualification (#778)

The `776d76d01` macOS arm64 build passed 15 path-arena tests, four
streamed-identity tests and ten resolver-attribution and execution checks.
The saved text receiver agreed on complete HIR, ordered loan and cleanup
plans, wire bytes, and diagnostics and spans for malformed input. The
traversal suite also passed on direct and synthesized lowering fixtures.
The unchanged Catalog23 and ShiftSim15 owners passed at `790a5d3ca`,
including ShiftSim's maximum-cardinality compact and escaped-key cases.

The additional paired test runs both complete HIR outputs through the
ordinary selected StreamText interpreter. Both skip the failing right
operand of a lazy Boolean and return 13. The two owned arguments stage and
commit in order 0, 1. When the second argument fails, both runs select
`semaprax.text.v1` status 1. The test log SHA-256 is
`35e3d248533dffe99abe12daf2015bcf87881b98b06363be2bd1707fff65e8dd`.

The arena growth control requests 80 and 160 bytes of node backing. It
observes a 240-byte allocation high-water during replacement and a total of
274 reserved and requested bytes including final path materialization. At
79 bytes the initial arena allocation refuses before any allocation, and
the zero-unit retry preserves the first refusal. The historical 64 MiB
application ledger observations remain recorded in the
[workspace contract](../../WORKSPACE-SEMANTIC-GRAPH-V1.md#resolver-path-arena-opt778-representation-tranche).

### Two checked callee loans in the paired forecast

An SSD-local experiment kept two checked target loans within a single paired
certificate. A primary miss paid for probing the alternate loan. A double miss
ran the full target authentication and paid for publishing the replacement.
Each observed call still checked its reached arity. The independent
standalone certificate was the same across all runs.

| Mode-six workspace fixture | Main pre-charge forecast | Staged publication only | Two checked targets |
| --- | ---: | ---: | ---: |
| 512 headers, wide | 40,803,469 | 41,068,933 | 42,259,368 |
| 64 headers, duplicate | 19,314,238 | 19,290,348 | 19,291,058 |
| 64 headers, shadow names | 18,859,947 | 18,530,306 | 16,426,335 |
| 64 headers, balanced | 16,082,639 | 15,752,998 | 13,649,027 |

All four live pre-charge fixtures finished. In the balanced fixture, the
candidate's full paired clone-and-HIR forecast was 14,149,439 bytes, still
1,415,727 bytes above the independent pre-898 value of 12,733,712 bytes.
The strict comparison still failed at the standalone mode-six duplicate-header
row. The five reuse controls and six paired controls passed, including the
alternating-loan arity, foreign-source and one-byte-short checks. This candidate
stays off main because the wide fixture increased by 1,455,899 bytes and the
strict comparison remains open. The application owners were outside this run.

The focused build used production base `776d76d01`, whose later observer fixes
affect the diagnostic output only. Baseline, candidate and strict test log
SHA-256 values, respectively:

- `95ba3d3fe036e954468e2e75d67bb69b7cfb9c4b6150b4671f6add22ccc7b639`
- `2355b7e7c956056a2245160c955fbdf4d77202bc4c4ec00f2ddaa46500e8b611`
- `5ecbc1a599424ae789cc6ed00477de3c78482286acba89c625e66377a33a4216`

### Historical default budget and retry comparison

The earlier 64 MiB Project reporting checks used an explicit lower limit
on a compiler whose default was 128 MiB. The ordinary core retries only at
the default limit and with no enclosing builder ledger. To exercise that
original path, a private production build of `dfe57743b` set the default
to 67,108,864 bytes and updated its matching static fallback diagnostic.
No forecast, replay limit, debit rule, retry policy or application input
was edited.

| Published project | Explicit 64 MiB limit | 64 MiB as the default |
| --- | --- | --- |
| Catalog | Passed in mode 1 at 48,615,173 bytes | Passed in mode 1 at 48,615,173 bytes |
| ShiftSim | Refused in mode 1 after 67,104,702 bytes | Passed in mode 5 at 66,822,056 bytes after four failed attempts |
| Catalog Normalizer | Refused in static precharge | Same static refusal before a live attempt |
| Selected-file LogLens | Passed in mode 1 at 21,430,497 bytes in the earlier reporting run | Passed in mode 1 at 21,430,497 bytes |

ShiftSim's first attempt matched the explicit-limit trace at the same
forecast and cumulative debit. The first failed request was 8,576 bytes
with 4,162 remaining. The phase census attributes 66,704,189 bytes to
precharge and 357,581 bytes to expected workspace edges before that
refusal. It records the generic reservation caller, so it does not
identify the more specific compiler operation.

| ShiftSim mode | Prebound bytes | Cumulative charged bytes | Outcome |
| --- | ---: | ---: | --- |
| 1 | 66,704,189 | 67,104,702 | Refused |
| 2 | 57,320,193 | 67,106,424 | Refused |
| 3 | 57,103,327 | 67,015,297 | Refused |
| 4 | 56,862,800 | 67,088,999 | Refused |
| 5 | 56,063,583 | 66,822,056 | Passed |

The profiling capture counted 193 completed PLAN comparison owners and
28,732 certified right-hand-side admissions across the successful ShiftSim
Project check. The per-owner detail buffer retained its first 128 rows.
Normalizer still stopped at 66,248,903 bytes of retained output plus the
873,483-byte NDJSON module forecast. The first-step excess is 13,522 bytes.
Later modules and live validation still need independent qualification.
These are production Project check observations. They do not establish
that the complete starter, native suite or an immediate predecessor passes
at this limit.

The local patch and linked probe SHA-256 values are
`8e9febcd9ce34480687337862ae3c7784cec7aceb96da2636a1786e41ad096ac`
and `60756fb6a5cbdc728f1df5d1d7cf305ec46542edfc24bd819341fb9cfc0c4941`.
The ShiftSim and Normalizer log SHA-256 values are
`8bc404db12443d6cc72d5379275e078b4a4ca09998e9759680710f253803785a`
and `ef9e21cea364c55de47d31a272dd14ebec72d7f74742d91e99f64ad0422e4dc4`.

### Fresh starter at the historical default

The full-toolchain CLI on the same 64 MiB build passed four of the five
unchanged starter tests, including native execution of the fresh LogLens
starter against CLI49. The fresh ShiftSim starter failed during `new`,
before its output tree was published. Six earlier modules retained
60,265,703 forecast bytes and the full generated response module added
13,130,193 bytes. The rejecting step totals 73,395,896 bytes.

A separate probe called the public codec-plan derivation on the exact
published ShiftSim revision. The authored Project passed. The codec-only
candidate and the complete starter with command activation produced the same
six-field diagnostic, including the same response module forecast. The
starter checks the codec bundle before advancing to command activation.
This locates the failing admission in the generated codec candidate.
The checked-in ShiftSim Project still passes at this limit. The early
forecast excess gives no estimate of what the subsequent live build needs.

The unchanged starter test and stage-isolation log SHA-256 values are
`386d999164e285b51b46a65525e9d3fa1e4d6068995bf3868b2f616c98c3d32a`
and `8ff41f510b25fd093e6d3a1b3d66311432e28e7d323fb50aa2a05bc2e59c467b`.

### Fresh ShiftSim codec candidate forecast by module

A test-only observer on `a9cec46db` restored the historical 64 MiB
builder default and captured the actual mode-six admission of the
complete ShiftSim codec draft. It used the original codec plan and
authored Project. A default-feature production-library run at the
128 MiB shipping limit generated byte-identical request and response
modules. Both runs generated 48,385 request bytes and 23,829 response
bytes, with SHA-256 values
`6e3aa28ae67c2c019203990b1123ca1da44dd54f124b3cfb7358230678833bad`
and `c32d6c4c99a47ea3acda70585477055a1de81e99b826184bea163fccbcb0a146`.

All 16 module component equations reconciled. Mode six refused
`src/response.spx` at the same point as the production-library 64 MiB
diagnostic. Its full 13,130,193-byte forecast added to 60,265,703 bytes
for six preceding modules. The observer captured every module's cost
before the sequential admission loop. The 16 retained costs sum to
94,355,872 bytes before later validation scratch and the function
directory. Reordering modules cannot eliminate that excess.

| Module | Full forecast | Retained forecast | Full fixed-layout and string term |
| --- | ---: | ---: | ---: |
| `src/request.spx` | 26,456,984 | 26,014,299 | 17,957,062 |
| `src/schedule.spx` | 24,593,948 | 24,115,015 | 21,840,198 |
| `src/response.spx` | 13,130,193 | 13,052,828 | 7,363,904 |
| `src/order.spx` | 8,876,715 | 8,834,226 | 8,692,852 |

The order module includes one `vec_sort_by` and the schedule module
includes one `vec_unique_by` and one `vec_update_field`. Each is
charged eight maximum-size descriptor buffers of 16,384 bytes as
string input. The 64-copy multiplier adds 8,388,608 forecast bytes
per site. Independently encoding the selected layouts yields final
descriptors of 409, 294 and 199 bytes respectively. Final descriptor
length alone does not cover temporary layout buffers, decoding,
reconstruction, or retained copies.

Subtracting all three maximum-size allowances would still leave
69,190,048 bytes in the retained sum, 2,081,184 bytes above the
original cap before later validation. This subtraction is a
counterfactual. The actual descriptor bounds and a second reduction
in retained costs need independent qualification.

The test-only observer log SHA-256 is
`143571ae244c3a076f8764049e1275292d96538f749797ba66a45c682ad2144b`.
The local diagnostic patch SHA-256 is
`82cc49a3442d116b9b9a44a8a905c174dc4f7eca2f289e46ff56fa02eb244741`.

### Descriptor construction and decode at the original limit

A second test-only observer on the same source measured the three
authenticated descriptors independently. Construction covered the ordinary
source descriptor and its identity. Decoding started with the complete
identity and reconstructed the ordinary descriptor. Each measurement
included dropping the resulting allocation.

| Site | Construction traffic | Construction peak | Decode traffic | Decode peak |
| --- | ---: | ---: | ---: | ---: |
| `src/order.spx` `vec_sort_by` | 3,371 | 1,078 | 1,121 | 988 |
| `src/schedule.spx` `vec_unique_by` | 2,786 | 788 | 603 | 577 |
| `src/schedule.spx` `vec_update_field` | 1,650 | 535 | 410 | 380 |

All amounts are bytes. Each measurement returned to zero live bytes.
The one owning observer test passed and reproduced the unchanged
`src/response.spx` refusal. The measured operations exclude later HIR
copies and their owning validation passes.

A local compiler experiment replaced the maximum-size descriptor allowance
at these sites with a charge derived from the source descriptor's
identity length, layout capacity and selector length. The unchanged
ShiftSim project integration test advanced from six to twelve retained
modules. Mode six then refused the UTF-8 dependency
`dependencies/std.data.json.utf8/0.1.0/utf8.spx`.
Its 3,155,235-byte forecast added to 64,675,563 retained bytes, for a
67,830,798-byte total, 721,934 bytes above the original cap.
The experiment measured the change in refusal site. A production bound
still needs to cover temporary allocations and all relevant HIR copies.

| Local artifact | SHA-256 |
| --- | --- |
| Descriptor observer patch | `859f8c1251936b3284238362c6ca74734a7bfe5c125ec9b4ef1c4d2b8a2ae66a` |
| Observer test log | `ca7b0f590009537c2d2550c65ab7b501ee5beb3eb9dfdfd2d21d1eb4bdc88394` |
| Forecast experiment patch | `233307f7c9165eba350781f5a26f6505183cf85c71f52dd744368a7d35dd9c13` |
| Unchanged project test log for the experiment | `9ef7d0415ca2709064bcacb9072aa11be8884431a258fccbc541d793879a8c6d` |

The shipping library at `cd0da39c8` passed all 294 selected comparator and
expression-fact tests in 873.13 seconds. The test log SHA-256 is
`ffcecdba59957c7de3f52fe5f7f25f56744ce47d8a02e7b594c6eae89b5b4cd7`.

### Shared Row response bodies on the complete ShiftSim command

This source experiment used the already generated request and response
modules and activated the unchanged ShiftSim command. It kept the
direct and indexed Row helper signatures, persistent identities, render
preconditions and field order. The wrappers pass borrowed String views
and Copy scalar fields. One arm shared the render body, and a second arm
also shared the length body. Metrics and request generation stayed
byte-identical.

All three arms used the same compiler built from `540d5ee48` with a 64 MiB
default. Each earlier module in the seven-module retained prefix was
unchanged, for a total of 61,391,864 bytes. Each arm still refused
`src/response.spx` at static admission.

| Response source | Source bytes | Complete response-module forecast | Forecast reduction |
| --- | ---: | ---: | ---: |
| Original generated source | 23,829 | 13,130,193 | 0 |
| Shared render body | 21,557 | 11,926,772 | 1,203,421 |
| Shared length and render bodies | 21,362 | 11,814,960 | 1,315,233 |

The separate descriptor forecast experiment admitted the response module
in all three arms. The next refusal was the UTF-8 dependency at the
same 3,155,235-byte complete forecast, with thirteen earlier retained
modules in each arm. The shared length and render arm reduced that
retained prefix by 1,340,193 bytes. Its first refusal was still 507,902
bytes above the cap. This first refusal leaves the rest of the forecast
and live validation unmeasured.

| Response source with the descriptor experiment | Retained before UTF-8 | Total at refusal | Excess over 64 MiB |
| --- | ---: | ---: | ---: |
| Original generated source | 65,801,724 | 68,956,959 | 1,848,095 |
| Shared render body | 64,585,823 | 67,741,058 | 632,194 |
| Shared length and render bodies | 64,461,531 | 67,616,766 | 507,902 |

At the shipping 128 MiB limit, the macOS arm64 reference compiler from
`6f1db7ec4` passed both declared Project tests and built a native executable
for each arm. All three executables passed the unchanged 11 valid and
four invalid acceptance cases. An additional differential run compared
exit status and exact stdout and stderr across those 15 inputs and the
two checked-in fixtures. All 17 inputs matched.

The experiments edited an isolated generated module. The current codec
emitter, source derivation and receipt replay remain the owning
qualification routes for a production change. This execution checks the
native command and its retained wire corpus. It does not establish the
original-cap paired codec gate or the behavior of arbitrary schemas.

| Artifact | SHA-256 |
| --- | --- |
| 64 MiB compiler binary | `4a7c22eedc04df6e25754c66236da1ea4898b58ebe760973f92a776a21417c25` |
| 128 MiB reference compiler binary | `33cd423478fd9b752b72daa8eebdd45d27d49fd640bc314656c5021e25e370ef` |
| Render-only experiment patch | `5ce1e8676248dca9a9774ab79f85c39349501618d04c2f90237bca0139433f77` |
| Shared length and render experiment patch | `6988226b88b0133b0cb82cba3a0e8636e4265a22351b006f9138ddf76e2cb3b1` |
| Identical native corpus receipt | `6d2edb5c32701c57d8f4b8c2c065c2e0bbd2951c1db88598c2dcda62c88dc1af` |
| Differential 17-input log | `e20933b25b4fb631d5ddd8101482356a1b7c2024740564319cf3bf5c459a90f2` |
| Descriptor experiment CLI binary | `18d71d7d5f8d7ce4a6fc8f1328b8d5803449e103ef8b068b94d1a2c561fe6c9d` |
| Descriptor experiment, original response log | `f725082d94d1eb51b2aaadf1505d9b641bca4516218dc878262b1126fadd3ed4` |
| Descriptor experiment, shared render log | `161eb2697f62a90fc6ac22276f509c64b31cc125f06510c4ce653707273afedb` |
| Descriptor experiment, shared length and render log | `b08d42defc06c60a5d23c20e37df0c1cc4249d7fd07acbb5768cc163b85b6fcd` |

### Complete ShiftSim forecast and decode-only request control

A local observer recomputed every mode-six step from the same 16 module
costs before the ordinary sequential limit check. It also included the
later validation scratch and directory. Each scenario kept the
previous descriptor forecast experiment. The installed command used
the already generated request and response modules.

| Installed ShiftSim source | Retained sum | Complete static peak | Complete excess over 64 MiB |
| --- | ---: | ---: | ---: |
| Full request and original response | 70,916,028 | 70,923,834 | 3,814,970 |
| Full request and shared Row bodies | 69,575,835 | 69,583,641 | 2,474,777 |
| Full request, shared Row bodies and compact digit and escape expressions | 69,154,620 | 69,162,394 | 2,053,530 |
| Decode-only request and shared Row bodies | 62,498,087 | 62,505,893 | Fits |
| Decode-only request, shared Row bodies and compact digit and escape expressions | 62,076,872 | 62,084,646 | Fits |

The full-request experiments retain the complete original codec shape.
The compact response variant uses bounded arithmetic for decimal and
hexadecimal digits and one scalar match for the existing short escapes.
A reference compiler at 128 MiB passed both declared tests and all 15
native acceptance cases with that response. Its receipt is identical to
the original response receipt above.

The existing, explicitly selected `stream-owned-request-decode-only.v1`
profile generated a 35,206-byte request at 128 MiB. The original full
request is 48,385 bytes. The experiment installed that generated request
into both response variants. The published ShiftSim plan still uses its
full request profile.

The decode-only paired codec derivation with the original response fit
static admission at 62,915,242 bytes in the experimental compiler. It
then refused a 104-byte request with 8 bytes remaining during live
synthetic module construction. On the two installed command variants,
the live builder refused 7,392 bytes with 3,056 remaining for loan vector
backing, and 376,225 bytes with 33,115 remaining for the uncached
synthetic AST peak, respectively. Both installed commands therefore
still refuse at the original cap. The live attempts pay their unchanged
reservations after the static phase.

A live module trace pinned those two refusals. The shared-Row arm
completed `src/app.spx` and the JSON scanner, then refused during
`src/tests.spx` with the loan vector diagnostic. The compact-response arm
also completed `src/tests.spx`. In `src/schedule.spx`, the live synthetic
AST peak targeted 519,343 bytes against the previously charged 143,118
bytes, yielding the observed 376,225-byte request. The static estimate
and the complete diagnostic were byte-identical after removing the new
observer lines. Neither live run reached the request module in its
sequential resolution pass.

The compact installed decode-only command passed both declared tests and
all 15 retained cases at 128 MiB. Its native result matched the original
full-request and compact full-request commands on exit status and exact
stdout and stderr for all 15 cases and the two checked-in fixtures.
A second decode-only derivation at 128 MiB reproduced the complete
paired bundle byte for byte.

| Artifact | SHA-256 |
| --- | --- |
| Full forecast observer patch | `85faeb9ee10607e838848b2603cb6a77488ef79b0859ec85e56c64d70ef5bd70` |
| Reconciled original and compact-response forecasts | `9b181d2bf10a0c5f11eb0af9770319f517287605e6e9be7f13688e03102df9dc` |
| Decode-only paired bundle at 128 MiB | `5cb46580c7cac679929e57372a8628d93f6349eb9e2631e0f524e980ac95198b` |
| Decode-only paired derivation refusal at 64 MiB | `8cfa643f23ef44ff481a52c8f21f17c12a80beadccfa778971ee268955e3e212` |
| Decode-only command with shared Row bodies | `af3a8cd6e8917d52020b23b2bd6518b3743fe06bcd0e4c648b24404d40db00b2` |
| Decode-only command with compact response | `84248289d7947708d22f64149beda7ebf43ef4f34244d6f4eaaf6961178d2a6a` |
| Live module observer patch | `0dd2fb9939da16055d1b59f11071a61915ad4c3cc0b8c2165b8858539f99fb9c` |
| Live shared-Row module trace | `e4f3e47bae336e880beda071ca01f93d90a4498b7437d9d5226c3b8f3f8624e8` |
| Live compact-response module trace | `19186c050ea4daacc590af9188e3d281a0ec9ceb6ce3242bbedb6c11fe2c1f45` |
| Native wire comparison | `c09436ca33a6d2d42a061d2595b9466c4cc4289dbdc0487eec1af1f16cc09e75` |

### Native codec and generated key controls at 64 MiB

On the historical-default build above, both native primitive String codec
tests passed. The unchanged Catalog23 functional test refused during the
combined codec derivation, before it ran the corpus. The live HIR
expression/type lowering charge requested 40 bytes with 8 remaining and a
zero-byte floor. The 18-module static precharge was 64,840,010 bytes.
The largest module was `src/request.spx` at 20,875,498 bytes. The published
Catalog Project check passed earlier at the same default. The functional
test passed separately at the shipping 128 MiB default.

A local ShiftSim experiment emitted multi-byte collection-response keys as
borrowed UTF-8 literals feeding the existing byte writer. The unchanged
fresh-starter test still refused during `new` at 64 MiB. The six earlier
modules retained the same 60,265,703 bytes. The full response forecast
rose from 13,130,193 to 13,143,255 bytes, an increase of 13,062 bytes.
The shipping generator remains unchanged.

The native codec and Catalog23 historical-default logs have SHA-256
`771da0d7a862f8c6294bcbbd5cff765ce18a6e8d8a5ef039ab2b6a5cd90f1880`
and `549771e290664eee0356f841f9843111b3ba766c54d02f7db5da7afe7f6c21b8`.
The borrowed-literal patch and starter-test log have SHA-256
`547657069f9c4e249eac0d176b427b2b34dc56fd1dcaa356b052ff6e93c5b9eb`
and `12bf64d7bcfd4492f910af3dd42660a6cb77cc75f4b0690b85d8c447fecd36c9`.

### OPT #904 current lexical-exit controls

At `fcc9ae0ad`, 14 of 17 lexical-exit tests passed on macOS arm64
with Rust 1.97.1. The three failures reached the unchanged 64-call
fixture in its second HIR validation pass. The old helper and current
inline arms returned identical first-refusal receipts. Both had paid
67,108,769 bytes and refused the same 128-byte request with 95 bytes
remaining and a zero floor. Both used a 144-byte frame, with 494
cumulative capacity slots across 14 requests, charged zero match-witness
roles, and released all observed allocations. The failure came from
the enumerated HIR content sort while receiving a root-items frame.
The remaining capacity describes that individual request. The higher-budget
diagnostic below measures the complete debit for the same fixture.

The lexical-exit test log SHA-256 is
`76340e5cfc0c618336081190d0c508dcc359b76ef6498c0a74e34c1dfc96db2a`.
The owning Rust library executable SHA-256 is
`5dde6fc17f18a42d88534d6a89c7f8d63bf976e105cd519384f2740f588de9d5`.

### OPT #889 current singleton controls

All 24 inline-singleton and ordinary-label tests passed on the
`fcc9ae0ad` Rust 1.97.1 macOS arm64 test image. The measured old and
current canonical frame layouts are both 40 bytes with alignment 8.
The outer result layout fell from 32 to 24 bytes. The 64-singleton
chain reduced cumulative reservations from 292,990 to 292,176 bytes
and total requested heap from 54,101 to 52,565 bytes. Both versions
peaked at 34,661 requested heap bytes and left zero live bytes.
The empty cached control instead rose from 1,068 to 2,508 reserved
bytes, and the tested many-label cases also incurred additional fees.

The two complete old/current HIR and PLAN comparison fixtures, WHILE
and GENERATED, paid 746,312 versus 696,716 bytes and 1,308,916 versus
1,189,052 bytes respectively. Both preserved their observed work
counters and returned the same verdict. The shipping comparator was
also exercised as a separate arm on those fixtures, at 785,967 and
1,363,315 paid bytes. These are owning fixture results. The original
64 MiB application gates remain outstanding.

The focused log SHA-256 is
`e9b65ad834af943a11a30c3c92b7fee2c6edc99001f99cdb405510f5e8e736c0`.

### Current OPT #885 and #886 receiver checks

At `da36ffb03`, all three focused ordinary lookup caller tests passed.
The #885 comparison against the immediate original receiver paid
38,780 fewer bytes at 32 rows and 78,716 fewer at 64 rows. The zero-
and one-row controls each paid 900 more. The independent reference
account stayed separate. The broader lookup and PLAN comparator
selection passed 137 of 140 tests.

For #886, the preserved pair-local-only arm reduced cumulative
reservations from 583,851 to 553,035 bytes on 115 prepared pairs and
266 completed rounds, with unchanged observed work and heap tuples.
The one-pair, two-round fixture rose from 28,926 to 29,406 bytes.
The single 257-round pair fell from 312,999 to 272,679 bytes, and
the no-pair control rose from 15,857 to 16,561 bytes. These are
instrumented library-test observations. Shipping application parity
at the original budget remains a separate gate.

The rerun passed all 140 tests from the original selection after
repairing the three failures. The source control restores the declared
43,965-byte pre-format view before applying the original inverse,
which yields the exact 43,884-byte predecessor. The two saved 289
independent-segment controls now call the comparator that owns the
census they read. The two comparator owners passed their 37 pair-scope
tests, including exact invoice and refusal controls.

The focused lookup log SHA-256 is
`7d03fabec6854079d75fe6d4c2d9ab67e9fee4371f88be048e55bd5655dd4389`.
The original combined log SHA-256 is
`31597a6f3a26cfe417644e6b024623a05ec9e0670344e5805ad91d2668cbfeda`.

#### External application checks for OPT #886

The immediate comparator at `4624d6b6a` and the pair-local-only successor
at `3f3f211de` were built separately with Rust 1.97.1 on macOS arm64.
The two commits have identical owning test sources, application inputs,
corpora and dependency locks. The external `native` and `project`
test executables linked the ordinary production library with default
features. Both revisions used their original 64 MiB builder limit.

All four selections refused with `SPX-G171` during HIR final validation
in both arms. These are the first refused requests and remaining bytes.

| External selection | Immediate comparator | Pair-local successor |
| --- | ---: | ---: |
| Native String codec allocation and exact/+1 preflight, both tests | 258 / 130 | 304 / 49 |
| Catalog23 typed records | 132 / 106 | 132 / 84 |
| ShiftSim15 typed records | 98,304 / 6,385 | 98,304 / 11,761 |

The two native tests and both application tests passed their static
forecast in both arms. The first-refusal pairs describe the observed
prefixes. They do not measure savings or the complete debit.

A separate build from `a9cec46db` passed Catalog23 and ShiftSim15 at
the current shipping builder limit of 128 MiB. A diagnostic checkout
of the same revision restored the original 64 MiB builder ceiling
and its matching diagnostic text. Both native codec tests passed
there. Catalog23 refused in HIR expression and type lowering with
a 40-byte request and 8 remaining. ShiftSim15 refused during the
static forecast after retaining 60,265,703 bytes and forecasting
13,130,193 more for `src/response.spx`. This diagnostic keeps the
current separate replay ledger. The historical two-arm comparison
above isolates the pair-local change and keeps the historical accounting
in both arms. The original-cap application gate remains open.

| Receipt | SHA-256 |
| --- | --- |
| Immediate native test log | `3626da672dab85f9c3f65c547540f43ca3b016e8fcec856befe60894041a2fa6` |
| Immediate project test log | `33f160b16e6e300b80695ce29a81975b941f1f7e77e8398309bb678c49ddb0ec` |
| Pair-local native test log | `6b2eee228a859723da03b2d9a70acdd997c8f630d449cc3f212acb296aade2c0` |
| Pair-local project test log | `abb797ea47da21a3e4d39c0a83f8e0662e93153389e00e2f23b4578143eb327a` |
| Current shipping project test log | `a029410def8c83f1bb3e8839f1274baef5daa921f0f8417f6f1faedc84d4b234` |
| Current original-cap diagnostic patch | `8e9febcd9ce34480687337862ae3c7784cec7aceb96da2636a1786e41ad096ac` |
| Current original-cap native test log | `5132515e07435b7df56dfab780555585700364595cc9a7b1948474d7f4fcc299` |
| Current original-cap project test log | `2f774b5dd98fd51d449580262ceb2b68d2b0554cb6a8b24e2460f5d86a20bf1b` |

Production library Clippy with `-D warnings` passed at `a9cec46db`.

#### Phase attribution for the 64-call control

A test-only build from `da36ffb03` ran the exact 392-byte fixture and
both original HIR calls with one shared 64 MiB account. The source
SHA-256 was
`5c73ba5f6ff61f92bca916bf88899327b9bd86bf601ca481ef858447b62b2ace`.
The observer used the existing stage and operation counters for debits.
It ran the original independently financed validation references.

| Observed debit | Bytes |
| --- | ---: |
| HIR resolution, including its internal validation | 178,548 |
| Second HIR validation through refusal | 66,930,221 |
| Cleanup replay across both calls | 66,845,514 |
| Total | 67,108,769 |

The first refusal was attributed to cleanup replay at
`src/cleanup_plan/validate/scratch_ledger.rs:65`. It was the same
128-byte request with 95 remaining. The independent reference
completed all eight allotted calls and charged 9,763,044 bytes on its
own ledger.

The second validation phase census saw 88,866 common-frame renewal
requests for 40,522,896 bytes and 73,243 root-items requests for
9,375,104 bytes. Those figures count requests, including refused
requests and any independently financed observer calls seen by the
phase census. Only the reconciled debit table above measures charges
to the fixture's 64 MiB ledger.

The diagnostic log SHA-256 is
`5ecd5553c7a8f6473adf016b3c78b7404dab5ec036007c8503d61bd13560c34f`.

A follow-up test-only run kept the exact source, both original HIR
calls, and all eight independently financed reference calls. It
varied only the primary ledger bound and emitted a receipt for
every outcome.

| Diagnostic bound | HIR resolution | Second HIR validation | Cleanup replay across both calls | Total debit | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| 64 MiB | 178,548 | 66,930,221 | 66,845,514 | 67,108,769 | `SPX-H006` |
| 128 MiB | 178,548 | 134,038,078 | 133,953,371 | 134,216,626 | `SPX-H006` |
| 256 MiB | 178,548 | 150,642,517 | 150,557,810 | 150,821,065 | Both calls passed |

At 128 MiB the first refused request was 1,216 bytes with 1,102
remaining. This time it reached the ordinary PLAN shared HIR cursor
next receivers. At 256 MiB the primary ledger had no recorded
refusal. Cleanup replay accounted for 150,557,810 of the 150,821,065
bytes charged. The separately financed reference ledger charged
9,763,044 bytes in all three runs.

In the successful second validation, the largest observation bucket
was the enumerated HIR common-frame renewal, with 151,849 requests
totalling 69,243,144 bytes. The next two buckets were the ordinary
PLAN shared HIR cursor next receivers at 15,325,248 requested bytes
and the enumerated HIR root-items frame at 14,974,464 requested bytes.
The observer counts requests across both ledgers. The original 64 MiB
application gate remains open.

The follow-up diagnostic log SHA-256 is
`e7878e4669d5fe0b472a4185395dad62eb7b08c06de945a09115e03529ab0f86`.
The observer source diff SHA-256 is
`cad5654303abf1ec3979b4fcb2d9d7b8e881de1cdd76194f5ae83584109d5855`.
