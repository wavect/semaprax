# Fixed imported-Point metadata migration (historical 67b capture)

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

This is source-only pin preparation and a static byte proof. It does not claim a
new build, producer run, current-head acceptance, full quality gate, or issue
closure. The actual capture is from
`67b1b02141f19040fdd1b45bc9bea16dcff5d74e`; its public builder debit is
1,114,864 bytes. Newer heads require their own actual build/readiness/capture.

The unchanged 1,711-byte fixture and complete canonical module facts identify
one intentional semantic change: imported scalar `Point` in `public.app`
selects `semaprax.graph.v10` instead of v74. Provider metadata remains v10.
`CAUSE.md` records the implementation cause and owning contract. This changes
the full manifest and therefore the real workspace revision and analysis digest
chain. This is explicitly accounted for here; budget-only admission still
refuses it (`strict-original-refusal.stderr`). No allowed path was widened.

`actual67b/` contains unchanged captured wires, candidate manifest, source and
binary bindings, producer statuses and logs. `actual-authentication/` retains
the actual readiness, build/producer receipts and round manifest, with exact
hashes in `transition-binding.json`. Archived receipt validation establishes
historical provenance only; it does not impose a false current ROOT-head claim.

`validate-transition.py` authenticates those inputs, checks canonical wire/self
and nested enclosing digests, verifies the complete fixture at the capture's
Git head, independently reconstructs both complete canonical manifests and
recomputes both revisions including their LF. The only semantic primitive it
reverses is that exact Point schema selection, followed by the authenticated
revision and digest cascade. It emits a labelled **DERIVED** compatibility view.
That view is never passed to the producer authority checker. Only the unchanged
history receiver consumes it, restoring all ten complete original wires in each
independent 47/83ca/55e domain. Historical files under `/Volumes/Semaprax/provenance`
remain prerequisites of the unchanged receiver. Their hashes remain checked.
`complete-pin-source-inverse.json` and `actual-only-pin-update.patch` record
complete source before/after bytes; installed pins use actual capture bytes.

Replay historical static proof (no compiler/build/test/capture):

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 \
  docs/evidence/managed67b-semantic-history-20261016/validate-transition.py
```

For a later frozen head, supply actual operands explicitly. The output directory
must be fresh. The preserved original capture frontier enforces live ROOT head,
clean source, exact owner selectors, binary roles and external receipt hashes.
Readiness/build/producer paths must refer to the same actual round; the readiness
hash also binds that round's manifest. No future debit or hash is predicted.
The tool emits a patch, without applying it. Recognized source pin baselines are
only the complete recorded 83ca or 67b values; other pin drift refuses.

```sh
TMPDIR=/Volumes/Semaprax/runtime-tmp PYTHONDONTWRITEBYTECODE=1 python3 \
  docs/evidence/managed67b-semantic-history-20261016/validate-transition.py \
  --root "$ROOT" --capture "$ACTUAL_CAPTURE" --expected-head "$ACTUAL_HEAD" \
  --round "$ACTUAL_ROUND" --readiness "$ACTUAL_READINESS" \
  --readiness-sha256 "$ACTUAL_READINESS_SHA256" \
  --build-receipt "$ACTUAL_ROUND/owning-build-result.json" \
  --build-receipt-sha256 "$ACTUAL_BUILD_RECEIPT_SHA256" \
  --producer-receipt "$ACTUAL_ROUND/prelude-pin-producers-result.json" \
  --producer-receipt-sha256 "$ACTUAL_PRODUCER_RECEIPT_SHA256" \
  --out "$FRESH_SSD_OUTPUT"
```

Focused owning selectors for the parent's actual post-integration verification:

- Workspace: `semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact`
- Workspace: `semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact`
- Library: `workspace_graph::type_proof_tests::projection_tests::rendered_document_has_literal_sha_exact_wire_order_and_digest_binding`
- Capture-only ignored library producer: `workspace_graph::type_proof_tests::projection_tests::emit_projection_wire_pin_candidate`

No Rust tests, builds or Cargo commands were run for this source-only migration.
