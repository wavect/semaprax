# Branch receiving constructor cut (source-only)

Base: `2d4b68d3a416cae5fba6ac08498cd56721878ba1`.
No Rust compilation, test execution, application success, or measured saving is claimed.

The cleanup CFG enumerator now executes the existing fused clone/push producer
in its own branch block. Its standalone `Result<Vec<Rc<SkeletonObservation>>,
Diagnostic>` and owned observation argument receiver no longer exist. One
budget/function forwarding pair is also absent. The root has one
conditionally initialized branch Vec, rather than an extra block-join receiver.
No persistent carrier, held failure slot, replacement payer, or global budget API
is introduced.

`inline_controls` retains the caller payment Result and every previously
charged nested work, census, conversion, backing and checked-size Result. It
also retains the checked multiplication Option, both Vec layouts, the direct
census reference, phase/scalar controls, mut Vec references and both iterator
initializer/receiver pairs. The former helper's complete `controls` and
`with_observation` functions remain byte-exact under cfg(test). The complete
immediate old enumerator calls those old functions independently. The older
Main enumerator and all three original Main control bodies are unchanged.

Clone work, complete clone census, materialization, push work, materialization,
backing delta, observation conversion, allocation, pointer copying and final
push retain their order. Existing budget methods retain their real nested
Results, payer authentication, first refusal, floor, work and Drop behavior.
There is no charge rebate or reinterpretation of a previously failed receipt.
The application trace's 1,706-byte first request establishes a real source site,
not attribution of its entire 52.5 MB cleanup-replay cumulative cost. The two
new controls report actual whole-enumerator costs once executed; they do not
claim application fit.

Two pending lib selectors are in `pending-selectors.json`: full path/work and
all-six-field refusal parity, and actual full-enumerator exact/one-short/zero
limits with complete allocation/Drop. Source custody runs after every measured
closure. It authenticates the complete compiled immediate-old function and
both exact original helper functions.

Run only the finite static proof here:

```
python3 docs/evidence/branch-inline-receiving-next-808-20261016/verify.py /path/to/checkout
```

`source-inverses.json` contains two whole Git beforeimages plus unique complete
inverse edits and two new owner hashes. No prior runtime JSON or evidence file
was changed. Final assembly must include both new children in all three raw
source audits and classify their actual cfg(test) chain. It must authenticate
current source back to this component and the independent Main/fixture branch
before the inherited source assertions; the component does not mutate a shared
source helper. `branch_clone/tests.rs` belongs to the parallel fixture fix and
is intentionally untouched here.

Suggested history entry for final assembly: cleanup replay removes the fused
branch Vec Result and forwarding receivers by executing the paid producer in
the enumerator, while retaining nested Result charges and complete independent
old enumerator controls; execution and application savings remain pending.
