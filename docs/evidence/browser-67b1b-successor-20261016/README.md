# Local 67b browser graph successor

Actual compiler subject: `67b1b02141f19040fdd1b45bc9bea16dcff5d74e`;
source tree: `7896884ed715bdb0bfd51c7e273e4b1304fba5d8`.
The two unmodified capture wires have builder bill `3468381`. This is historical
local producer evidence, not a browser pass, owner-test pass, or a result for a
later integration head. `binding.json` and `results.json` retain producer custody.

`static-proof.py` checks raw graph digests, complete historical graph preimages,
and complete source inverse chains without invoking the compiler or tests.
`static-proof.json` records that check. Whole old receivers, including both
independent actual83ca and Main75b trees, remain registered.

New selectors in the `wasm` harness:

- `scalar_browser_ci_contract::historical_builder::actual_67b::actual67b_wires_run_both_complete_independent_receiver_trees`
- `scalar_browser_ci_contract::historical_builder::actual_67b::actual67b_source_inverse_preserves_every_complete_old_guard_input`

The existing live fixture selector is
`scalar_browser_ci_contract::browser_known_answers_match_authenticated_baseline_and_rename_graphs`.
It requires the pins for its actual compiler build. The final integration owner
must obtain a fresh capture after source integration before running it.

## Preparing the next actual capture

`update-next-actual.py` is a reusable preparation command for a clean checkout
containing this layer. It reads an independently acquired capture; it does not
build, capture, run tests, infer a bill, or change the old evidence. Supply full
literal hashes from the fresh producer receipts. The target HEAD and tree must
match the capture and owning-build receipt. First omit `--apply` to review its
static proof and output inventory; repeat with `--apply` to write the new layer.

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 \
  docs/evidence/browser-67b1b-successor-20261016/update-next-actual.py \
  --root /Volumes/Semaprax/worktrees/YOUR-CHECKOUT/AI-Lang \
  --capture /Volumes/Semaprax/provenance/YOUR-FRESH-CAPTURE \
  --head FULL_CAPTURE_SUBJECT --tree FULL_CAPTURE_TREE \
  --binding-sha256 FRESH_BINDING_SHA256 \
  --receipt /Volumes/Semaprax/provenance/YOUR-OWNING-BUILD-RECEIPT.json \
  --receipt-sha256 FRESH_RECEIPT_SHA256 \
  --evidence-name browser-next-actual-YOUR-SUBJECT
```

The command checks successful authenticated baseline/renamed results and graph
digests. It accepts only a builder-bill/digest change whose full inverse equals
the retained 67b wires. Any other graph change requires a separately reviewed
adapter. The generated `next_actual.rs` adds an outer receiver; old67b,
actual83ca, actual55e, and Main75b receivers keep their exact old inputs. The
new source table restores every edited file in full, including the custody
module. It installs once per target checkout and refuses an existing next layer.
The generated Rust and final integration remain subject to the owner's normal
verification; this lane ran no Cargo, compiler, browser, tests, or Clippy.
